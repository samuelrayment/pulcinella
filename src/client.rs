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
            state: then(ThenBuilder { status: 200 }).build(self.state),
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
        let body = reqwest::Client::new()
            .post(&self.client.control_plane_url)
            .body(serde_json::to_string(&mock).unwrap())
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

        Ok(())
    }
}

pub struct WhenBuilder {
    match_path: String,
    form_data: Option<Vec<(String, String)>>,
}

impl Default for WhenBuilder {
    fn default() -> Self {
        Self {
            match_path: String::from(""),
            form_data: None,
        }
    }
}

impl WhenBuilder {
    pub fn path(mut self, path: &str) -> Self {
        self.match_path = String::from(path);
        self
    }

    pub fn form_data(mut self, form_data: &[(String, String)]) -> Self {
        self.form_data = Some(form_data.to_vec());
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
}

impl ThenBuilder {
    pub fn status(mut self, status: u16) -> Self {
        self.status = status;
        self
    }

    fn build(self, when_state: WhenState) -> WhenThenState {
        let then_state = ThenState {
            status: self.status,
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

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("Failed to start mock client")]
    FailedToConnectToMockServer,
    #[error("Failed to create test instance")]
    FailedToCreateTestInstance,
}
