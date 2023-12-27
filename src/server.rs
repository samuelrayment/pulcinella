use std::{collections::HashMap, convert::Infallible, net::SocketAddr, sync::Arc};

use http_body_util::{BodyExt, Empty, Full};
use hyper::{
    body::{Body, Bytes},
    server::conn::http1,
    service::service_fn,
    Request, Response,
};
use hyper_util::rt::TokioIo;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::RwLock,
};

use crate::interchange::{Command, InstanceId, InstanceResponse};

pub async fn handler<T>(
    req: Request<T>,
    state: Arc<RwLock<State>>,
    mode: Mode,
) -> Result<Response<Full<Bytes>>, Infallible>
where
    T: Body,
    T::Error: std::fmt::Debug,
{
    match (req.method(), req.uri().path()) {
        (&hyper::Method::POST, "/cp") => handle_control_plane(req, state).await,
        _ => match mode {
            Mode::Mock => Ok(Response::builder()
                .status(404)
                .body(Full::new(Bytes::from_static(b"Not Found")))
                .unwrap()),
            Mode::Proxy => proxy_handler(req).await,
        },
    }
}

async fn proxy_handler<T>(req: Request<T>) -> Result<Response<Full<Bytes>>, Infallible>
where
    T: Body,
    T::Error: std::fmt::Debug,
{
    println!("method: {:?}", req.method());
    println!("path: {}", req.uri().path());
    println!("headers: {:?}", req.headers());
    let host_header = req.headers().get("host").unwrap();
    let url = host_header.to_str().unwrap().parse::<hyper::Uri>().unwrap();
    let host = url.host().expect("uri has no host");
    let port = url.port_u16().unwrap_or(80);
    let request_headers = req.headers().clone();
    println!("body: {:?}", req.into_body().collect().await.unwrap().to_bytes());
    let address = format!("{}:{}", host, port);
    let stream = TcpStream::connect(address).await.unwrap();
    let io = TokioIo::new(stream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await.unwrap();

    // Spawn a task to poll the connection, driving the HTTP state
    tokio::task::spawn(async move {
        if let Err(err) = conn.await {
            println!("Connection failed: {:?}", err);
        }
    });

    let authority = url.authority().unwrap().clone();
    let mut builder = Request::builder()
        .uri(url)
        .header(hyper::header::HOST, authority.as_str());
    let headers = builder.headers_mut().unwrap();
    *headers = request_headers;
    let req = builder
        .body(Empty::<Bytes>::new())
        .unwrap();
    println!("request: {:?}", req);
    let res = sender.send_request(req).await.unwrap();
    let res_status = res.status();
    let res_headers = res.headers().clone();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();

    let mut builder = Response::builder().status(res_status);
    let headers_map = builder.headers_mut().unwrap();
    *headers_map = res_headers;
    let response = builder.body(Full::new(bytes)).unwrap();
    println!("response: {:?}", response);

    Ok(response)
}

async fn handle_control_plane<T>(
    req: Request<T>,
    state: Arc<RwLock<State>>,
) -> Result<Response<Full<Bytes>>, Infallible>
where
    T: Body,
    T::Error: std::fmt::Debug,
{
    let body = req.into_body().collect().await.unwrap().to_bytes();
    let command = serde_json::from_slice::<Command>(&body).unwrap();
    match command {
        Command::CreateInstance => {
            let instance = Instance {};
            let instance_id = uuid7::uuid7().to_string();
            {
                let mut state = state.write().await;
                state.instances.insert(instance_id.clone(), instance);
            }
            let instance_response = InstanceResponse {
                instance: InstanceId(instance_id),
            };
            Ok(Response::builder()
                .status(200)
                .body(Full::new(Bytes::from(
                    serde_json::to_string(&instance_response).unwrap(),
                )))
                .unwrap())
        }
        Command::InstallMock { instance, mock } => todo!(),
    }
}

pub async fn bind_socket(
    addr: SocketAddr,
) -> Result<(u16, TcpListener), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind(addr).await?;
    let port = listener.local_addr()?.port();
    Ok((port, listener))
}

pub async fn run(
    listener: TcpListener,
    mode: Mode,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let state = Arc::new(RwLock::new(State {
        instances: HashMap::new(),
    }));
    loop {
        let state = state.clone();
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(move |req| handler(req, state.clone(), mode)))
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}

#[derive(Debug)]
pub struct State {
    instances: HashMap<String, Instance>,
}

#[derive(Debug)]
pub struct Instance {}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Mode {
    Mock,
    Proxy,
}
