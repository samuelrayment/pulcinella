use serde_derive::{Serialize, Deserialize};

#[derive(Deserialize, Serialize, Debug)]
pub enum Command {
    CreateInstance,
    InstallMock {
        instance: InstanceId,
        mock: Mock,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InstanceResponse {
    pub instance: InstanceId,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InstanceId(pub(crate) String);

#[derive(Serialize, Deserialize, Debug)]
pub struct Mock{}
