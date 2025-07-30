//!
//! The high level api of the moonlight wrapper
//!

use pem::Pem;
use reqwest::{Certificate, Client};
use tokio::task::block_in_place;
use uuid::Uuid;

use crate::{
    Error, MoonlightInstance,
    crypto::MoonlightCrypto,
    data::{
        ColorRange, Colorspace, EncryptionFlags, ServerInfo, StreamConfiguration, StreamingConfig,
        SupportedVideoFormats,
    },
    network::{
        ApiError, ClientInfo, ClientStreamRequest, HostInfo, PairStatus, ServerState,
        ServerVersion, host_info, host_launch,
    },
    pair::{
        PairPin,
        high::{ClientAuth, PairError, PairSuccess, host_pair},
    },
    stream::MoonlightStream,
};

pub struct MoonlightHost<PairStatus> {
    client_unique_id: String,
    client_uuid: Uuid,
    client: Client,
    address: String,
    http_port: u16,
    info: Option<HostInfo>,
    paired: PairStatus,
}

pub struct Unknown;
pub struct Unpaired;
pub struct Paired {
    device_name: String,
    server_certificate: Pem,
}
pub enum MaybePaired {
    Unpaired(Unpaired),
    Paired(Paired),
}

impl From<Paired> for MaybePaired {
    fn from(value: Paired) -> Self {
        Self::Paired(value)
    }
}
impl From<Unpaired> for MaybePaired {
    fn from(value: Unpaired) -> Self {
        Self::Unpaired(value)
    }
}

impl MoonlightHost<Unknown> {
    pub fn new(address: String, http_port: u16, client: Option<ClientInfo>) -> Self {
        #[allow(clippy::unwrap_or_default)]
        let client = client.unwrap_or(ClientInfo::default());

        Self {
            client: Client::new(),
            client_unique_id: client.unique_id.to_string(),
            client_uuid: client.uuid(),
            address,
            http_port,
            info: None,
            paired: Unknown,
        }
    }
}

impl<Pair> MoonlightHost<Pair> {
    fn http_address(&self) -> String {
        format!("{}:{}", self.address, self.http_port)
    }

    async fn host_info(&mut self) -> Result<&HostInfo, ApiError> {
        if self.info.is_none() {
            self.info = Some(
                host_info(
                    &self.client,
                    false,
                    &self.http_address(),
                    Some(self.client_info()),
                )
                .await?,
            );
        }

        let Some(info) = &self.info else {
            unreachable!()
        };

        Ok(info)
    }

