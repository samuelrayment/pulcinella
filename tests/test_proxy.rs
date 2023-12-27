use http_body_util::Full;
use hyper::{
    body::{Body, Bytes, Incoming},
    server::conn::http1,
    service::service_fn,
    Request, Response,
};
use hyper_util::rt::TokioIo;
use std::{collections::HashMap, convert::Infallible, net::SocketAddr, sync::Arc};
use tokio::{net::TcpListener, sync::RwLock};

use wasm_test_server::{
    client::Client,
    server::{bind_socket, run, Mode},
};

#[tokio::test]
async fn should_proxy_through_to_real_server() {
    let server_port = start_server().await;
    let proxy_port = start_proxied_server().await;

    let proxy = reqwest::Proxy::http(&format!("http://localhost:{}/", server_port)).unwrap();
    let client = reqwest::ClientBuilder::new().proxy(proxy).build().unwrap();
    let response = client
        .get(&format!("http://localhost:{}/", proxy_port))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 200);
}

async fn start_server() -> u16 {
    let (port, listener) = bind_socket(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let server = run(listener, Mode::Proxy);
    let _ = tokio::spawn(server);
    port
}

async fn start_proxied_server() -> u16 {
    let (port, listener) = bind_socket(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let server = run_proxied(listener);
    let _ = tokio::spawn(server);

    port
}

pub async fn run_proxied(
    listener: TcpListener,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(move |req| proxied_handler(req)))
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}

pub async fn proxied_handler(_: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    Ok(Response::builder()
        .status(200)
        .body(Full::new(Bytes::from_static(b"hello")))
        .unwrap())
}
