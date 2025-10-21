use common::api_bindings::{DetailedHost, UndetailedHost};

use crate::app::{AppError, AppRef};

#[derive(Debug, Clone, Copy)]
pub struct HostId(pub u32);

pub struct Host {
    app: AppRef,
    id: HostId,
}

impl Host {
    pub fn id(&self) -> HostId {
        self.id
    }

    pub async fn undetailed_host(&self) -> Result<UndetailedHost, AppError> {
        todo!()
    }
    pub async fn detailed_host(&self) -> Result<DetailedHost, AppError> {
        todo!()
    }

    pub async fn pair(&self) -> Result<Host, AppError> {
        todo!()
    }
}
