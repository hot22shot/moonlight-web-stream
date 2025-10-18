use std::path::{Path, PathBuf};

use crate::app::{AppError, storage::Storage};

pub struct JsonStorage {
    file: PathBuf,
}

impl JsonStorage {
    pub fn new(file: PathBuf) -> Result<Self, anyhow::Error> {
        Ok(Self { file })
    }
}

impl Storage for JsonStorage {}
