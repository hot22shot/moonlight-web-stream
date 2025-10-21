use std::{collections::HashMap, io::ErrorKind, path::PathBuf, sync::Arc};

use anyhow::anyhow;
use async_trait::async_trait;
use futures::future::join_all;
use log::error;
use openssl::rand::rand_bytes;
use tokio::{
    fs, spawn,
    sync::{
        RwLock,
        mpsc::{self, Receiver, Sender, error::TrySendError},
    },
};

use crate::app::{
    AppError,
    auth::SessionToken,
    host::HostId,
    password::StoragePassword,
    storage::{
        Storage, StorageHost, StorageHostAdd, StorageHostModify, StorageQueryHosts, StorageUser,
        StorageUserAdd, StorageUserModify,
        json::versions::{Json, V2, V2Host, V2User, V2UserPassword, migrate_to_latest},
    },
    user::UserId,
};

mod serde_helpers;
mod versions;

pub struct JsonStorage {
    file: PathBuf,
    store_sender: Sender<()>,
    users: RwLock<HashMap<u32, RwLock<V2User>>>,
    hosts: RwLock<HashMap<u32, RwLock<V2Host>>>,
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

    pub fn force_write(&self) {
        if let Err(TrySendError::Closed(_)) = self.store_sender.try_send(()) {
            error!("Failed to save data because the writer task closed!");
        }
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

        let data = migrate_to_latest(json)?;

        {
            let mut users = self.users.write().await;
            let mut hosts = self.hosts.write().await;

            *users = data
                .users
                .into_iter()
                .map(|(id, user)| (id, RwLock::new(user)))
                .collect();
            *hosts = data
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

            Json::V2(V2 {
                users: users_json,
                hosts: hosts_json,
            })
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

fn user_into_json(user: StorageUser) -> V2User {
    V2User {
        role: user.role,
        name: user.name,
        password: V2UserPassword {
            salt: user.password.salt,
            hash: user.password.hash,
        },
    }
}
fn user_from_json(user_id: UserId, user: &V2User) -> StorageUser {
    StorageUser {
        id: user_id,
        name: user.name.clone(),
        password: StoragePassword {
            salt: user.password.salt,
            hash: user.password.hash,
        },
        role: user.role,
    }
}

#[async_trait]
impl Storage for JsonStorage {
    async fn add_user(&self, user: StorageUserAdd) -> Result<StorageUser, AppError> {
        let user = V2User {
            role: user.role,
            name: user.name,
            password: V2UserPassword {
                salt: user.password.salt,
                hash: user.password.hash,
            },
        };

        // TODO: check for duplicate names

        let mut id = [0u8; 4];
        rand_bytes(&mut id)?;
        let id = u32::from_be_bytes(id);

        let mut users = self.users.write().await;

        users.insert(id, RwLock::new(user.clone()));

        drop(users);

        self.force_write();

        Ok(StorageUser {
            id: UserId(id),
            name: user.name,
            password: StoragePassword {
                salt: user.password.salt,
                hash: user.password.hash,
            },
            role: user.role,
        })
    }
    async fn modify_user(&self, user: StorageUserModify) -> Result<(), AppError> {
        self.force_write();
        todo!()
    }
    async fn get_user(&self, user_id: UserId) -> Result<StorageUser, AppError> {
        let users = self.users.read().await;

        let user_lock = users.get(&user_id.0).ok_or(AppError::UserNotFound)?;
        let user = user_lock.read().await;

        Ok(user_from_json(user_id, &user))
    }
    async fn get_user_by_name(
        &self,
        name: &str,
    ) -> Result<(UserId, Option<StorageUser>), AppError> {
        let users = self.users.read().await;

        let results = join_all(users.iter().map(|(user_id, user)| async move {
            let user = user.read().await;

            let user_id = UserId(*user_id);
            let user = (user.name == name).then(|| user_from_json(user_id, &user));

            (user_id, user)
        }))
        .await;

        let user = results.into_iter().find(|(_, user)| user.is_some());

        user.ok_or(AppError::UserNotFound)
    }
    async fn remove_user(&self, user_id: UserId) -> Result<(), AppError> {
        self.force_write();
        todo!()
    }

    async fn add_session_token(
        &self,
        user_id: UserId,
        session: SessionToken,
    ) -> Result<(), AppError> {
        todo!()
    }
    async fn remove_session_token(&self, session: SessionToken) -> Result<(), AppError> {
        todo!()
    }
    async fn remove_all_user_session_tokens(&self, user_id: UserId) -> Result<(), AppError> {
        todo!()
    }
    async fn get_user_by_session_token(
        &self,
        session: SessionToken,
    ) -> Result<(UserId, Option<StorageUser>), AppError> {
        todo!()
    }

    async fn add_host(&self, host: StorageHostAdd) -> Result<StorageHost, AppError> {
        self.force_write();
        todo!()
    }
    async fn modify_host(&self, host: StorageHostModify) -> Result<(), AppError> {
        self.force_write();
        todo!()
    }
    async fn get_host(&self, host_id: HostId) -> Result<StorageHost, AppError> {
        todo!()
    }
    async fn remove_host(&self, host_id: HostId) -> Result<(), AppError> {
        self.force_write();
        todo!()
    }

    async fn list_user_hosts(
        &self,
        query: StorageQueryHosts,
    ) -> Result<Vec<StorageHost>, AppError> {
        todo!()
    }
}
