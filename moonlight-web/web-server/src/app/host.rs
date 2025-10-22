use std::fmt::{Debug, Formatter};

use common::api_bindings::{DetailedHost, UndetailedHost};

use crate::app::{AppError, AppRef, user::UserId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostId(pub u32);

pub struct Host {
    pub(super) app: AppRef,
    pub(super) id: HostId,
}

impl Debug for Host {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.id)
    }
}

impl Host {
    pub fn id(&self) -> HostId {
        self.id
    }

    pub async fn owner(&self) -> Result<Option<UserId>, AppError> {
        let app = self.app.access()?;

        let host = app.storage.get_host(self.id).await?;

        Ok(host.owner)
    }

    pub async fn undetailed_host(&self) -> Result<UndetailedHost, AppError> {
        todo!()
    }
    pub async fn detailed_host(&self) -> Result<DetailedHost, AppError> {
        todo!()
    }

    pub async fn pair(&self, user_id: UserId) -> Result<Host, AppError> {
        todo!()
    }

    pub async fn unpair(&self, user_id: UserId) -> Result<Host, AppError> {
        todo!()
    }

    pub async fn wake(&self) -> Result<(), AppError> {
        todo!()
    }

    pub async fn delete(&self, user_id: UserId) -> Result<(), AppError> {
        let app = self.app.access()?;

        let host = app.storage.get_host(self.id).await?;

        if matches!(host.owner, Some(user_id)) {
            self.delete_no_auth().await
        } else {
            Err(AppError::Forbidden)
        }
    }
    pub async fn delete_no_auth(&self) -> Result<(), AppError> {
        todo!()
    }
}
