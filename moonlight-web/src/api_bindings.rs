use moonlight_common::network::ServerState;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, TS, Clone, Copy)]
#[ts(export, export_to = "../web/api_bindings.d.ts")]
pub enum HostState {
    Free,
    Busy,
}

impl From<ServerState> for HostState {
    fn from(value: ServerState) -> Self {
        match value {
            ServerState::Free => Self::Free,
            ServerState::Busy => Self::Busy,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export, export_to = "../web/api_bindings.d.ts")]
pub struct GetHosts {
    pub hosts: Vec<UndetailedHost>,
}

#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export, export_to = "../web/api_bindings.d.ts")]
pub struct UndetailedHost {
    pub host_id: u32,
    pub name: String,
    pub server_state: HostState,
}

#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export, export_to = "../web/api_bindings.d.ts")]
pub struct DetailedHost {
    pub host_id: u32,
    pub name: String,
    pub server_state: HostState,
}
