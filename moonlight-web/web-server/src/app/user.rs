use serde::{Deserialize, Serialize};

use crate::app::{
    AppError, AppRef,
    host::{Host, HostId},
    storage::StorageUserModify,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum UserRole {
    Admin,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserId(pub u32);

pub struct User {
    pub(super) app: AppRef,
    pub(super) id: UserId,
}

impl User {
    pub fn id(&self) -> UserId {
        self.id
    }

    pub async fn role(&self) -> Result<UserRole, AppError> {
        let app = self.app.access()?;

        let user = app.storage.get_user(self.id).await?;

        Ok(user.role)
    }

    pub async fn set_role(&self, role: UserRole) -> Result<(), AppError> {
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
