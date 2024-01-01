pub use crate::shared_client::*;

use crate::{interchange::{Command, InstanceId, InstanceResponse, WhenRules, InstallResponse, InstallError}, network_client::{ClientNetworkError, NetworkClient}};

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
        let response = body.map_err(|err| {
            match err {
                ClientNetworkError::FailedToConnectToMockServer => ClientError::FailedToConnectToMockServer,
                _ => ClientError::FailedToCreateTestInstance,
            }
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
        NetworkClient::send::<Command, InstallResponse, InstallError>(&self.control_plane_url, &command)
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
