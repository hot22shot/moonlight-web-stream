use std::path::PathBuf;

use async_trait::async_trait;

use crate::app::storage::Storage;

pub struct JsonStorage {
    file: PathBuf,
}

impl JsonStorage {
    pub fn new(file: PathBuf) -> Result<Self, anyhow::Error> {
        Ok(Self { file })
    }
}

#[async_trait]
impl Storage for JsonStorage {}
