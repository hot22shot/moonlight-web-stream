use std::sync::Arc;

use async_trait::async_trait;
use common::config::StorageConfig;
use moonlight_common::mac::MacAddress;
use pem::Pem;

use crate::app::{
    AppError,
    auth::SessionToken,
    host::HostId,
    password::StoragePassword,
    storage::json::JsonStorage,
    user::{Role, UserId},
};

pub mod json;

pub async fn create_storage(
    config: StorageConfig,
) -> Result<Arc<dyn Storage + Send + Sync>, anyhow::Error> {
    match config {
        StorageConfig::Json { path } => {
            let storage = JsonStorage::load(path.into()).await?;

            // TODO: remove force write, this is just testing
            storage.force_write();

            Ok(storage)
        }
    }
}

// Storages:
// - If two options are in a Modify struct it means: First option = change the field, second option = should pair info exist

pub struct StorageUser {
    pub id: UserId,
    pub name: String,
    pub password: StoragePassword,
    pub role: Role,
}
pub struct StorageUserAdd {
    pub role: Role,
    pub name: String,
    pub password: StoragePassword,
}
pub struct StorageUserModify {
    pub role: Option<Role>,
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
}
pub struct StorageHostCache {
    pub name: String,
    pub mac: MacAddress,
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
    /// No duplicate names are allowed!
    async fn add_user(&self, user: StorageUserAdd) -> Result<StorageUser, AppError>;
    async fn modify_user(&self, user: StorageUserModify) -> Result<(), AppError>;
    async fn get_user(&self, user_id: UserId) -> Result<StorageUser, AppError>;
    /// The returned tuple can contain a StorageUser if the Storage thinks it's more efficient to query all data of the user directly
    async fn get_user_by_name(&self, name: &str)
    -> Result<(UserId, Option<StorageUser>), AppError>;
    async fn remove_user(&self, user_id: UserId) -> Result<(), AppError>;

    // TODO: maybe expiration date?
    async fn create_session_token(&self, user_id: UserId) -> Result<SessionToken, AppError>;
    async fn remove_session_token(&self, session: SessionToken) -> Result<(), AppError>;
    async fn remove_all_user_session_tokens(&self, user_id: UserId) -> Result<(), AppError>;
    /// The returned tuple can contain a StorageUser if the Storage thinks it's more efficient to query all data of the user directly
    async fn get_user_by_session_token(
        &self,
        session: SessionToken,
    ) -> Result<(UserId, Option<StorageUser>), AppError>;

    async fn add_host(&self, host: StorageHostAdd) -> Result<StorageHost, AppError>;
    async fn modify_host(&self, host: StorageHostModify) -> Result<(), AppError>;
    async fn get_host(&self, host_id: HostId) -> Result<StorageHost, AppError>;
    async fn remove_host(&self, host_id: HostId) -> Result<(), AppError>;

    async fn list_user_hosts(&self, query: StorageQueryHosts)
    -> Result<Vec<StorageHost>, AppError>;
}
