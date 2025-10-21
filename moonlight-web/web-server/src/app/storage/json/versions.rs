use std::collections::HashMap;

use moonlight_common::mac::MacAddress;
use serde::{Deserialize, Serialize};

use crate::app::user::UserRole;

#[derive(Serialize, Deserialize)]
#[serde(tag = "version")]
pub enum Json {
    #[serde(rename = "2")]
    V2(V2),
    #[serde(untagged)]
    V1(V1),
}

// -- V1

#[derive(Serialize, Deserialize)]
pub struct V1 {
    hosts: Vec<V1Host>,
}

#[derive(Serialize, Deserialize)]
pub struct V1Host {
    address: String,
    http_port: u16,
    #[serde(default)]
    cache: V1HostCache,
    paired: Option<V1HostPairInfo>,
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct V1HostCache {
    pub name: Option<String>,
    pub mac: Option<MacAddress>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V1HostPairInfo {
    pub client_private_key: String,
    pub client_certificate: String,
    pub server_certificate: String,
}

pub fn migrate_v1_to_v2(old: V1) -> V2 {
    let mut v2_hosts = HashMap::new();

    for (id, old_host) in old.hosts.into_iter().enumerate() {
        let v2_host = V2Host {
            owner: None,
            address: old_host.address,
            http_port: old_host.http_port,
            pair_info: old_host.paired,
            cache_name: old_host.cache.name.unwrap_or_else(|| "Unknown".to_string()),
            cache_mac: old_host
                .cache
                .mac
                .unwrap_or_else(|| MacAddress::from_bytes([0; 6])),
        };

        v2_hosts.insert(id as u32, v2_host);
    }

    V2 {
        users: Default::default(),
        hosts: v2_hosts,
    }
}

// -- V2

use crate::app::storage::json::serde_hashmap_fix::de_int_key;

#[derive(Serialize, Deserialize)]
pub struct V2 {
    #[serde(deserialize_with = "de_int_key")]
    pub users: HashMap<u32, V2User>,
    #[serde(deserialize_with = "de_int_key")]
    pub hosts: HashMap<u32, V2Host>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V2User {
    pub role: UserRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V2Host {
    pub owner: Option<u32>,
    pub address: String,
    pub http_port: u16,
    pub pair_info: Option<V1HostPairInfo>,
    pub cache_name: String,
    pub cache_mac: MacAddress,
}

pub fn migrate_to_latest(json: Json) -> Result<V2, anyhow::Error> {
    match json {
        Json::V1(v1) => Ok(migrate_v1_to_v2(v1)),
        Json::V2(v2) => Ok(v2),
    }
}