    pub fn client_info(&'_ self) -> ClientInfo<'_> {
        ClientInfo {
            unique_id: &self.client_unique_id,
            // uuid: self.client_uuid,
        }
    }

    pub async fn https_port(&mut self) -> Result<u16, ApiError> {
        let info = self.host_info().await?;
        Ok(info.https_port)
    }
    pub async fn https_address(&mut self) -> Result<String, ApiError> {
        let https_port = self.https_port().await?;
        Ok(format!("{}:{}", self.address, https_port))
    }
    pub async fn external_port(&mut self) -> Result<u16, ApiError> {
        let info = self.host_info().await?;
        Ok(info.external_port)
    }

    pub async fn host_name(&mut self) -> Result<&str, ApiError> {
        let info = self.host_info().await?;
        Ok(&info.host_name)
    }
    pub async fn version(&mut self) -> Result<ServerVersion, ApiError> {
        let info = self.host_info().await?;
        Ok(info.app_version)
    }

    pub async fn gfe_version(&mut self) -> Result<&str, ApiError> {
        let info = self.host_info().await?;
        Ok(&info.gfe_version)
    }
    pub async fn unique_id(&mut self) -> Result<Uuid, ApiError> {
        let info = self.host_info().await?;
        Ok(info.unique_id)
    }

    pub async fn mac(&mut self) -> Result<&str, ApiError> {
        let info = self.host_info().await?;
        Ok(&info.mac)
    }
    pub async fn local_ip(&mut self) -> Result<&str, ApiError> {
        let info = self.host_info().await?;
        Ok(&info.local_ip)
    }

    pub async fn current_game(&mut self) -> Result<u32, ApiError> {
        let info = self.host_info().await?;
        Ok(info.current_game)
    }

    pub async fn state(&mut self) -> Result<(&str, ServerState), ApiError> {
        let info = self.host_info().await?;
        Ok((&info.state_string, info.state))
    }

    pub async fn max_luma_pixels_hevc(&mut self) -> Result<u32, ApiError> {
        let info = self.host_info().await?;
        Ok(info.max_luma_pixels_hevc)
    }
    pub async fn server_codec_mode_support(&mut self) -> Result<u32, ApiError> {
        let info = self.host_info().await?;
        Ok(info.server_codec_mode_support)
    }

    pub fn into_unpaired(self) -> MoonlightHost<Unpaired> {
        MoonlightHost {
            client: self.client,
            client_unique_id: self.client_unique_id,
            client_uuid: self.client_uuid,
            address: self.address,
            http_port: self.http_port,
            info: self.info,
            paired: Unpaired,
        }
    }
    // TODO: add some values to make it possible to either be paired or unpaired
    pub async fn pair_state(
        mut self,
        auth: Option<&ClientAuth>,
    ) -> Result<MoonlightHost<MaybePaired>, (Self, ApiError)> {
        let client = if let Some(auth) = auth {
            let client_cert = Certificate::from_der(&auth.certificate.contents()).unwrap();

            Client::builder()
                .add_root_certificate(client_cert)
                .build()
                .unwrap()
        } else {
            self.client.clone()
        };

        let https_address = match self.https_address().await {
            Err(err) => return Err((self, err)),
            Ok(value) => value,
        };

        let info = match host_info(&client, true, &https_address, Some(self.client_info())).await {
            Err(err) => return Err((self, err)),
            Ok(value) => value,
        };

        match info.pair_status {
            PairStatus::NotPaired => Ok(MoonlightHost {
                client: self.client,
                client_unique_id: self.client_unique_id,
                client_uuid: self.client_uuid,
                address: self.address,
                http_port: self.http_port,
                info: Some(info),
                paired: MaybePaired::Unpaired(Unpaired),
            }),
            PairStatus::Paired => Ok(MoonlightHost {
                client: self.client,
                client_unique_id: self.client_unique_id,
                client_uuid: self.client_uuid,
                address: self.address,
                http_port: self.http_port,
                info: Some(info),
                paired: MaybePaired::Paired(Paired {
                    device_name: todo!(),
                    server_certificate: todo!(),
                }),
            }),
        }
    }
}

impl<PairStatus> MoonlightHost<PairStatus>
where
    PairStatus: Into<MaybePaired>,
{
    pub fn maybe_paired(self) -> MoonlightHost<MaybePaired> {
        MoonlightHost {
            client: self.client,
            client_unique_id: self.client_unique_id,
            client_uuid: self.client_uuid,
            address: self.address,
            http_port: self.http_port,
            info: self.info,
            paired: self.paired.into(),
        }
    }
}

impl MoonlightHost<MaybePaired> {
    #[allow(clippy::result_large_err)]
    pub fn into_paired(self) -> Result<MoonlightHost<Paired>, MoonlightHost<Unpaired>> {
        match self.paired {
            MaybePaired::Paired(paired) => Ok(MoonlightHost {
                client: self.client,
                client_unique_id: self.client_unique_id,
                client_uuid: self.client_uuid,
                address: self.address,
                http_port: self.http_port,
                info: self.info,
                paired,
            }),
            MaybePaired::Unpaired(paired) => Err(MoonlightHost {
                client: self.client,
                client_unique_id: self.client_unique_id,
                client_uuid: self.client_uuid,
                address: self.address,
                http_port: self.http_port,
                info: self.info,
                paired,
            }),
        }
    }
}

impl MoonlightHost<Unpaired> {
    pub async fn pair(
        mut self,
        crypto: &MoonlightCrypto,
        auth: &ClientAuth,
        device_name: String,
        pin: PairPin,
    ) -> Result<MoonlightHost<Paired>, (Self, PairError)> {
        let http_address = self.http_address();
        let server_version = match self.version().await {
            Err(err) => return Err((self, err.into())),
            Ok(value) => value,
        };

        let PairSuccess { server_certificate } = match host_pair(
            crypto,
            &self.client,
            &http_address,
            self.client_info(),
            &auth.key_pair,
            &auth.certificate,
            &device_name,
            server_version,
            pin,
        )
        .await
        {
            Err(err) => return Err((self, err)),
            Ok(value) => value,
        };

        Ok(MoonlightHost {
            client: self.client,
            client_unique_id: self.client_unique_id,
            client_uuid: self.client_uuid,
            address: self.address,
            http_port: self.http_port,
            info: self.info,
            // TODO: other info which is required
            paired: Paired {
                device_name,
                server_certificate,
            },
        })
    }
}

#[derive(Debug, Error)]
pub enum StreamError {
    #[error("{0}")]
    Moonlight(#[from] Error),
    #[error("{0}")]
    Api(#[from] ApiError),
}

impl MoonlightHost<Paired> {
    // TODO: add a fn to create / correct streaming info: e.g. width, height, fps

    pub async fn start_stream(
        &mut self,
        instance: &MoonlightInstance,
        app_id: u32,
        width: u32,
        height: u32,
        fps: u32,
        color_space: Colorspace,
        color_range: ColorRange,
    ) -> Result<MoonlightStream, StreamError> {
        let http_address = self.http_address();
        let https_address = self.https_address().await?;

        let launch_response = host_launch(
            instance,
            &self.client,
            &https_address,
            self.client_info(),
            ClientStreamRequest {
                app_id,
                mode_width: width,
                mode_height: height,
                mode_fps: fps,
                ri_key: [0u8; 16],
                ri_key_id: [0u8; 16],
            },
        )
        .await?;

        let app_version = self.version().await?;
        let server_codec_mode_support = self.server_codec_mode_support().await?;
        let gfe_version = self.gfe_version().await?;

        let connection = block_in_place(|| {
            let server_info = ServerInfo {
                address: &http_address,
                app_version,
                gfe_version,
                rtsp_session_url: &launch_response.rtsp_session_url,
                server_codec_mode_support: server_codec_mode_support as i32,
            };

            // TODO: check if the width,height,fps,color_space,color_range are valid
            let stream_config = StreamConfiguration {
                width: width as i32,
                height: height as i32,
                fps: fps as i32,
                bitrate: 10,
                packet_size: 1024,
                streaming_remotely: StreamingConfig::Remote,
                audio_configuration: 0,
                supported_video_formats: SupportedVideoFormats::default(),
                client_refresh_rate_x100: 60,
                color_space,
                color_range,
                encryption_flags: EncryptionFlags::all(),
                // TODO: aquire them from paired member field
                remote_input_aes_key: [0u8; 16usize],
                remote_input_aes_iv: [0u8; 16usize],
            };

            instance.start_connection(server_info, stream_config)
        })?;

        Ok(connection)
    }

    pub fn pair_device_name(&self) -> &str {
        &self.paired.device_name
    }
}
