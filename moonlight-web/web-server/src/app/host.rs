use std::fmt::{Debug, Formatter};

use common::api_bindings::{DetailedHost, HostState, PairStatus, UndetailedHost};
use log::warn;
use moonlight_common::{
    ServerState,
    high::{HostError, MoonlightHost},
    network::reqwest::ReqwestMoonlightHost,
};

use crate::app::{
    AppError, AppInner, AppRef,
    storage::StorageHost,
    user::{User, UserId},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

    async fn use_host<R>(
        &self,
        user: &mut User,
        f: impl AsyncFnOnce(&AppInner, &ReqwestMoonlightHost) -> R,
    ) -> Result<R, AppError> {
        let app = self.app.access()?;
        let key = (user.id(), self.id);

        loop {
            // Try to get host if already loaded
            let hosts = app.loaded_hosts.read().await;
            if let Some(host) = hosts.get(&key) {
                return Ok(f(&app, host).await);
            }
            drop(hosts);

            // Insert as a new host
            let mut hosts = app.loaded_hosts.write().await;

            let user_unique_id = user.host_unique_id().await?;
            let host_data = self.storage_host(&app).await?;

            let new_host =
                MoonlightHost::new(host_data.address, host_data.http_port, Some(user_unique_id))?;

            hosts.entry(key).or_insert(new_host);
            drop(hosts);
        }
    }

    async fn storage_host(&self, app: &AppInner) -> Result<StorageHost, AppError> {
        app.storage.get_host(self.id).await
    }

    pub async fn undetailed_host(&self, user: &mut User) -> Result<UndetailedHost, AppError> {
        self.use_host(user, async |app, host| {
            let name = match host.host_name().await {
                Ok(value) => value,
                Err(HostError::LikelyOffline) => {
                    let storage_host = self.storage_host(app).await?;
                    storage_host.cache_name.clone()
                }
                Err(err) => {
                    warn!("Failed to query host info for host {self:?}: {err:?}");

                    let storage_host = self.storage_host(app).await?;
                    storage_host.cache_name.clone()
                }
            };
            let paired = match host.verify_paired().await {
                Ok(value) => PairStatus::from(value),
                Err(HostError::LikelyOffline) => {
                    if host.pair_info().await.is_some() {
                        PairStatus::Paired
                    } else {
                        PairStatus::NotPaired
                    }
                }
                Err(err) => {
                    warn!("Failed to query if host is paired for host {self:?}: {err:?}");
                    if host.pair_info().await.is_some() {
                        PairStatus::Paired
                    } else {
                        PairStatus::NotPaired
                    }
                }
            };
            let server_state = match host.state().await {
                Ok((_, state)) => Some(HostState::from(state)),
                Err(HostError::LikelyOffline) => None,
                Err(err) => {
                    warn!("Failed to query server state for host {self:?}: {err:?}");
                    None
                }
            };

            Ok(UndetailedHost {
                host_id: self.id.0,
                name,
                paired,
                server_state,
            })
        })
        .await?
    }
    pub async fn detailed_host(&self, user: &mut User) -> Result<DetailedHost, AppError> {
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
