use crate::interchange::{Command, InstanceId, WhenRules};
pub use crate::shared_client::*;

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
        let _body = reqwest::Client::new()
            .post(&self.control_plane_url)
            .body(serde_json::to_string(&command).unwrap())
            .send()
            .await
            .map_err(|_| ClientError::FailedToConnectToMockServer)
            .and_then(|res| {
                if res.status().is_success() {
                    Ok(res)
                } else if res.status().is_client_error() {
                    Err(ClientError::InstanceNoLongerValid)
                } else {
                    Err(ClientError::FailedToInstallMockRule)
                }
            })?
            .text()
            .await
            .map_err(|_| ClientError::FailedToConnectToMockServer)?;
        Ok(())
    }

    fn instance(&self) -> &InstanceId {
        &self.instance
    }
}
