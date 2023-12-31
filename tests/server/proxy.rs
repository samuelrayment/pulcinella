use fake::{Fake, Faker};
use http_body_util::{BodyExt, Full};
use hyper::{
    body::{Bytes, Incoming},
    server::conn::http1,
    service::service_fn,
    Request, Response,
};
use hyper_util::rt::TokioIo;
use reqwest::header::HeaderValue;
use std::{convert::Infallible, net::SocketAddr};
use tokio::net::TcpListener;

use wasm_test_server::server::{bind_socket, Mode};

use crate::helpers::start_server;

#[tokio::test]
async fn should_proxy_through_to_real_server() {
    let header_name = Faker.fake::<String>();
    let header_value = Faker.fake::<String>();
    let proxy_port = start_proxied_server(
        ProxyExpectations::default(),
        #[allow(clippy::needless_update)]
        ProxyResponseOptions {
            headers: vec![(header_name.clone(), header_value.clone())],
            ..Default::default()
        },
    )
    .await;
    let client = setup_server().await;

    let response = client
        .get(&format!("http://localhost:{}/", proxy_port))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(
        Some(header_value.as_str()),
        response
            .headers()
            .get(header_name)
            .and_then(|i: &HeaderValue| i.to_str().ok())
    );
    assert_eq!(200, response.status());
    assert_eq!("hello", response.text().await.unwrap());
}

#[tokio::test]
async fn should_proxy_through_headers_to_real_server() {
    let header_name = Faker.fake::<String>();
    let header_value = Faker.fake::<String>();
    let proxy_port = start_proxied_server(
        ProxyExpectations {
            expected_headers: vec![(header_name.clone(), header_value.clone())],
            ..Default::default()
        },
        ProxyResponseOptions::default(),
    )
    .await;
    let client = setup_server().await;

    let response = client
        .get(&format!("http://localhost:{}/", proxy_port))
        .header(header_name.clone(), header_value.clone())
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(200, response.status());
    assert_eq!("hello", response.text().await.unwrap());
}

#[tokio::test]
async fn should_proxy_through_post_and_body() {
    let body = Faker.fake::<String>();
    let proxy_port = start_proxied_server(
        ProxyExpectations {
            expected_body: Some(body.clone()),
            expected_method: Some("POST".to_string()),
            ..Default::default()
        },
        ProxyResponseOptions::default(),
    )
    .await;
    let client = setup_server().await;

    let response = client
        .post(&format!("http://localhost:{}/", proxy_port))
        .body(body.clone())
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(200, response.status());
    assert_eq!("hello", response.text().await.unwrap());
}

#[tokio::test]
async fn should_respond_with_bad_request_if_host_header_not_correctly_formed() {
    let proxy_port = start_proxied_server(
        ProxyExpectations::default(),
        ProxyResponseOptions::default(),
    )
    .await;
    let client = setup_server().await;

    let response = client
        .post(&format!("http://localhost:{}/", proxy_port))
        .header("host", "not a url")
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(400, response.status());
    assert_eq!("Bad host header", response.text().await.unwrap());
}

#[tokio::test]
async fn should_respond_with_upstream_not_found_if_server_not_available() {
    let _proxy_port = start_proxied_server(
        ProxyExpectations::default(),
        ProxyResponseOptions::default(),
    )
    .await;
    let client = setup_server().await;

    let response = client
        .post(&format!("http://localhost:{}/", 1))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(502, response.status());
    assert_eq!("Upstream not found", response.text().await.unwrap());
}

#[tokio::test]
async fn should_respond_with_upstream_for_different_path() {
    let path = Faker.fake::<String>();
    let proxy_port = start_proxied_server(
        ProxyExpectations {
            expected_path: Some(format!("/{}", path)),
            ..Default::default()
        },
        ProxyResponseOptions::default(),
    )
    .await;
    let client = setup_server().await;

    let response = client
        .post(&format!("http://localhost:{}/{}", proxy_port, path))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(200, response.status());
    assert_eq!("hello", response.text().await.unwrap());
}

async fn setup_server() -> reqwest::Client {
    let server_ports = start_server(Mode::Proxy).await;
    create_client(server_ports.mock)
}

fn create_client(server_port: u16) -> reqwest::Client {
    let proxy = reqwest::Proxy::http(format!("http://localhost:{}/", server_port)).unwrap();

    reqwest::ClientBuilder::new().proxy(proxy).build().unwrap()
}

async fn start_proxied_server(
    expectations: ProxyExpectations,
    response_options: ProxyResponseOptions,
) -> u16 {
    let binding = bind_socket(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let server = run_proxied(binding.listener, expectations, response_options);
    tokio::spawn(server);

    binding.port
}

async fn run_proxied(
    listener: TcpListener,
    expectations: ProxyExpectations,
    response_options: ProxyResponseOptions,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    loop {
        let expectations = expectations.clone();
        let response_options = response_options.clone();
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(
                    io,
                    service_fn(move |req| {
                        proxied_handler(req, expectations.clone(), response_options.clone())
                    }),
                )
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}

async fn proxied_handler(
    req: Request<Incoming>,
    expectations: ProxyExpectations,
    response_options: ProxyResponseOptions,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let request_headers = req.headers();
    for (header_name, header_value) in expectations.expected_headers {
        if request_headers
            .get(header_name)
            .and_then(|i| i.to_str().ok())
            != Some(&header_value)
        {
            return error_response("bad header");
        }
    }

    if let Some(expected_method) = expectations.expected_method {
        if req.method().as_str() != expected_method {
            return error_response("bad method");
        }
    }

    if let Some(expected_path) = expectations.expected_path {
        if req.uri().path() != expected_path {
            return error_response("bad path");
        }
    }

    if let Some(expected_body) = expectations.expected_body {
        let body = req.into_body().collect().await.unwrap().to_bytes();
        if body != expected_body {
            return error_response("bad body");
        }
    }

    let builder = Response::builder().status(200);
    let builder = response_options
        .headers
        .iter()
        .fold(builder, |builder, (name, value)| {
            builder.header(name, value)
        });
    Ok(builder
        .body(Full::new(Bytes::from_static(b"hello")))
        .unwrap())
}

fn error_response(message: &'static str) -> Result<Response<Full<Bytes>>, Infallible> {
    Ok(Response::builder()
        .status(400)
        .body(Full::new(Bytes::from(message)))
        .unwrap())
}

#[derive(Clone, Default)]
struct ProxyExpectations {
    expected_headers: Vec<(String, String)>,
    expected_body: Option<String>,
    expected_method: Option<String>,
    expected_path: Option<String>,
}

#[derive(Clone, Default)]
struct ProxyResponseOptions {
    headers: Vec<(String, String)>,
}
