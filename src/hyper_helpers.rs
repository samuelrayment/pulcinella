use std::error::Error;

use async_trait::async_trait;
use http_body_util::BodyExt;
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

        Ok(sender
            .send_request(request)
            .await
            .map_err(|_| RequestError::UpstreamSendError)?)
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
}

#[derive(Error, Debug, PartialEq)]
pub enum ResponseError {
    #[error("Cannot fetch body")]
    CannotFetchBody,
    #[error("Cannot deserialize body")]
    DeserializeError,
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
