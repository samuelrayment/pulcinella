use thiserror::Error;

pub struct NetworkClient;

#[cfg(all(feature = "client", not(target_arch = "wasm32")))]
impl NetworkClient {
    pub async fn send<T, U, E>(control_plane_url: &str, message: &T) -> Result<U, ClientNetworkError<E>>
    where
        T: serde::Serialize,
        U: serde::de::DeserializeOwned,
        E: serde::de::DeserializeOwned,
    {
        let response = reqwest::Client::new()
            .post(control_plane_url)
            .json(message)
            .send()
            .await
            .map_err(|_| ClientNetworkError::FailedToConnectToMockServer)?;
        if response.status().is_success() {
            response
                .json::<U>()
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

#[cfg(all(feature = "client", any(target_arch = "wasm32", rust_analyzer)))]
impl NetworkClient {
    pub async fn send<T, U, E>(control_plane_url: &str, message: &T) -> Result<U, ClientNetworkError<E>>
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
