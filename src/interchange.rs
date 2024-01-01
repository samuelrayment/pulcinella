use serde_derive::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub enum Command {
    CreateInstance,
    InstallMock {
        instance: InstanceId,
        mock: MockRule,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InstanceResponse {
    pub instance: InstanceId,
    pub url: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InstallResponse;

#[derive(Serialize, Deserialize, Debug)]
pub enum InstallError {
    InstanceNotFound,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct InstanceId(pub(crate) String);

#[derive(Serialize, Deserialize, Debug)]
pub struct MockRule {
    pub when: WhenRules,
    pub then: ThenState,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WhenRules {
    pub match_path: String,
    pub form_data: Vec<(String, String)>,
    pub method: Option<Method>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ThenState {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Method {
    GET,
    POST,
    DELETE,
    PUT,
}
