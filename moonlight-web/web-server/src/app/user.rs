use std::ops::Deref;

use moonlight_common::network::{ApiError, ClientInfo, host_info, request_client::RequestClient};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app::{
    AppError, AppRef, MoonlightClient,
    auth::SessionToken,
    host::{Host, HostId},
    storage::{
        StorageHostAdd, StorageHostCache, StorageQueryHosts, StorageUser, StorageUserModify,
    },
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Role {
    User,
    Admin,
}

impl From<common::api_bindings::UserRole> for Role {
    fn from(value: common::api_bindings::UserRole) -> Self {
        use common::api_bindings::UserRole;

        match value {
            UserRole::User => Self::User,
            UserRole::Admin => Self::Admin,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(pub u32);

// TODO: maybe cache?
pub struct User {
    pub(super) app: AppRef,
    pub(super) id: UserId,
}

impl User {
    pub fn id(&self) -> UserId {
        self.id
    }

    pub async fn verify_password(&self, password: &str) -> Result<bool, AppError> {
        let app = self.app.access()?;

        let user = app.storage.get_user(self.id).await?;

        user.password.verify(password)
    }

    pub async fn new_session(&self) -> Result<SessionToken, AppError> {
        let app = self.app.access()?;

        let token = app.storage.create_session_token(self.id()).await?;

        Ok(token)
    }

    // TODO: how to authenticate this?
    pub async fn modify(&self, user_id: UserId, modify: StorageUserModify) -> Result<(), AppError> {
        let app = self.app.access()?;

        // TODO: clear all hosts from the loaded hosts if unique id changed

        app.storage.modify_user(user_id, modify).await?;

        Ok(())
    }

    async fn storage_user(&self) -> Result<StorageUser, AppError> {
        let app = self.app.access()?;

        let user = app.storage.get_user(self.id).await?;

        Ok(user)
    }
    pub async fn role(&self) -> Result<Role, AppError> {
        let user = self.storage_user().await?;

        Ok(user.role)
    }

    pub async fn host_unique_id(&self) -> Result<String, AppError> {
        let user = self.storage_user().await?;

        // TODO: have an override for user
        Ok(user.name)
    }

    pub async fn hosts(&self) -> Result<Vec<Host>, AppError> {
        let app = self.app.access()?;

        let hosts = app
            .storage
            .list_user_hosts(StorageQueryHosts { user_id: self.id })
            .await?
            .into_iter()
            .map(|(host_id, host)| Host {
                // TODO: use storage host
                app: self.app.clone(),
                id: host_id,
            })
            .collect();

        Ok(hosts)
    }

    pub async fn host(&self, host_id: HostId) -> Result<Host, AppError> {
        let app = self.app.access()?;

        let host = app.storage.get_host(host_id).await?;

        if host.owner.is_none() || host.owner == Some(self.id) {
            Ok(Host {
                app: self.app.clone(),
                id: host.id,
            })
        } else {
            Err(AppError::Forbidden)
        }
    }

    pub async fn host_add(&self, address: String, http_port: u16) -> Result<Host, AppError> {
        let app = self.app.access()?;

        let unique_id = self.host_unique_id().await?;

        let mut client = MoonlightClient::with_defaults().map_err(ApiError::RequestClient)?;
        let info = host_info(
            &mut client,
            false,
            &format!("{}:{}", address, http_port),
            Some(ClientInfo {
                uuid: Uuid::new_v4(),
                unique_id: &unique_id,
            }),
        )
        .await?;

        let host = app
            .storage
            .add_host(StorageHostAdd {
                owner: Some(self.id),
                address,
                http_port,
                pair_info: None,
                cache: StorageHostCache {
                    name: info.host_name,
                    mac: info.mac,
                },
            })
            .await?;

        Ok(Host {
            // TODO: use storage_host
            app: self.app.clone(),
            id: host.id,
        })
    }

    pub async fn host_delete(&mut self, host_id: HostId) -> Result<(), AppError> {
        let host = self.host(host_id).await?;

        host.delete(self).await?;

        Ok(())
    }

    pub async fn delete(self) -> Result<(), AppError> {
        todo!()
    }
}

pub struct Admin(User);

impl Admin {
    pub async fn try_from(user: User) -> Result<Self, AppError> {
        match user.role().await? {
            Role::Admin => Ok(Self(user)),
            _ => Err(AppError::Forbidden),
        }
    }
}

impl Deref for Admin {
    type Target = User;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
