use std::{collections::HashMap, convert::Infallible, net::SocketAddr, sync::Arc};

use crate::{
    hyper_helpers::{Address, HyperHelpers, ResponseExt},
    interchange::{
        Command, InstallError, InstallResponse, InstanceId, InstanceResponse, Method, MockRule,
    },
};
use eyre::{eyre, WrapErr};
use http_body_util::{BodyExt, Full};
use hyper::{
    body::{Body, Bytes},
    server::conn::http1,
    Request, Response,
};
use hyper_util::{rt::TokioIo, service::TowerToHyperService};
use thiserror::Error;
use tokio::{net::TcpListener, sync::RwLock};
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tracing::{info, trace};

#[tracing::instrument(skip(state, req), level = "trace", fields(http.method=%req.method(), http.uri=%req.uri()))]
pub async fn control_handler<T>(
    req: Request<T>,
    state: SequentialState,
) -> Result<Response<Full<Bytes>>, Infallible>
where
    T: Body + std::fmt::Debug,
    T::Error: std::fmt::Debug,
{
    match (req.method(), req.uri().path()) {
        (&hyper::Method::POST, "/") => handle_control_plane(req, state).await,
        _ => respond(404, "Not Found"),
    }
}

#[tracing::instrument(skip(state, mode, req), level = "info", fields(http.method=%req.method(), http.uri=%req.uri()))]
async fn mock_handler<T>(
    req: Request<T>,
    state: SequentialState,
    mode: Mode,
) -> Result<Response<Full<Bytes>>, Infallible>
where
    T: Body + std::fmt::Debug,
    T::Error: std::fmt::Debug,
{
    let req = UnpackedRequest::from_request(req).await;
    {
        let instance = state.instance.read().await;

        if let Some((_, mocks)) = instance.as_ref() {
            for mock in mocks {
                if mock.matches(&req) {
                    info!("Found matching mock rule");
                    let builder = Response::builder().status(mock.then.status);
                    let builder = mock
                        .then
                        .headers
                        .iter()
                        .fold(builder, |builder, (k, v)| builder.header(k, v));
                    return Ok(builder
                        .body(Full::new(Bytes::from(mock.then.body.clone())))
                        .unwrap());
                }
            }
        }
    }

    match mode {
        Mode::Proxy => match request_from_proxy(req).await {
            Ok(res) => proxy_response_to_response(res)
                .await
                .map(|res| {
                    info!("Proxying response");
                    res
                })
                .or_else(|e| e.to_response()),
            Err(e) => e.to_response(),
        },
        Mode::Mock => respond(404, "Not Found"),
    }
}

async fn request_from_proxy(
    req: UnpackedRequest,
) -> Result<Response<hyper::body::Incoming>, ProxyError> {
    let address = address_from_request(&req)
        .ok_or(ProxyError::BadHostHeader)?;

    let mut builder = Request::builder().method(req.method).uri(req.uri.path());
    if let Some(headers) = builder.headers_mut() {
        *headers = req.headers;
    }
    let proxied_req = builder
        .body(Full::new(req.body))
        .map_err(|_| ProxyError::CannotReadRequestBody)?;

    let response = crate::hyper_helpers::HyperHelpers::send(&address, proxied_req)
        .await
        .map_err(|err| {
            use crate::hyper_helpers::RequestError::*;
            match err {
                UpstreamNotHttp => ProxyError::UpstreamNotHttp,
                UpstreamSendError => ProxyError::UpstreamSendError,
                CannotConnect => ProxyError::UpstreamNotFound,
                CannotSerializeBody => ProxyError::UpstreamSendError,
            }
        })?;

    Ok(response)
}

fn address_from_request(req: &UnpackedRequest) -> Option<Address> {
    let host_header = req.headers.get("host")?;
    let host_string = host_header.to_str().ok()?;
    HyperHelpers::parse_address(host_string).ok()
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
            ProxyError::BadHostHeader => respond(400, "Bad host header"),
            ProxyError::UpstreamNotFound => respond(502, "Upstream not found"),
            ProxyError::UpstreamNotHttp => respond(502, "Upstream not HTTP"),
            ProxyError::CannotReadRequestBody => respond(502, "Cannot read request body"),
            ProxyError::UpstreamSendError => respond(502, "Upstream send error"),
            ProxyError::CannotReadResponseBody => respond(502, "Cannot read response body"),
            ProxyError::CannotConstructResponseBody => {
                respond(502, "Cannot construct response body")
            }
        }
    }
}

