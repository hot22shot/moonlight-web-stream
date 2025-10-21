use common::api_bindings::UndetailedHost;
use uuid::Uuid;

use crate::app::{AppError, AppRef, host::Host};

pub enum UserRole {
    Admin,
    User,
}

#[derive(Debug, Clone, Copy)]
pub struct UserId(pub u32);

pub struct User {
    app: AppRef,
    id: UserId,
}

impl User {
    pub fn id(&self) -> UserId {
        self.id
    }

    pub async fn role(&self) -> Result<UserRole, AppError> {
        todo!()
    }

    pub async fn set_role(&self, role: UserRole) -> Result<(), AppError> {
        todo!()
    }

    pub async fn hosts(&self) -> Result<Vec<UndetailedHost>, AppError> {
        todo!()
    }

    pub async fn host(&self, uuid: Uuid) -> Result<Host, AppError> {
        todo!()
    }

    pub async fn host_add(&self, uuid: Uuid) -> Result<Host, AppError> {
        todo!()
    }

    pub async fn host_remove(&self, uuid: Uuid) -> Result<(), AppError> {
        todo!()
    }

    pub async fn delete(self) -> Result<(), AppError> {
        todo!()
    }
}
