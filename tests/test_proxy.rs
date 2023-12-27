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

use wasm_test_server::server::{bind_socket, run, Mode};

#[tokio::test]
async fn should_proxy_through_to_real_server() {
    let header_name = Faker.fake::<String>();
    let header_value = Faker.fake::<String>();
    let server_port = start_server().await;
    let proxy_port = start_proxied_server(
        ProxyExpectations::default(),
        ProxyResponseOptions {
            headers: vec![(header_name.clone(), header_value.clone())],
            ..Default::default()
        }
    )
    .await;
    let client = create_client(server_port);

    let response = client
        .get(&format!("http://localhost:{}/", proxy_port))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(
        Some(header_value.as_str()),
        response.headers()
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
    let server_port = start_server().await;
    let proxy_port = start_proxied_server(
        ProxyExpectations {
            expected_headers: vec![(header_name.clone(), header_value.clone())],
            ..Default::default()
        },
        ProxyResponseOptions::default(),
    )
    .await;
    let client = create_client(server_port);

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
    let server_port = start_server().await;
    let proxy_port = start_proxied_server(
        ProxyExpectations {
            expected_body: Some(body.clone()),
            expected_method: Some("POST".to_string()),
            ..Default::default()
        },
        ProxyResponseOptions::default(),
    )
    .await;
    let client = create_client(server_port);

    let response = client
        .post(&format!("http://localhost:{}/", proxy_port))
        .body(body.clone())
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(200, response.status());
    assert_eq!("hello", response.text().await.unwrap());
}

fn create_client(server_port: u16) -> reqwest::Client {
    let proxy = reqwest::Proxy::http(&format!("http://localhost:{}/", server_port)).unwrap();
    let client = reqwest::ClientBuilder::new().proxy(proxy).build().unwrap();
    client
}

async fn start_server() -> u16 {
    let (port, listener) = bind_socket(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let server = run(listener, Mode::Proxy);
    let _ = tokio::spawn(server);
    port
}

async fn start_proxied_server(
    expectations: ProxyExpectations,
    response_options: ProxyResponseOptions,
) -> u16 {
    let (port, listener) = bind_socket(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let server = run_proxied(listener, expectations, response_options);
    let _ = tokio::spawn(server);

    port
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
}

#[derive(Clone, Default)]
struct ProxyResponseOptions {
    headers: Vec<(String, String)>,
}
