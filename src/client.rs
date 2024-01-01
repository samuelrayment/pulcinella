use std::future::Future;

use thiserror::Error;

use crate::{
    interchange::{
        Command, InstallError, InstallResponse, InstanceId, InstanceResponse, WhenRules, MockRule, ThenState,
    },
    network_client::{ClientNetworkError, NetworkClient},
};
pub use crate::interchange::Method;

pub struct Client {
    control_plane_url: String,
    instance: InstanceId,
    mock_url: String,
}

impl Client {
    pub async fn new(control_plane_url: &str) -> Result<Self, ClientError> {
        let body = NetworkClient::send::<Command, InstanceResponse, InstanceResponse>(
            control_plane_url,
            &Command::CreateInstance,
        )
        .await;
        let response = body.map_err(|err| match err {
            ClientNetworkError::FailedToConnectToMockServer => {
                ClientError::FailedToConnectToMockServer
            }
            _ => ClientError::FailedToCreateTestInstance,
        })?;

        Ok(Self {
            control_plane_url: String::from(control_plane_url),
            instance: response.instance,
            mock_url: response.url,
        })
    }

    pub fn when<F>(&self, when: F) -> MockBuilder<WhenRules, Client>
    where
        F: FnOnce(WhenBuilder) -> WhenBuilder,
    {
        MockBuilder::new(self, when(WhenBuilder::default()).build())
    }

    pub fn url(&self) -> String {
        self.mock_url.clone()
    }
}

impl MockClient for Client {
    async fn send_command(&self, command: Command) -> Result<(), ClientError> {
        NetworkClient::send::<Command, InstallResponse, InstallError>(
            &self.control_plane_url,
            &command,
        )
        .await
        .map(|_| ())
        .map_err(|e| match e {
            ClientNetworkError::Response(InstallError::InstanceNotFound) => {
                ClientError::InstanceNoLongerValid
            }
            _ => ClientError::FailedToConnectToMockServer,
        })
    }

    fn instance(&self) -> &InstanceId {
        &self.instance
    }
}

pub trait MockClient {
    fn send_command(&self, command: Command) -> impl Future<Output = Result<(), ClientError>>;
    fn instance(&self) -> &InstanceId;
}

pub struct MockBuilder<'a, State, C: MockClient> {
    state: State,
    client: &'a C,
}

impl<'a, C: MockClient> MockBuilder<'a, WhenRules, C> {
    pub fn new(client: &'a C, when_rules: WhenRules) -> MockBuilder<'a, WhenRules, C> {
        MockBuilder {
            state: when_rules,
            client,
        }
    }
}

impl<'a, C: MockClient> MockBuilder<'a, WhenRules, C> {
    pub fn then<F>(self, then: F) -> MockBuilder<'a, WhenThenState, C>
    where
        F: FnOnce(ThenBuilder) -> ThenBuilder,
    {
        MockBuilder {
            state: then(ThenBuilder::new()).build(self.state),
            client: self.client,
        }
    }
}

impl<'a, C: MockClient> MockBuilder<'a, WhenThenState, C> {
    // TODO should this return an ID to be used to delete the mock?
    pub async fn send(self) -> Result<(), ClientError> {
        let mock = Command::InstallMock {
            mock: MockRule {
                when: self.state.when_rules,
                then: self.state.then_state,
            },
            instance: self.client.instance().clone(),
        };
        self.client.send_command(mock).await
    }
}

#[derive(Default)]
pub struct WhenBuilder {
    method: Option<Method>,
    match_path: String,
    form_data: Vec<(String, String)>,
}

impl WhenBuilder {
    pub fn path(mut self, path: &str) -> Self {
        self.match_path = String::from(path);
        self
    }

    pub fn form_data(mut self, name: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        self.form_data
            .push((name.as_ref().to_string(), value.as_ref().to_string()));
        self
    }

    pub fn method(mut self, method: Method) -> Self {
        self.method = Some(method);
        self
    }

    pub(crate) fn build(self) -> WhenRules {
        WhenRules {
            match_path: self.match_path,
            form_data: self.form_data,
            method: self.method,
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

    fn build(self, when_rules: WhenRules) -> WhenThenState {
        let then_state = ThenState {
            status: self.status,
            headers: self.headers,
            body: self.body,
        };
        WhenThenState {
            when_rules,
            then_state,
        }
    }
}

pub struct WhenThenState {
    when_rules: WhenRules,
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
    #[error("Failed to install mock rule into server")]
    FailedToInstallMockRule,
}
