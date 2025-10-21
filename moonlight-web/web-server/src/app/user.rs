use std::ops::Deref;

use serde::{Deserialize, Serialize};

use crate::app::{
    AppError, AppRef,
    auth::SessionToken,
    host::{Host, HostId},
    storage::StorageUserModify,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

        let session = SessionToken::new()?;

        app.storage.add_session_token(self.id(), session).await?;

        Ok(session)
    }

    pub async fn role(&self) -> Result<Role, AppError> {
        let app = self.app.access()?;

        let user = app.storage.get_user(self.id).await?;

        Ok(user.role)
    }

    pub async fn set_role(&self, role: Role) -> Result<(), AppError> {
        let app = self.app.access()?;

        app.storage
            .modify_user(StorageUserModify { role: Some(role) })
            .await;

        Ok(())
    }

    pub async fn hosts(&self) -> Result<Vec<Host>, AppError> {
        todo!()
    }

    pub async fn host(&self, host_id: HostId) -> Result<Host, AppError> {
        todo!()
    }

    pub async fn host_add(&self, address: String, http_port: u16) -> Result<Host, AppError> {
        todo!()
    }

    pub async fn host_delete(&self, host_id: HostId) -> Result<(), AppError> {
        todo!()
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