async fn proxy_response_to_response(
    res: Response<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, ProxyError> {
    let res_status = res.status();
    let res_headers = res.headers().clone();
    let bytes = res
        .bytes()
        .await
        .map_err(|_| ProxyError::CannotReadResponseBody)?;

    let mut builder = Response::builder().status(res_status);
    if let Some(headers_map) = builder.headers_mut() {
        *headers_map = res_headers;
    }
    let response = builder
        .body(Full::new(bytes))
        .map_err(|_| ProxyError::CannotConstructResponseBody)?;

    Ok(response)
}

async fn handle_control_plane<T>(
    req: Request<T>,
    state: SequentialState,
) -> Result<Response<Full<Bytes>>, Infallible>
where
    T: Body,
    T::Error: std::fmt::Debug,
{
    let command = match parse_command(req).await {
        Ok(command) => command,
        Err(err) => {
            info!(error=%err, "Cannot parse command");
            return respond(400, "Bad Request");
        }
    };
    trace!("Received Command: {command:?}");
    match command {
        Command::CreateInstance => {
            let instance_id = InstanceId(uuid7::uuid7().to_string());
            {
                info!("attempt to lock: {:?}", state.instance.try_write());
                let mut instance = state.instance.write().await;
                *instance = Some((instance_id.clone(), vec![]));
            }
            info!(instance=?instance_id, "Created instance");
            let instance_response = InstanceResponse {
                instance: instance_id,
                url: format!("http://localhost:{}", state.mock_port),
            };
            respond(200, serde_json::to_string(&instance_response).unwrap())
        }
        Command::InstallMock {
            mock,
            instance: instance_id,
        } => {
            if instance_id != state.instance.read().await.as_ref().unwrap().0 {
                let body = serde_json::to_string(&InstallError::InstanceNotFound).unwrap();
                return respond(400, body);
            }

            let mut instance = state.instance.write().await;
            info!("Mock installed: {:?}", mock.when);
            if let Some((_, mocks)) = instance.as_mut() {
                mocks.push(mock);
                mocks.sort_by_key(|m| m.priority());
                mocks.reverse();
            }
            respond(200, serde_json::to_string(&InstallResponse).unwrap())
        }
    }
}

async fn parse_command<T>(req: Request<T>) -> Result<Command, eyre::Report>
where
    T: Body,
    T::Error: std::fmt::Debug,
{
    let body = req
        .into_body()
        .collect()
        .await
        .map_err(|_| eyre!("Cannot read body"))?
        .to_bytes();
    let command = serde_json::from_slice::<Command>(&body).wrap_err("Invalid command sent")?;

    Ok(command)
}

fn respond(status: u16, body: impl Into<Bytes>) -> Result<Response<Full<Bytes>>, Infallible> {
    Ok(Response::builder()
        .status(status)
        .header("Access-Control-Allow-Origin", "*")
        .body(Full::new(body.into()))
        .unwrap())
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

pub async fn run_controlplane(
    listener: TcpListener,
    state: SequentialState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    loop {
        let state = state.clone();
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            let service = ServiceBuilder::new()
                .layer(CorsLayer::permissive())
                .service_fn(move |req| control_handler(req, state.clone()));
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, TowerToHyperService::new(service))
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}

pub async fn run_mock(
    listener: TcpListener,
    state: SequentialState,
    mode: Mode,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    loop {
        let state = state.clone();
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            let service = ServiceBuilder::new()
                .layer(CorsLayer::permissive())
                .service_fn(move |req| mock_handler(req, state.clone(), mode));
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, TowerToHyperService::new(service))
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}

type Instance = Arc<RwLock<Option<(InstanceId, Vec<MockRule>)>>>;

#[derive(Debug, Clone)]
pub struct SequentialState {
    mock_port: u16,
    instance: Instance,
}

impl SequentialState {
    pub fn new(mock_port: u16) -> Self {
        Self {
            mock_port,
            instance: Arc::default(),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Mode {
    Mock,
    Proxy,
}

trait RequestMatch {
    fn matches(&self, req: &UnpackedRequest) -> bool;
    fn priority(&self) -> u8;
    fn method_match(method: &Method, req_method: &hyper::Method) -> bool;
}

impl RequestMatch for MockRule {
    fn matches(&self, req: &UnpackedRequest) -> bool {
        let params_match = self.check_params_match(req);
        let method_match = self
            .when
            .method
            .as_ref()
            .map(|m| Self::method_match(m, &req.method))
            .unwrap_or(true);
        let path_match = self.when.match_path == req.uri.path();

        path_match && params_match && method_match
    }

    fn priority(&self) -> u8 {
        let form_data = if self.when.form_data.is_empty() { 0 } else { 1 };
        let method = if self.when.method.is_some() { 1 } else { 0 };
        form_data + method
    }

    fn method_match(method: &Method, req_method: &hyper::Method) -> bool {
        matches!(
            (method, req_method),
            (Method::GET, &hyper::Method::GET)
                | (Method::POST, &hyper::Method::POST)
                | (Method::PUT, &hyper::Method::PUT)
                | (Method::DELETE, &hyper::Method::DELETE)
        )
    }
}

impl MockRule {
    fn check_params_match(&self, req: &UnpackedRequest) -> bool {
        let params = form_urlencoded::parse(req.body.as_ref())
            .into_owned()
            .collect::<HashMap<String, String>>();
        let correct_param_count = params.len() == self.when.form_data.len();
        let correct_params = self
            .when
            .form_data
            .iter()
            .all(|(key, value)| params.get(key).map(|v| v == value).unwrap_or(false));
        correct_param_count && correct_params
    }
}

#[derive(Debug)]
struct UnpackedRequest {
    method: hyper::Method,
    headers: hyper::HeaderMap,
    uri: hyper::Uri,
    body: Bytes,
}

impl UnpackedRequest {
    async fn from_request<T>(req: Request<T>) -> Self
    where
        T: Body + std::fmt::Debug,
        T::Error: std::fmt::Debug,
    {
        let method = req.method().clone();
        let headers = req.headers().clone();
        let uri = req.uri().clone();
        let body = req
            .into_body()
            .collect()
            .await
            .expect("Cannot read body")
            .to_bytes();
        Self {
            method,
            headers,
            uri,
            body,
        }
    }
}
