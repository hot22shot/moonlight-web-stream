use crate::host::network::ApiError;

pub mod network;
pub mod pair;

pub struct MoonlightClient {}

impl MoonlightClient {
    pub async fn pair(&mut self) -> Result<(), ApiError> {
        todo!()
    }
}
