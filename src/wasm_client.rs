pub use crate::shared_client::*;
use gloo_net::http::Request;

use crate::interchange::{Command, InstanceId, InstanceResponse, WhenRules, InstallResponse};

pub struct Client {
    control_plane_url: String,
    instance: InstanceId,
    mock_url: String,
}

impl Client {
    pub async fn new(control_plane_url: &str) -> Result<Self, ClientError> {
        let response: InstanceResponse = Request::post(control_plane_url)
            .json(&Command::CreateInstance)
            .map_err(|_| ClientError::FailedToConnectToMockServer)?
            .send()
            .await
            .map_err(|_| ClientError::FailedToConnectToMockServer)?
            .json()
            .await
            .map_err(|_| ClientError::FailedToConnectToMockServer)?;

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
        let url = self.control_plane_url.clone();
        let _body: InstallResponse = Request::post(&url)
            .json(&command)
            .map_err(|_| ClientError::FailedToConnectToMockServer)?
            .send()
            .await
            .map_err(|_| ClientError::FailedToConnectToMockServer)?
            .json()
            .await
            .map_err(|_| ClientError::FailedToConnectToMockServer)?;
        Ok(())
    }

    fn instance(&self) -> &InstanceId {
        &self.instance
    }
}
