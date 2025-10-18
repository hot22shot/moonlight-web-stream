use uuid::Uuid;

use crate::app::{AppError, AppRef};

pub struct Host {
    app: AppRef,
    host_uuid: Uuid,
}

impl Host {
    pub fn id(&self) -> Uuid {
        self.host_uuid
    }

    pub async fn pair(&self, uuid: Uuid) -> Result<Host, AppError> {
        todo!()
    }
}
