use fake::{faker, Faker, Fake};
use http_body_util::Full;
use hyper::{
    body::{Body, Bytes, Incoming},
    server::conn::http1,
    service::service_fn,
    Request, Response,
};
use hyper_util::rt::TokioIo;
use reqwest::header::HeaderValue;
use std::{collections::HashMap, convert::Infallible, net::SocketAddr, sync::Arc};
use tokio::{net::TcpListener, sync::RwLock};

use wasm_test_server::{
    client::Client,
    server::{bind_socket, run, Mode},
};

#[tokio::test]
async fn should_proxy_through_to_real_server() {
    let server_port = start_server().await;
    let proxy_port = start_proxied_server(vec![]).await;
    let client = create_client(server_port);

    let response = client
        .get(&format!("http://localhost:{}/", proxy_port))
        .send()
        .await
        .expect("Failed to send request");
    let response_status = response.status();
    let response_headers = response.headers().clone();
    let body = response.text().await.unwrap();

    assert_eq!(200, response_status);
    assert_eq!("hello", body);
    assert_eq!(
        Some("extra-value"),
        response_headers.get("extra-header").and_then(|i: &HeaderValue| i.to_str().ok())
    );
}

#[tokio::test]
async fn should_proxy_through_headers_to_real_server() {
    let header_name = Faker.fake::<String>();
    let header_value = Faker.fake::<String>();
    let server_port = start_server().await;
    let proxy_port = start_proxied_server(vec![(header_name.clone(), header_value.clone())]).await;
    let client = create_client(server_port);

    let response = client
        .get(&format!("http://localhost:{}/", proxy_port))
        .header(header_name.clone(), header_value.clone())
        .send()
        .await
        .expect("Failed to send request");
    let response_status = response.status();
    let body = response.text().await.unwrap();

    assert_eq!(200, response_status);
    assert_eq!("hello", body);
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

async fn start_proxied_server(expected_headers: Vec<(String, String)>) -> u16 {
    let (port, listener) = bind_socket(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let server = run_proxied(listener, expected_headers);
    let _ = tokio::spawn(server);

    port
}

async fn run_proxied(
    listener: TcpListener,
    expected_headers: Vec<(String, String)>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    loop {
        let expected_headers = expected_headers.clone();
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(move |req| proxied_handler(req, expected_headers.clone())))
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}

async fn proxied_handler(req: Request<Incoming>, expected_headers: Vec<(String, String)>) -> Result<Response<Full<Bytes>>, Infallible> {
    let request_headers = req.headers();
    for (header_name, header_value) in expected_headers {
        if request_headers.get(header_name).and_then(|i| i.to_str().ok()) != Some(&header_value) {
            return Ok(Response::builder()
                .status(400)
                .body(Full::new(Bytes::from_static(b"bad header")))
                .unwrap());
        }
    }

    Ok(Response::builder()
        .status(200)
        .header("extra-header", "extra-value")
        .body(Full::new(Bytes::from_static(b"hello")))
        .unwrap())
}
