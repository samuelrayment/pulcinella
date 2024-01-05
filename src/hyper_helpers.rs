use std::error::Error;

use async_trait::async_trait;
use http_body_util::{BodyExt, Full};
use hyper::{
    body::{Body, Bytes, Incoming},
    Request, Response,
};
use hyper_util::rt::TokioIo;
use serde::de::DeserializeOwned;
use thiserror::Error;
use tokio::net::TcpStream;
pub struct HyperHelpers;

impl HyperHelpers {
    pub async fn send<B>(
        address: &str,
        request: Request<B>,
    ) -> Result<Response<Incoming>, RequestError>
    where
        B: Body + 'static + Send,
        B::Data: Send,
        B::Error: Into<Box<dyn Error + Send + Sync>>,
    {
        let stream = TcpStream::connect(address)
            .await
            .map_err(|_| RequestError::CannotConnect)?;
        let io = TokioIo::new(stream);
        let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
            .await
            .map_err(|_| RequestError::UpstreamNotHttp)?;

        // Spawn a task to poll the connection, driving the HTTP state
        tokio::task::spawn(async move {
            if let Err(err) = conn.await {
                println!("Connection failed: {:?}", err);
            }
        });

        sender
            .send_request(request)
            .await
            .map_err(|_| RequestError::UpstreamSendError)
    }
}

#[derive(Error, Debug, PartialEq)]
pub enum RequestError {
    #[error("Upstream is not HTTP")]
    UpstreamNotHttp,
    #[error("Cannot send request to upstream")]
    UpstreamSendError,
    #[error("Cannot connect to upstream")]
    CannotConnect,
    #[error("Cannot serialize body")]
    CannotSerializeBody,
}

#[derive(Error, Debug, PartialEq)]
pub enum ResponseError {
    #[error("Cannot fetch body")]
    CannotFetchBody,
    #[error("Cannot deserialize body")]
    DeserializeError,
}

pub trait RequestExt {
    fn json<T>(self, body: T) -> Result<Request<Full<Bytes>>, RequestError>
    where
        T: serde::Serialize;
}

impl RequestExt for hyper::http::request::Builder {
    fn json<T>(self, body: T) -> Result<Request<Full<Bytes>>, RequestError>
    where
        T: serde::Serialize,
    {
        let message =
            serde_json::to_string(&body).map_err(|_| RequestError::CannotSerializeBody)?;

        self.header("content-type", "application/json")
            .body(Full::new(Bytes::from(message)))
            .map_err(|_| RequestError::UpstreamSendError)
    }
}

#[async_trait]
pub trait ResponseExt {
    async fn bytes(self) -> Result<Bytes, ResponseError>;
    async fn json<T>(self) -> Result<T, ResponseError>
    where
        T: DeserializeOwned;
}

#[async_trait]
impl ResponseExt for Response<Incoming> {
    async fn bytes(self) -> Result<Bytes, ResponseError> {
        let bytes = self
            .into_body()
            .collect()
            .await
            .map_err(|_| ResponseError::CannotFetchBody)?
            .to_bytes();
        Ok(bytes)
    }

    async fn json<T>(self) -> Result<T, ResponseError>
    where
        T: DeserializeOwned,
    {
        let bytes = self.bytes().await?;
        let json = serde_json::from_slice(&bytes).map_err(|_| ResponseError::DeserializeError)?;
        Ok(json)
    }
}
