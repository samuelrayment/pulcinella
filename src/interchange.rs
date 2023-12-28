use serde_derive::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub enum Command {
    CreateInstance,
    InstallMock { instance: InstanceId, mock: Mock },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InstanceResponse {
    pub instance: InstanceId,
    pub url: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InstanceId(pub(crate) String);

#[derive(Serialize, Deserialize, Debug)]
pub struct Mock {
    pub when: WhenState,
    pub then: ThenState,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WhenState {
    pub match_path: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ThenState {
    pub status: u16,
}
