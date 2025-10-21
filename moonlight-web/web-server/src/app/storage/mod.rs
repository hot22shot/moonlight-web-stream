use std::sync::Arc;

use async_trait::async_trait;
use common::config::StorageConfig;
use moonlight_common::mac::MacAddress;
use pem::Pem;

use crate::app::{
    AppError,
    host::HostId,
    storage::json::JsonStorage,
    user::{UserId, UserRole},
};

pub mod json;

pub async fn create_storage(
    config: StorageConfig,
) -> Result<Arc<dyn Storage + Send + Sync>, anyhow::Error> {
    match config {
        StorageConfig::Json { path } => {
            let storage = JsonStorage::load(path.into()).await?;
            storage.force_write();

            Ok(storage)
        }
    }
}

// Storages:
// - If two options are in a Modify struct it means: First option = change the field, second option = should pair info exist

pub struct StorageUser {
    pub id: UserId,
    pub role: UserRole,
}
pub struct StorageUserAdd {
    pub role: UserRole,
}
pub struct StorageUserModify {
    pub role: Option<UserRole>,
}

pub struct StorageHost {
    pub id: HostId,
    // If this is none it means the host is accessible by everyone
    pub owner: Option<UserId>,
    pub hostport: String,
    pub pair_info: Option<Pem>,
    pub cache_name: String,
    pub cache_mac: MacAddress,
}
pub struct StorageHostAdd {
    pub owner: UserId,
    pub hostport: String,
    pub pair_info: Option<StorageHostPairInfo>,
    pub cache_name: String,
    pub cache_mac: MacAddress,
}
pub struct StorageHostPairInfo {
    client_private_key: Pem,
    client_certificate: Pem,
    server_certificate: Pem,
}
pub struct StorageHostModify {
    pub id: HostId,
    pub owner: Option<Option<UserId>>,
    pub hostport: Option<String>,
    pub pair_info: Option<Option<Pem>>,
    pub cache_name: Option<String>,
    pub cache_mac: Option<MacAddress>,
}

pub struct StorageQueryHosts {
    pub id: UserId,
}

#[async_trait]
pub trait Storage {
    async fn add_user(&self, user: StorageUserAdd) -> Result<StorageUser, AppError>;
    async fn modify_user(&self, user: StorageUserModify) -> Result<(), AppError>;
    async fn get_user(&self, user_id: UserId) -> Result<StorageUser, AppError>;
    async fn remove_user(&self, user_id: UserId) -> Result<(), AppError>;

    async fn add_host(&self, host: StorageHostAdd) -> Result<StorageHost, AppError>;
    async fn modify_host(&self, host: StorageHostModify) -> Result<(), AppError>;
    async fn get_host(&self, host_id: HostId) -> Result<StorageHost, AppError>;
    async fn remove_host(&self, host_id: HostId) -> Result<(), AppError>;

    async fn list_user_hosts(&self, query: StorageQueryHosts)
    -> Result<Vec<StorageHost>, AppError>;
}
