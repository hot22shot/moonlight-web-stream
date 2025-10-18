use common::config::StorageConfig;
use uuid::Uuid;

use crate::app::{
    AppError,
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
    pub uuid: Uuid,
}
pub struct StorageHostAdd {
    pub uuid: Uuid,
    pub hostport: String,
}
pub struct StorageHostModify {}

pub struct StorageQueryHosts {
    pub id: UserId,
}

pub trait Storage {
    async fn get_user(&self, user: StorageUser) -> Result<StorageUser, AppError>;
    async fn add_user(&self, user: StorageUserAdd) -> Result<(), AppError>;
    async fn modify_user(&self, user: StorageUserModify) -> Result<(), AppError>;
    async fn remove_user(&self, user: UserId) -> Result<(), AppError>;

    async fn get_host(&self, host: StorageHost) -> Result<StorageHost, AppError>;
    async fn add_host(&self, host: StorageHostAdd) -> Result<(), AppError>;
    async fn modify_host(&self, host: StorageHostModify) -> Result<(), AppError>;
    async fn remove_host(&self, host_id: Uuid) -> Result<(), AppError>;

    async fn list_user_hosts(&self, query: StorageQueryHosts) -> Result<(), AppError>;
}
