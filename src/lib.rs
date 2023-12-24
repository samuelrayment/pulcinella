use std::{convert::Infallible, net::SocketAddr};

use http_body_util::Full;
use hyper::{Request, Response, body::Bytes, server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

pub async fn hello(_: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    Ok(Response::builder()
        .status(404).body(Full::new(Bytes::from_static(b"Not Found"))).unwrap())
}

pub async fn bind_socket(addr: SocketAddr) -> Result<(u16, TcpListener), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind(addr).await?;
    let port = listener.local_addr()?.port();
    Ok((port, listener))
}

pub async fn run(listener: TcpListener) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(hello))
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}
