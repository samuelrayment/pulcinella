use thiserror::Error;

pub struct NetworkClient;

#[cfg(all(feature = "client", not(target_arch = "wasm32")))]
impl NetworkClient {
    pub async fn send<T, U, E>(
        control_plane_url: &str,
        message: &T,
    ) -> Result<U, ClientNetworkError<E>>
    where
        T: serde::Serialize,
        U: serde::de::DeserializeOwned,
        E: serde::de::DeserializeOwned,
    {
        //let response = reqwest::Client::new()
        //    .post(control_plane_url)
        //    .json(message)
        //    .send()
        //    .await
        //    .map_err(|_| ClientNetworkError::FailedToConnectToMockServer)?;
        //if response.status().is_success() {
        //    response
        //        .json::<U>()
        //        .await
        //        .map_err(|_| ClientNetworkError::ResponseDeserializeError)
        //} else {
        //    response
        //        .json::<E>()
        //        .await
        //        .map_err(|_| ClientNetworkError::ResponseDeserializeError)
        //        .and_then(|e| Err(ClientNetworkError::Response(e)))
        //}

        use hyper::body::Bytes;
        use hyper_util::rt::TokioIo;
        use http_body_util::BodyExt;
        use tokio::net::TcpStream;

        // TODO perhaps some form of bad address network error
        let url = control_plane_url
            .parse::<hyper::Uri>()
            .map_err(|_| ClientNetworkError::FailedToConnectToMockServer)?;
        let host = url
            .host()
            .ok_or(ClientNetworkError::FailedToConnectToMockServer)?;
        let port = url.port_u16().unwrap_or(80);
        let address = format!("{}:{}", host, port);

        let stream = TcpStream::connect(address)
            .await
            .map_err(|_| ClientNetworkError::FailedToConnectToMockServer)?;

        let io = TokioIo::new(stream);
        let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
            .await
            .map_err(|_| ClientNetworkError::FailedToConnectToMockServer)?;

        // Spawn a task to poll the connection, driving the HTTP state
        tokio::task::spawn(async move {
            if let Err(err) = conn.await {
                println!("Connection failed: {:?}", err);
            }
        });

        let request = hyper::Request::builder()
            .method(hyper::Method::POST)
            .uri(url.path())
            .header("content-type", "application/json")
            .body(http_body_util::Full::new(
                Bytes::from(serde_json::to_string(message).unwrap())
            ))
            .unwrap();

        let response = sender.send_request(request).await.map_err(|_| ClientNetworkError::FailedToConnectToMockServer)?;

        let status = response.status();
        if status.is_success() {
            let bytes = response
                .into_body()
                .collect()
                .await
                .map_err(|_| ClientNetworkError::ResponseDeserializeError)?
                .to_bytes();
            let response = serde_json::from_slice::<U>(&bytes).map_err(|_| ClientNetworkError::ResponseDeserializeError)?;
            Ok(response)
        } else {
            let bytes = response
                .into_body()
                .collect()
                .await
                .map_err(|_| ClientNetworkError::ResponseDeserializeError)?
                .to_bytes();
            let response = serde_json::from_slice::<E>(&bytes).map_err(|_| ClientNetworkError::ResponseDeserializeError)?;
            Err(ClientNetworkError::Response(response))
        }
    }
}

#[cfg(all(feature = "client", any(target_arch = "wasm32", rust_analyzer)))]
impl NetworkClient {
    pub async fn send<T, U, E>(
        control_plane_url: &str,
        message: &T,
    ) -> Result<U, ClientNetworkError<E>>
    where
        T: serde::Serialize,
        U: serde::de::DeserializeOwned,
        E: serde::de::DeserializeOwned,
    {
        let response = gloo_net::http::Request::post(control_plane_url)
            .json(message)
            .map_err(|_| ClientNetworkError::FailedToSerializeCommand)?
            .send()
            .await
            .map_err(|_| ClientNetworkError::FailedToConnectToMockServer)?;
        if response.status() >= 200 && response.status() < 300 {
            response
                .json()
                .await
                .map_err(|_| ClientNetworkError::ResponseDeserializeError)
        } else {
            response
                .json::<E>()
                .await
                .map_err(|_| ClientNetworkError::ResponseDeserializeError)
                .and_then(|e| Err(ClientNetworkError::Response(e)))
        }
    }
}

#[derive(Error, Debug, PartialEq)]
#[allow(dead_code)]
pub enum ClientNetworkError<E> {
    #[error("Failed to deserialize response")]
    ResponseDeserializeError,
    #[error("Response")]
    Response(E),
    #[error("Failed to connect to mock server")]
    FailedToConnectToMockServer,
    #[error("Failed to serialize command")]
    FailedToSerializeCommand,
}
