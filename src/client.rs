use thiserror::Error;

use crate::interchange::{Command, InstanceId};

pub struct Client {
    control_plane: String,
    instance: InstanceId,
}

impl Client {
    pub async fn new(control_plane_url: &str) -> Result<Self, ClientError> {
        let body = reqwest::Client::new()
            .post(control_plane_url)
            .body(serde_json::to_string(&Command::CreateInstance).unwrap())
            .send()
            .await
            .map_err(|_| ClientError::FailedToConnectToMockServer)
            .and_then(|res| {
                if res.status().is_success() {
                    Ok(res)
                } else {
                    Err(ClientError::FailedToCreateTestInstance)
                }
            })?
            .text()
            .await
            .map_err(|_| ClientError::FailedToConnectToMockServer)?;

        let response = serde_json::from_str::<crate::interchange::InstanceResponse>(&body)
            .map_err(|_| ClientError::FailedToCreateTestInstance)?;

        println!("instance: {:?}", response.instance);

        Ok(Self {
            control_plane: String::from(control_plane_url),
            instance: response.instance,
        })
    }

    pub fn when<F>(&self, when: F) -> MockBuilder<WhenState>
    where
        F: FnOnce(WhenBuilder) -> WhenBuilder,
    {
        MockBuilder {
            state: when(WhenBuilder {match_path: String::from("")}).build(),
        }
    }

    pub fn url(&self) -> String {
        //self.base_url.clone()
        String::from("")
    }
}

pub enum Method {
    GET,
}

pub struct MockBuilder<State> {
    state: State,
}

impl MockBuilder<WhenState> {
    pub fn then<F>(self, then: F) -> MockBuilder<ThenState>
    where
        F: FnOnce(ThenBuilder) -> ThenBuilder,
    {
        MockBuilder {
            state: then(ThenBuilder{ status: 200 }).build(self.state),
        }
    }
}

impl MockBuilder<ThenState> {
    // TODO should this return an ID to be used to delete the mock?
    pub async fn send(self) {}
}

pub struct WhenBuilder {
    match_path: String,
}

impl WhenBuilder {
    pub fn path(mut self, path: &str) -> Self {
        self.match_path = String::from(path);
        self
    }

    fn build(self) -> WhenState {
        WhenState {
            match_path: self.match_path,
        }
    }
}

pub struct ThenBuilder {
    status: u16,
}

impl ThenBuilder {
    pub fn status(mut self, status: u16) -> Self {
        self.status = status;
        self
    }

    fn build(self, when_state: WhenState) -> ThenState {
        ThenState { when_state, status: self.status }
    }
}

pub struct WhenState {
    match_path: String,
}

pub struct ThenState {
    when_state: WhenState,
    status: u16,
}

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("Failed to start mock client")]
    FailedToConnectToMockServer,
    #[error("Failed to create test instance")]
    FailedToCreateTestInstance,
}
