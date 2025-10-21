use async_trait::async_trait;
use common::config::StorageConfig;
use moonlight_common::mac::MacAddress;
use pem::Pem;
use uuid::Uuid;

use crate::app::{
    AppError,
    host::HostId,
    storage::file::JsonStorage,
    user::{UserId, UserRole},
};

pub mod file;

pub async fn create_storage(config: StorageConfig) -> Result<Box<dyn Storage>, anyhow::Error> {
    match config {
        StorageConfig::Json { path } => {
            let storage = JsonStorage::new(path.into())?;

            Ok(Box::new(storage))
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
}
pub struct StorageHostAdd {
    pub owner: UserId,
    pub hostport: String,
    pub pair_info: Option<Pem>,
    pub cache_name: String,
    pub cache_mac: MacAddress,
}

pub struct StorageHostModify {
    pub id: HostId,
    pub owner: Option<UserId>,
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
    async fn get_user(&self, user: StorageUser) -> Result<StorageUser, AppError>;
    async fn add_user(&self, user: StorageUserAdd) -> Result<(), AppError>;
    async fn modify_user(&self, user: StorageUserModify) -> Result<(), AppError>;
    async fn remove_user(&self, user: UserId) -> Result<(), AppError>;

    async fn get_host(&self, host: HostId) -> Result<StorageHost, AppError>;
    async fn add_host(&self, host: StorageHostAdd) -> Result<(), AppError>;
    async fn modify_host(&self, host: StorageHostModify) -> Result<(), AppError>;
    async fn remove_host(&self, host_id: HostId) -> Result<(), AppError>;

    async fn list_user_hosts(&self, query: StorageQueryHosts)
    -> Result<Vec<StorageHost>, AppError>;
}
