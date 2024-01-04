use std::{error::Error, net::TcpStream};

use hyper::{
    body::{Body, Incoming, Bytes},
    Request, Response,
};
use hyper_util::rt::TokioIo;
use eyre::WrapErr;
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

pub enum RequestError {
    UpstreamNotHttp,
    UpstreamSendError,
    CannotConnect,
}

pub trait ResponseExt {
    async fn bytes(self) -> Result<Bytes, eyre::Error>;
    //fn json<T>(self) -> Result<T, dyn serde::de::Error> where T: DeserializeOwned;
}

impl ResponseExt for Response<Incoming> {
    async fn bytes(self) -> Result<Bytes, eyre::Error> {
            let bytes = self
                .into_body()
                .collect()
                .await
            .wrap_err("")
                .to_bytes();

    }
}
