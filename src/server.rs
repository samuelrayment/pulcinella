use std::{collections::HashMap, convert::Infallible, net::SocketAddr, sync::Arc};

use http_body_util::{BodyExt, Full};
use hyper::{
    body::{Body, Bytes},
    server::conn::http1,
    service::service_fn,
    Request, Response,
};
use hyper_util::rt::TokioIo;
use thiserror::Error;
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
    T: Body + std::fmt::Debug,
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
    T: Body + std::fmt::Debug,
    T::Error: std::fmt::Debug,
{
    match request_from_proxy(req).await {
        Ok(res) => proxy_response_to_response(res)
            .await
            .or_else(|e| e.to_response()),
        Err(e) => e.to_response(),
    }
}

async fn request_from_proxy<T>(
    req: Request<T>,
) -> Result<Response<hyper::body::Incoming>, ProxyError>
where
    T: Body + std::fmt::Debug,
    T::Error: std::fmt::Debug,
{
    println!("Request: {:?}", req);
    let method = req.method().clone();

    let url = req
        .headers()
        .get("host")
        .and_then(|host| host.to_str().ok()?.parse::<hyper::Uri>().ok())
        .ok_or(ProxyError::BadHostHeader)?;

    let host = url.host().ok_or(ProxyError::BadHostHeader)?;
    let port = url.port_u16().unwrap_or(80);
    let address = format!("{}:{}", host, port);

    let stream = TcpStream::connect(address)
        .await
        .map_err(|_| ProxyError::UpstreamNotFound)?;
    let io = TokioIo::new(stream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
        .await
        .map_err(|_| ProxyError::UpstreamNotHttp)?;

    // Spawn a task to poll the connection, driving the HTTP state
    tokio::task::spawn(async move {
        if let Err(err) = conn.await {
            println!("Connection failed: {:?}", err);
        }
    });

    let mut builder = Request::builder().method(method).uri(req.uri().path());
    let request_headers = req.headers().clone();
    let body = req
        .into_body()
        .collect()
        .await
        .map_err(|_| ProxyError::CannotReadRequestBody)?
        .to_bytes();

    if let Some(headers) = builder.headers_mut() {
        *headers = request_headers;
    }
    let proxied_req = builder
        .body(Full::new(body))
        .map_err(|_| ProxyError::CannotReadRequestBody)?;

    let res = sender
        .send_request(proxied_req)
        .await
        .map_err(|_| ProxyError::UpstreamSendError)?;
    Ok(res)
}

#[derive(Debug, Error)]
enum ProxyError {
    #[error("Bad host header")]
    BadHostHeader,
    #[error("Upstream not found")]
    UpstreamNotFound,
    #[error("Upstream does not support HTTP")]
    UpstreamNotHttp,
    #[error("Cannot read request body")]
    CannotReadRequestBody,
    #[error("Upstream send error")]
    UpstreamSendError,
    #[error("Cannot read response body")]
    CannotReadResponseBody,
    #[error("Cannot construct response body")]
    CannotConstructResponseBody,
}

impl ProxyError {
    fn to_response(&self) -> Result<Response<Full<Bytes>>, Infallible> {
        match self {
            ProxyError::BadHostHeader => Self::generate_response(400, "Bad host header"),
            ProxyError::UpstreamNotFound => Self::generate_response(502, "Upstream not found"),
            ProxyError::UpstreamNotHttp => Self::generate_response(502, "Upstream not HTTP"),
            ProxyError::CannotReadRequestBody => {
                Self::generate_response(502, "Cannot read request body")
            }
            ProxyError::UpstreamSendError => Self::generate_response(502, "Upstream send error"),
            ProxyError::CannotReadResponseBody => {
                Self::generate_response(502, "Cannot read response body")
            }
            ProxyError::CannotConstructResponseBody => {
                Self::generate_response(502, "Cannot construct response body")
            }
        }
    }

    fn generate_response(
        status: u16,
        message: &'static str,
    ) -> Result<Response<Full<Bytes>>, Infallible> {
        Ok(Response::builder()
            .status(status)
            .body(Full::new(Bytes::from(message)))
            .unwrap())
    }
}

async fn proxy_response_to_response(
    res: Response<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, ProxyError> {
    let res_status = res.status();
    let res_headers = res.headers().clone();
    let bytes = res
        .into_body()
        .collect()
        .await
        .map_err(|_| ProxyError::CannotReadResponseBody)?
        .to_bytes();

    let mut builder = Response::builder().status(res_status);
    if let Some(headers_map) = builder.headers_mut() {
        *headers_map = res_headers;
    }
    let response = builder.body(Full::new(bytes)).map_err(|_| {
        ProxyError::CannotConstructResponseBody
    })?;

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

pub struct SocketBinding {
    pub port: u16,
    pub listener: TcpListener,
}

pub async fn bind_socket(
    addr: SocketAddr,
) -> Result<SocketBinding, Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind(addr).await?;
    let port = listener.local_addr()?.port();
    Ok(SocketBinding { port, listener })
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
