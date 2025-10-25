use std::{
    fmt::{Debug, Formatter},
    str::FromStr,
};

use actix_web::web::Bytes;
use common::api_bindings::{DetailedHost, HostState, PairStatus, UndetailedHost};
use log::{error, warn};
use moonlight_common::{
    PairPin, ServerState, ServerVersion,
    high::{HostError, MoonlightHost, PairInfo, broadcast_magic_packet},
    network::{
        ApiError, ClientAppBoxArtRequest, ClientInfo, HostInfo, host_app_box_art, host_app_list,
        host_info,
        request_client::RequestClient,
        reqwest::{ReqwestApiError, ReqwestError, ReqwestMoonlightHost},
    },
    pair::{PairError, PairSuccess, generate_new_client, host_pair},
};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::app::{
    AppError, AppInner, AppRef, MoonlightClient,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AppId(pub u32);

pub struct App {
    pub id: AppId,
    pub title: String,
    pub is_hdr_supported: bool,
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

    async fn use_client<R>(
        &self,
        app: &AppInner,
        user: &mut User,
        pairing: bool,
        // app, https_capable, client, host, port, client_info
        f: impl AsyncFnOnce(bool, &mut MoonlightClient, &str, u16, ClientInfo) -> R,
    ) -> Result<R, AppError> {
        // TODO: make this mut and store client as cache

        let user_unique_id = user.host_unique_id().await?;
        let host_data = self.storage_host(app).await?;

        let (mut client, https_capable) = if pairing {
            (
                MoonlightClient::with_defaults_long_timeout().map_err(ApiError::RequestClient)?,
                false,
            )
        } else if let Some(pair_info) = host_data.pair_info {
            (
                MoonlightClient::with_certificates(
                    &pair_info.client_private_key,
                    &pair_info.client_certificate,
                    &pair_info.server_certificate,
                )
                .map_err(ApiError::RequestClient)?,
                true,
            )
        } else {
            (
                MoonlightClient::with_defaults().map_err(ApiError::RequestClient)?,
                false,
            )
        };

        let info = ClientInfo {
            unique_id: &user_unique_id,
            uuid: Uuid::new_v4(),
        };

        Ok(f(
            https_capable,
            &mut client,
            &host_data.address,
            host_data.http_port,
            info,
        )
        .await)
    }
    fn build_hostport(host: &str, port: u16) -> String {
        format!("{host}:{port}")
    }

    async fn storage_host(&self, app: &AppInner) -> Result<StorageHost, AppError> {
        app.storage.get_host(self.id).await
    }

    fn is_offline<T>(
        &self,
        result: Result<T, ApiError<ReqwestError>>,
    ) -> Result<Option<T>, AppError> {
        match result {
            Ok(value) => Ok(Some(value)),
            Err(ApiError::RequestClient(ReqwestError::Reqwest(err)))
                if err.is_timeout() || err.is_connect() =>
            {
                Ok(None)
            }
            Err(err) => Err(AppError::MoonlightApi(err)),
        }
    }
    // None = Offline
    async fn host_info(
        &self,
        app: &AppInner,
        user: &mut User,
    ) -> Result<Option<HostInfo>, AppError> {
        // TODO: make this mut and store results as cache

        self.use_client(
            app,
            user,
            false,
            async |https_capable, client, host, port, client_info| {
                let mut info = match self.is_offline(
                    host_info(
                        client,
                        false,
                        &Self::build_hostport(host, port),
                        Some(client_info),
                    )
                    .await,
                ) {
                    Ok(Some(value)) => value,
                    err => return err,
                };

                if https_capable {
                    info = host_info(
                        client,
                        true,
                        &Self::build_hostport(host, info.https_port),
                        Some(client_info),
                    )
                    .await?;
                }

                Ok(Some(info))
            },
        )
        .await?
    }

    pub async fn undetailed_host(&self, user: &mut User) -> Result<UndetailedHost, AppError> {
        let app = self.app.access()?;

        match self.host_info(&app, user).await {
            Ok(Some(info)) => {
                let server_state = match ServerState::from_str(&info.state_string) {
                    Ok(state) => Some(state),
                    Err(err) => {
                        warn!(
                            "failed to parse server state of host {self:?}: {:?}, {}",
                            err, info.state_string
                        );

                        None
                    }
                };

                Ok(UndetailedHost {
                    host_id: self.id.0,
                    name: info.host_name,
                    paired: info.pair_status.into(),
                    server_state: server_state.map(HostState::from),
                })
            }
            Ok(None) => {
                let host = self.storage_host(&app).await?;

                let paired = if host.pair_info.is_some() {
                    PairStatus::Paired
                } else {
                    PairStatus::NotPaired
                };

                Ok(UndetailedHost {
                    host_id: self.id.0,
                    name: host.cache.name,
                    paired,
                    server_state: None,
                })
            }
            Err(err) => Err(err),
        }
    }
    pub async fn detailed_host(&self, user: &mut User) -> Result<DetailedHost, AppError> {
        let app = self.app.access()?;
        let host = self.storage_host(&app).await?;

        match self.host_info(&app, user).await {
            Ok(Some(info)) => {
                let server_state = match ServerState::from_str(&info.state_string) {
                    Ok(state) => Some(state),
                    Err(err) => {
                        warn!(
                            "failed to parse server state of host {self:?}: {:?}, {}",
                            err, info.state_string
                        );

                        None
                    }
                };

                Ok(DetailedHost {
                    host_id: self.id.0,
                    name: info.host_name,
                    paired: info.pair_status.into(),
                    server_state: server_state.map(HostState::from),
                    address: host.address,
                    http_port: host.http_port,
                    https_port: info.https_port,
                    external_port: info.external_port,
                    version: info.app_version.to_string(),
                    gfe_version: info.gfe_version,
                    unique_id: info.unique_id.to_string(),
                    mac: info.mac.map(|mac| mac.to_string()),
                    local_ip: info.local_ip,
                    current_game: info.current_game,
                    max_luma_pixels_hevc: info.max_luma_pixels_hevc,
                    server_codec_mode_support: info.server_codec_mode_support,
                })
            }
            Ok(None) => {
                let paired = if host.pair_info.is_some() {
                    PairStatus::Paired
                } else {
                    PairStatus::NotPaired
                };

                Ok(DetailedHost {
                    host_id: self.id.0,
                    name: host.cache.name,
                    paired,
                    server_state: None,
                    address: host.address,
                    http_port: host.http_port,
                    https_port: 0,
                    external_port: 0,
                    version: "Offline".to_string(),
                    gfe_version: "Offline".to_string(),
                    unique_id: "Offline".to_string(),
                    mac: host.cache.mac.map(|mac| mac.to_string()),
                    local_ip: "Offline".to_string(),
                    current_game: 0,
                    max_luma_pixels_hevc: 0,
                    server_codec_mode_support: 0,
                })
            }
            Err(err) => Err(err),
        }
    }

    pub async fn is_paired(&self, user: &mut User) -> Result<PairStatus, AppError> {
        let app = self.app.access()?;

        match self.host_info(&app, user).await? {
            Some(info) => Ok(info.pair_status.into()),
            None => Ok(PairStatus::NotPaired),
        }
    }

    pub async fn pair(&self, user: &mut User, pin: PairPin) -> Result<(), AppError> {
        let app = self.app.access()?;

        // TODO: maybe generalize this in some private func?
        if self.owner().await? != Some(user.id()) && !matches!(user.role().await?, Role::Admin) {
            return Err(AppError::Forbidden);
        }

        let info = self
            .host_info(&app, user)
            .await?
            .ok_or(AppError::HostNotFound)?;

        if matches!(info.pair_status.into(), PairStatus::Paired) {
            // TODO: return not modified
            todo!();
        }

        let modify = self
            .use_client(
                &app,
                user,
                true,
                async |_https_capable, client, host, port, client_info| {
                    let auth = generate_new_client()?;

                    // TODO: device name
                    let PairSuccess { server_certificate } = host_pair(
                        client,
                        &Self::build_hostport(host, port),
                        client_info,
                        &auth.private_key,
                        &auth.certificate,
                        "roth",
                        info.app_version,
                        pin,
                    )
                    .await
                    // TODO: handle pair error correctly!
                    .unwrap();

                    // Store pair info

                    // TODO: we'll have to create a new client with certificates and store that in cache and use it here
                    let mut client = MoonlightClient::with_certificates(&auth.private_key,&auth.certificate, &server_certificate).map_err(ApiError::RequestClient)?;

                    let (name, mac) = match host_info(
                        &mut client,
                        true,
                        &Self::build_hostport(host, info.https_port),
                        Some(client_info),
                    )
                    .await
                    {
                        Ok(info) => (Some(info.host_name), Some(info.mac)),
                        Err(err) => {
                            error!("Failed to make https request to host {self:?} after pairing completed: {err}");
                            (None, None)},
                    };

                    Ok::<_, AppError>(StorageHostModify {
                        pair_info: Some(Some(StorageHostPairInfo {
                            client_private_key: auth.private_key,
                            client_certificate: auth.certificate,
                            server_certificate,
                        })),
                        cache_name: name,
                        cache_mac: mac,
                        ..Default::default()
                    })
                },
            )
            .await??;

        self.modify(modify).await
    }

    pub async fn unpair(&self, user_id: UserId) -> Result<Host, AppError> {
        todo!()
    }

    pub async fn wake(&self) -> Result<(), AppError> {
        let app = self.app.access()?;

        let storage = self.storage_host(&app).await?;

        if let Some(mac) = storage.cache.mac {
            // TODO: error
            broadcast_magic_packet(mac).await.unwrap();
            Ok(())
        } else {
            Err(AppError::HostNotFound)
        }
    }

    pub async fn list_apps(&self, user: &mut User) -> Result<Vec<App>, AppError> {
        let app = self.app.access()?;

        let info = self
            .host_info(&app, user)
            .await?
            .ok_or(AppError::HostOffline)?;

        self.use_client(
            &app,
            user,
            false,
            async |https_capable, client, host, _port, client_info| {
                if !https_capable {
                    return Err(AppError::HostNotPaired);
                }

                let apps = host_app_list(
                    client,
                    &Self::build_hostport(host, info.https_port),
                    client_info,
                )
                .await?;

                let apps = apps
                    .apps
                    .into_iter()
                    .map(|app| App {
                        id: AppId(app.id),
                        title: app.title,
                        is_hdr_supported: app.is_hdr_supported,
                    })
                    .collect::<Vec<_>>();

                Ok(apps)
            },
        )
        .await?
    }
    pub async fn app_image(&self, user: &mut User, app_id: AppId) -> Result<Bytes, AppError> {
        let app = self.app.access()?;

        let info = self
            .host_info(&app, user)
            .await?
            .ok_or(AppError::HostOffline)?;

        self.use_client(
            &app,
            user,
            false,
            async |https_capable, client, host, _port, client_info| {
                if !https_capable {
                    return Err(AppError::HostNotPaired);
                }

                let image = host_app_box_art(
                    client,
                    &Self::build_hostport(host, info.https_port),
                    client_info,
                    ClientAppBoxArtRequest { app_id: app_id.0 },
                )
                .await?;

                Ok(image)
            },
        )
        .await?
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

        app.storage.remove_host(self.id).await?;

        Ok(())
    }
}
