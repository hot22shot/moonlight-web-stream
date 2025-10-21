use std::{collections::HashMap, io::ErrorKind, path::PathBuf, sync::Arc};

use anyhow::anyhow;
use async_trait::async_trait;
use log::error;
use moonlight_common::mac::MacAddress;
use serde::{Deserialize, Serialize};
use tokio::{
    fs, spawn,
    sync::{
        RwLock,
        mpsc::{self, Receiver, Sender},
    },
};

use crate::app::{
    AppError,
    host::HostId,
    storage::{
        Storage, StorageHost, StorageHostAdd, StorageHostModify, StorageQueryHosts, StorageUser,
        StorageUserAdd, StorageUserModify,
    },
    user::{UserId, UserRole},
};

pub struct JsonStorage {
    file: PathBuf,
    store_sender: Sender<()>,
    users: RwLock<HashMap<u32, RwLock<JsonUser>>>,
    hosts: RwLock<HashMap<u32, RwLock<JsonHost>>>,
}

#[derive(Serialize, Deserialize)]
struct Json {
    users: HashMap<u32, JsonUser>,
    hosts: HashMap<u32, JsonHost>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonUser {
    pub role: UserRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonHost {
    pub owner: u32,
    pub hostport: String,
    pub pair_info: Option<String>,
    pub cache_name: String,
    pub cache_mac: MacAddress,
}

impl JsonStorage {
    pub async fn load(file: PathBuf) -> Result<Arc<Self>, anyhow::Error> {
        let (store_sender, store_receiver) = mpsc::channel(1);

        let this = Self {
            file,
            store_sender,
            hosts: Default::default(),
            users: Default::default(),
        };

        this.load_internal().await?;

        let this = Arc::new(this);
        spawn({
            let this = this.clone();

            async move { file_writer(store_receiver, this).await }
        });

        Ok(this)
    }

    async fn load_internal(&self) -> Result<(), anyhow::Error> {
        let text = match fs::read_to_string(&self.file).await {
            Ok(text) => text,
            Err(err) if err.kind() == ErrorKind::NotFound => {
                return Ok(());
            }
            Err(err) => {
                return Err(anyhow!("Failed to read data: {err:?}"));
            }
        };

        let json = match serde_json::from_str::<Json>(&text) {
            Ok(value) => value,
            Err(err) => {
                return Err(anyhow!("Failed to deserialize data as json: {err:?}"));
            }
        };

        {
            let mut users = self.users.write().await;
            let mut hosts = self.hosts.write().await;

            *users = json
                .users
                .into_iter()
                .map(|(id, host)| (id, RwLock::new(host)))
                .collect();
            *hosts = json
                .hosts
                .into_iter()
                .map(|(id, host)| (id, RwLock::new(host)))
                .collect();
        }

        Ok(())
    }
    async fn store(&self) {
        let json = {
            let users = self.users.read().await;
            let hosts = self.hosts.read().await;

            let mut users_json = HashMap::new();
            for (key, value) in users.iter() {
                let value = value.read().await;

                users_json.insert(*key, (*value).clone());
            }

            let mut hosts_json = HashMap::new();
            for (key, value) in hosts.iter() {
                let value = value.read().await;

                hosts_json.insert(*key, (*value).clone());
            }

            Json {
                users: users_json,
                hosts: hosts_json,
            }
        };

        let text = match serde_json::to_string_pretty(&json) {
            Ok(text) => text,
            Err(err) => {
                error!("Failed to serialize data to json: {err:?}");
                return;
            }
        };

        if let Err(err) = fs::write(&self.file, text).await {
            error!("Failed to write data to file: {err:?}");
        }
    }
}

async fn file_writer(mut store_receiver: Receiver<()>, json: Arc<JsonStorage>) {
    loop {
        if store_receiver.recv().await.is_none() {
            return;
        }

        json.store().await;
    }
}

#[async_trait]
impl Storage for JsonStorage {
    async fn add_user(&self, user: StorageUserAdd) -> Result<StorageUser, AppError> {
        todo!()
    }
    async fn modify_user(&self, user: StorageUserModify) -> Result<(), AppError> {
        todo!()
    }
    async fn get_user(&self, user_id: UserId) -> Result<StorageUser, AppError> {
        todo!()
    }
    async fn remove_user(&self, user_id: UserId) -> Result<(), AppError> {
        todo!()
    }

    async fn add_host(&self, host: StorageHostAdd) -> Result<StorageHost, AppError> {
        todo!()
    }
    async fn modify_host(&self, host: StorageHostModify) -> Result<(), AppError> {
        todo!()
    }
    async fn get_host(&self, host_id: HostId) -> Result<StorageHost, AppError> {
        todo!()
    }
    async fn remove_host(&self, host_id: HostId) -> Result<(), AppError> {
        todo!()
    }

    async fn list_user_hosts(
        &self,
        query: StorageQueryHosts,
    ) -> Result<Vec<StorageHost>, AppError> {
        todo!()
    }
}
