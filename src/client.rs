
use thiserror::Error;

use crate::interchange::{Command, InstanceId, Mock, ThenState, WhenState};

pub struct Client {
    control_plane_url: String,
    instance: InstanceId,
    mock_url: String,
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
            control_plane_url: String::from(control_plane_url),
            instance: response.instance,
            mock_url: response.url,
        })
    }

    pub fn when<F>(&self, when: F) -> MockBuilder<WhenState>
    where
        F: FnOnce(WhenBuilder) -> WhenBuilder,
    {
        MockBuilder {
            state: when(WhenBuilder::default()).build(),
            client: self,
        }
    }

    pub fn url(&self) -> String {
        self.mock_url.clone()
    }
}

pub enum Method {
    GET,
}

pub struct MockBuilder<'a, State> {
    state: State,
    client: &'a Client,
}

impl<'a> MockBuilder<'a, WhenState> {
    pub fn then<F>(self, then: F) -> MockBuilder<'a, WhenThenState>
    where
        F: FnOnce(ThenBuilder) -> ThenBuilder,
    {
        MockBuilder {
            state: then(ThenBuilder::new()).build(self.state),
            client: self.client,
        }
    }
}

impl<'a> MockBuilder<'a, WhenThenState> {
    // TODO should this return an ID to be used to delete the mock?
    pub async fn send(self) -> Result<(), ClientError> {
        let mock = Command::InstallMock {
            mock: Mock {
                when: self.state.when_state,
                then: self.state.then_state,
            },
            instance: self.client.instance.clone(),
        };
        let _body = reqwest::Client::new()
            .post(&self.client.control_plane_url)
            .body(serde_json::to_string(&mock).unwrap())
            .send()
            .await
            .map_err(|_| ClientError::FailedToConnectToMockServer)
            .and_then(|res| {
                if res.status().is_success() {
                    Ok(res)
                } else if res.status().is_client_error() {
                    Err(ClientError::InstanceNoLongerValid)
                } else {
                    Err(ClientError::FailedToCreateTestInstance)
                }
            })?
            .text()
            .await
            .map_err(|_| ClientError::FailedToConnectToMockServer)?;

        Ok(())
    }
}

#[derive(Default)]
pub struct WhenBuilder {
    match_path: String,
    form_data: Vec<(String, String)>,
}



impl WhenBuilder {
    pub fn path(mut self, path: &str) -> Self {
        self.match_path = String::from(path);
        self
    }

    pub fn form_data(mut self, name: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        self.form_data.push((
            name.as_ref().to_string(),
            value.as_ref().to_string(),
        ));
        self
    }

    fn build(self) -> WhenState {
        WhenState {
            match_path: self.match_path,
            form_data: self.form_data,
        }
    }
}

pub struct ThenBuilder {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl ThenBuilder {
    fn new() -> Self {
        Self {
            status: 0,
            headers: vec![],
            body: vec![],
        }
    }

    pub fn status(mut self, status: u16) -> Self {
        self.status = status;
        self
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((String::from(name), String::from(value)));
        self
    }

    pub fn body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = body.into();
        self
    }

    fn build(self, when_state: WhenState) -> WhenThenState {
        let then_state = ThenState {
            status: self.status,
            headers: self.headers,
            body: self.body,
        };
        WhenThenState {
            when_state,
            then_state,
        }
    }
}

pub struct WhenThenState {
    when_state: WhenState,
    then_state: ThenState,
}

#[derive(Error, Debug, PartialEq)]
pub enum ClientError {
    #[error("Failed to start mock client")]
    FailedToConnectToMockServer,
    #[error("Failed to create test instance")]
    FailedToCreateTestInstance,
    #[error("Mock instance has been replaced with a new instance")]
    InstanceNoLongerValid,
}
