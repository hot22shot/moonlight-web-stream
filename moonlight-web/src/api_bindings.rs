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
pub enum PairStatus {
    NotPaired,
    Paired,
}

impl From<moonlight_common::network::PairStatus> for PairStatus {
    fn from(value: moonlight_common::network::PairStatus) -> Self {
        use moonlight_common::network::PairStatus as MlPairStatus;
        match value {
            MlPairStatus::NotPaired => Self::NotPaired,
            MlPairStatus::Paired => Self::Paired,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export, export_to = "../web/api_bindings.d.ts")]
pub struct UndetailedHost {
    pub host_id: u32,
    pub name: String,
    pub paired: PairStatus,
    pub server_state: HostState,
}

#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export, export_to = "../web/api_bindings.d.ts")]
pub struct DetailedHost {
    pub host_id: u32,
    pub name: String,
    pub paired: PairStatus,
    pub server_state: HostState,
    pub address: String,
    pub http_port: u16,
    pub https_port: u16,
    pub external_port: u16,
    pub version: String, // TODO: server version struct?
    pub gfe_version: String,
    pub unique_id: String,
    pub mac: String,
    pub local_ip: String,
    pub current_game: u32,
    pub max_luma_pixels_hevc: u32,
    pub server_codec_mode_support: u32,
}

#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export, export_to = "../web/api_bindings.d.ts")]
pub struct App {
    pub app_id: u32,
    pub title: String,
    pub is_hdr_supported: bool,
}

impl From<moonlight_common::network::App> for App {
    fn from(value: moonlight_common::network::App) -> Self {
        Self {
            app_id: value.id,
            title: value.title,
            is_hdr_supported: value.is_hdr_supported,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export, export_to = "../web/api_bindings.d.ts")]
pub struct GetHostsResponse {
    pub hosts: Vec<UndetailedHost>,
}

#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export, export_to = "../web/api_bindings.d.ts")]
pub struct GetHostQuery {
    pub host_id: u32,
}

#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export, export_to = "../web/api_bindings.d.ts")]
pub struct GetHostResponse {
    pub host: DetailedHost,
}

#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export, export_to = "../web/api_bindings.d.ts")]
pub struct PutHostRequest {
    pub address: String,
    pub http_port: Option<u16>,
}

#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export, export_to = "../web/api_bindings.d.ts")]
pub struct PutHostResponse {
    pub host: DetailedHost,
}

#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export, export_to = "../web/api_bindings.d.ts")]
pub struct DeleteHostQuery {
    pub host_id: u32,
}

#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export, export_to = "../web/api_bindings.d.ts")]
pub struct PostPairRequest {
    pub host_id: u32,
}

#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export, export_to = "../web/api_bindings.d.ts")]
pub enum PostPairResponse1 {
    InternalServerError,
    PairError,
    Pin(String),
}

#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export, export_to = "../web/api_bindings.d.ts")]
pub enum PostPairResponse2 {
    PairError,
    Paired(DetailedHost),
}

#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export, export_to = "../web/api_bindings.d.ts")]
pub struct GetAppsQuery {
    pub host_id: u32,
}

#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export, export_to = "../web/api_bindings.d.ts")]
pub struct GetAppsResponse {
    pub apps: Vec<App>,
}
