use std::fmt::{Debug, Formatter};

use common::api_bindings::{DetailedHost, HostState, PairStatus, UndetailedHost};
use log::{error, warn};
use moonlight_common::{
    PairPin, ServerVersion,
    high::{HostError, MoonlightHost, PairInfo},
    network::reqwest::{ReqwestError, ReqwestMoonlightHost},
    pair::{PairError, generate_new_client},
};
use uuid::Uuid;

use crate::app::{
    AppError, AppInner, AppRef,
    storage::{StorageHost, StorageHostModify, StorageHostPairInfo},
    user::{Admin, Role, User, UserId},
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

    pub async fn modify(&self, modify: StorageHostModify) -> Result<(), AppError> {
        let app = self.app.access()?;

        // TODO: clear cache
        app.storage.modify_host(self.id, modify).await?;

        Ok(())
    }

    pub async fn owner(&self) -> Result<Option<UserId>, AppError> {
        let app = self.app.access()?;

        let host = app.storage.get_host(self.id).await?;

        Ok(host.owner)
    }

    async fn use_host_no_auth<R>(
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
            if let Some(pair_info) = host_data.pair_info {
                new_host
                    .set_pair_info(PairInfo {
                        client_certificate: pair_info.client_certificate,
                        client_private_key: pair_info.client_private_key,
                        server_certificate: pair_info.server_certificate,
                    })
                    .await?;
            }

            hosts.entry(key).or_insert(new_host);
            drop(hosts);
        }
    }

    async fn storage_host(&self, app: &AppInner) -> Result<StorageHost, AppError> {
        app.storage.get_host(self.id).await
    }

    async fn internal_undetailed_host(
        &self,
        app: &AppInner,
        host: &ReqwestMoonlightHost,
    ) -> Result<UndetailedHost, AppError> {
        // TODO: maybe use the or_if_likely_offline from the detailed_host?
        let name = match host.host_name().await {
            Ok(value) => value,
            Err(HostError::LikelyOffline) => {
                let storage_host = self.storage_host(app).await?;
                storage_host.cache.name.clone()
            }
            Err(err) => {
                warn!("Failed to query host info for host {self:?}: {err:?}");

                let storage_host = self.storage_host(app).await?;
                storage_host.cache.name.clone()
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
    }
    pub async fn undetailed_host(&self, user: &mut User) -> Result<UndetailedHost, AppError> {
        self.use_host_no_auth(user, async |app, host| {
            self.internal_undetailed_host(app, host).await
        })
        .await?
    }
    pub async fn detailed_host(&self, user: &mut User) -> Result<DetailedHost, AppError> {
        fn or_if_likely_offline<T>(
            result: Result<T, HostError<ReqwestError>>,
            f: impl FnOnce() -> T,
        ) -> Result<T, HostError<ReqwestError>> {
            match result {
                Ok(value) => Ok(value),
                Err(HostError::LikelyOffline) => Ok(f()),
                _ => result,
            }
        }

        self.use_host_no_auth(user, async |app, host| {
            let UndetailedHost {
                host_id,
                name,
                paired,
                server_state,
            } = self.internal_undetailed_host(app, host).await?;

            Ok(DetailedHost {
                host_id,
                name,
                paired,
                server_state,
                http_port: host.http_port(),
                address: host.address().to_string(),
                https_port: or_if_likely_offline(host.https_port().await, || 0)?,
                external_port: or_if_likely_offline(host.external_port().await, || 0)?,
                current_game: or_if_likely_offline(host.current_game().await, || 0)?,
                gfe_version: or_if_likely_offline(host.gfe_version().await, || {
                    "OFFLINE".to_string()
                })?,
                local_ip: or_if_likely_offline(host.local_ip().await, || "OFFLINE".to_string())?,
                mac: or_if_likely_offline(host.mac().await, || None)
                    .map(|mac| mac.map(|mac| mac.to_string()))?,
                max_luma_pixels_hevc: or_if_likely_offline(
                    host.max_luma_pixels_hevc().await,
                    || 0,
                )?,
                server_codec_mode_support: or_if_likely_offline(
                    host.server_codec_mode_support_raw().await,
                    || 0,
                )?,
                unique_id: or_if_likely_offline(host.unique_id().await, Uuid::nil)
                    .map(|uuid| uuid.to_string())?,
                version: or_if_likely_offline(host.version().await, || {
                    ServerVersion::new(0, 0, 0, 0)
                })
                .map(|version| version.to_string())?,
            })
        })
        .await?
    }

    pub async fn is_paired(&self, user: &mut User) -> Result<PairStatus, AppError> {
        // TODO: should we use is_paired or verify_paired?
        self.use_host_no_auth(user, async |_app, host| host.is_paired().await)
            .await
            .map(PairStatus::from)
    }

    pub async fn pair(&self, user: &mut User, pin: PairPin) -> Result<(), AppError> {
        // TODO: maybe generalize this in some private func?
        if self.owner().await? != Some(user.id()) && !matches!(user.role().await?, Role::Admin) {
            return Err(AppError::Forbidden);
        }

        let modify = self
            .use_host_no_auth(user, async |_app, host| {
                if matches!(
                    PairStatus::from(host.verify_paired().await?),
                    PairStatus::Paired
                ) {
                    // TODO
                    todo!();
                }

                let auth = generate_new_client()?;

                // TODO: device name
                host.pair(&auth, "roth".to_string(), pin).await?;

                // Store pair info
                let Some(pair_info) = host.pair_info().await else {
                    error!("Failed to store pair info back into storage.");
                    return Err(AppError::MoonlightHost(HostError::Pair(PairError::Failed)));
                };

                let name = host.host_name().await?;
                let mac = host.mac().await?;

                Ok(StorageHostModify {
                    pair_info: Some(Some(StorageHostPairInfo {
                        client_private_key: pair_info.client_private_key,
                        client_certificate: pair_info.client_certificate,
                        server_certificate: pair_info.server_certificate,
                    })),
                    cache_name: Some(name),
                    cache_mac: Some(mac),
                    ..Default::default()
                })
            })
            .await??;

        self.modify(modify).await
    }

    pub async fn unpair(&self, user_id: UserId) -> Result<Host, AppError> {
        todo!()
    }

    pub async fn wake(&self) -> Result<(), AppError> {
        todo!()
    }

    pub async fn delete(self, user: &mut User) -> Result<(), AppError> {
        let app = self.app.access()?;

        let host = app.storage.get_host(self.id).await?;

        if host.owner == Some(user.id()) || matches!(user.role().await?, Role::Admin) {
            drop(app);
            self.delete_no_auth().await
        } else {
            Err(AppError::Forbidden)
        }
    }
    pub async fn delete_no_auth(self) -> Result<(), AppError> {
        let app = self.app.access()?;

        let mut hosts = app.loaded_hosts.write().await;
        hosts.retain(|(_, host_id), _| *host_id != self.id);

        app.storage.remove_host(self.id).await?;

        Ok(())
    }
}
