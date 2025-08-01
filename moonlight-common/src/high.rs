//!
//! The high level api of the moonlight wrapper
//!

use pem::Pem;
use reqwest::{Certificate, Client, ClientBuilder, Identity};
use tokio::task::block_in_place;
use uuid::Uuid;

use crate::{
    Error, MoonlightInstance,
    audio::AudioDecoder,
    connection::ConnectionListener,
    crypto::MoonlightCrypto,
    network::{
        ApiError, App, ClientInfo, ClientStreamRequest, DEFAULT_UNIQUE_ID, HostAppListResponse,
        HostInfo, PairStatus, ServerState, ServerVersion, host_app_list, host_info, host_launch,
        host_resume,
    },
    pair::{
        PairPin,
        high::{ClientAuth, PairError, PairSuccess, host_pair},
    },
    stream::{
        ColorRange, Colorspace, EncryptionFlags, MoonlightStream, ServerCodeModeSupport,
        ServerInfo, StreamConfiguration, StreamingConfig,
    },
    video::VideoDecoder,
};

fn default_client_builder() -> ClientBuilder {
    ClientBuilder::new()
    // .connect_timeout(Duration::from_secs(5))
    // .timeout(Duration::from_secs(7))
}
fn tls_client_builder(
    auth: &ClientAuth,
    server_certificate: &Pem,
) -> Result<ClientBuilder, ApiError> {
    let server_cert = Certificate::from_pem(server_certificate.to_string().as_bytes())?;

    let mut client_pem = String::new();
    client_pem.push_str(&auth.key_pair.to_string());
    client_pem.push('\n');
    client_pem.push_str(&auth.certificate.to_string());

    let identity = Identity::from_pkcs8_pem(
        auth.certificate.to_string().as_bytes(),
        auth.key_pair.to_string().as_bytes(),
    )?;

    Ok(default_client_builder()
        .use_native_tls()
        .tls_built_in_root_certs(false)
        .add_root_certificate(server_cert)
        .identity(identity)
        .danger_accept_invalid_hostnames(true))
}

pub struct MoonlightHost<PairStatus> {
    client_unique_id: String,
    client: Client,
    address: String,
    http_port: u16,
    info: Option<HostInfo>,
    paired: PairStatus,
}

pub struct Unknown;
pub struct Unpaired;
pub struct Paired {
    server_certificate: Pem,
    app_list: Option<HostAppListResponse>,
}
pub enum MaybePaired {
    Unpaired(Unpaired),
    Paired(Box<Paired>),
}

impl From<Paired> for MaybePaired {
    fn from(value: Paired) -> Self {
        Self::Paired(Box::new(value))
    }
}
impl From<Unpaired> for MaybePaired {
    fn from(value: Unpaired) -> Self {
        Self::Unpaired(value)
    }
}

impl MoonlightHost<Unknown> {
    pub fn new(address: String, http_port: u16, unique_id: Option<String>) -> Self {
        Self {
            client: default_client_builder().build().expect("reqwest client"),
            client_unique_id: unique_id.unwrap_or_else(|| DEFAULT_UNIQUE_ID.to_string()),
            address,
            http_port,
            info: None,
            paired: Unknown,
        }
    }
}

impl<Pair> MoonlightHost<Pair> {
    pub fn address(&self) -> &str {
        &self.address
    }
    pub fn http_port(&self) -> u16 {
        self.http_port
    }

    pub fn http_address(&self) -> String {
        format!("{}:{}", self.address, self.http_port)
    }

    async fn host_info(&mut self) -> Result<&HostInfo, ApiError> {
        if self.info.is_none() {
            self.reload_host_info().await?;
        }

        let Some(info) = &self.info else {
            unreachable!()
        };

        Ok(info)
    }
    pub async fn reload_host_info(&mut self) -> Result<(), ApiError> {
        self.info = Some(
            host_info(
                &self.client,
                false,
                &self.http_address(),
                Some(self.client_info()),
            )
            .await?,
        );

        Ok(())
    }

    pub fn client_info(&'_ self) -> ClientInfo<'_> {
        ClientInfo {
            unique_id: &self.client_unique_id,
            uuid: Uuid::new_v4(),
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
    pub async fn server_codec_mode_support(&mut self) -> Result<ServerCodeModeSupport, ApiError> {
        let info = self.host_info().await?;
        Ok(info.server_codec_mode_support)
    }

    pub fn into_unpaired(self) -> MoonlightHost<Unpaired> {
        MoonlightHost {
            client: self.client,
            client_unique_id: self.client_unique_id,
            address: self.address,
            http_port: self.http_port,
            info: self.info,
            paired: Unpaired,
        }
    }

    pub async fn pair_state(
        mut self,
        auth: Option<(&ClientAuth, &Pem)>,
    ) -> Result<MoonlightHost<MaybePaired>, (Self, ApiError)> {
        let client = if let Some((auth, server_certificate)) = auth {
            match tls_client_builder(auth, server_certificate)
                .and_then(|client| client.build().map_err(ApiError::from))
            {
                Err(err) => return Err((self, err)),
                Ok(client) => client,
            }
        } else {
            self.client.clone()
        };

        let https_address = match self.https_address().await {
            Err(err) => return Err((self, err)),
            Ok(value) => value,
        };

        let info = match host_info(&client, true, &https_address, Some(self.client_info())).await {
            Err(err) => {
                return Err((self, err));
            }
            Ok(value) => value,
        };

        match info.pair_status {
            PairStatus::NotPaired => Ok(MoonlightHost {
                client,
                client_unique_id: self.client_unique_id,
                address: self.address,
                http_port: self.http_port,
                info: Some(info),
                paired: MaybePaired::Unpaired(Unpaired),
            }),
            PairStatus::Paired => {
                let (_, server_certificate) = auth.expect("valid private key and certificates");

                Ok(MoonlightHost {
                    client,
                    client_unique_id: self.client_unique_id,
                    address: self.address,
                    http_port: self.http_port,
                    info: Some(info),
                    paired: Paired {
                        server_certificate: server_certificate.clone(),
                        app_list: None,
                    }
                    .into(),
                })
            }
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
            address: self.address,
            http_port: self.http_port,
            info: self.info,
            paired: self.paired.into(),
        }
    }
}

impl MoonlightHost<MaybePaired> {
    #[allow(clippy::result_large_err)]
    pub fn try_into_paired(self) -> Result<MoonlightHost<Paired>, MoonlightHost<Unpaired>> {
        match self.paired {
            MaybePaired::Paired(paired) => Ok(MoonlightHost {
                client: self.client,
                client_unique_id: self.client_unique_id,
                address: self.address,
                http_port: self.http_port,
                info: self.info,
                paired: *paired,
            }),
            MaybePaired::Unpaired(paired) => Err(MoonlightHost {
                client: self.client,
                client_unique_id: self.client_unique_id,
                address: self.address,
                http_port: self.http_port,
                info: self.info,
                paired,
            }),
        }
    }

    pub fn is_paired(&self) -> PairStatus {
        match &self.paired {
            MaybePaired::Unpaired(_) => PairStatus::NotPaired,
            MaybePaired::Paired(_) => PairStatus::NotPaired,
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

        let client = match tls_client_builder(auth, &server_certificate)
            .and_then(|client| client.build().map_err(ApiError::from))
        {
            Err(err) => return Err((self, err.into())),
            Ok(client) => client,
        };

        Ok(MoonlightHost {
            client,
            client_unique_id: self.client_unique_id,
            address: self.address,
            http_port: self.http_port,
            info: self.info,
            // TODO: other info which is required
            paired: Paired {
                server_certificate,
                app_list: None,
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
    pub fn server_certificate(&self) -> &Pem {
        &self.paired.server_certificate
    }

    pub async fn app_list(&mut self) -> Result<&[App], ApiError> {
        if self.paired.app_list.is_none() {
            let https_address = self.https_address().await?;

            self.paired.app_list =
                Some(host_app_list(&self.client, &https_address, self.client_info()).await?);
        }

        let Some(app_list) = &self.paired.app_list else {
            unreachable!()
        };

        Ok(&app_list.apps)
    }

    // TODO: add a fn to create / correct streaming info: e.g. width, height, fps

    pub async fn start_stream(
        &mut self,
        instance: &MoonlightInstance,
        crypto: &MoonlightCrypto,
        app_id: u32,
        width: u32,
        height: u32,
        fps: u32,
        color_space: Colorspace,
        color_range: ColorRange,
        bitrate: u32,
        packet_size: u32,
        connection_listener: impl ConnectionListener + Send + 'static,
        video_decoder: impl VideoDecoder + Send + 'static,
        audio_decoder: impl AudioDecoder + Send + 'static,
    ) -> Result<MoonlightStream, StreamError> {
        let address = self.address.clone();
        let https_address = self.https_address().await?;

        let mut aes_key = [0u8; 16];
        crypto.generate_random(&mut aes_key);

        let mut aes_iv = [0u8; 4];
        crypto.generate_random(&mut aes_iv);
        let aes_iv = i32::from_be_bytes(aes_iv);

        let request = ClientStreamRequest {
            app_id,
            mode_width: width,
            mode_height: height,
            mode_fps: fps,
            ri_key: aes_key,
            ri_key_id: aes_iv,
        };

        let rtsp_session_url = if self.current_game().await? == 0 {
            let launch_response = host_launch(
                instance,
                &self.client,
                &https_address,
                self.client_info(),
                request,
            )
            .await?;

            launch_response.rtsp_session_url
        } else {
            let resume_response = host_resume(
                instance,
                &self.client,
                &https_address,
                self.client_info(),
                request,
            )
            .await?;

            resume_response.rtsp_session_url
        };

        let app_version = self.version().await?;
        let server_codec_mode_support = self.server_codec_mode_support().await?;
        let gfe_version = self.gfe_version().await?;

        let connection = block_in_place(|| {
            let server_info = ServerInfo {
                address: &address,
                app_version,
                gfe_version,
                rtsp_session_url: &rtsp_session_url,
                server_codec_mode_support,
            };

            // TODO: check if the width,height,fps,color_space,color_range are valid
            let stream_config = StreamConfiguration {
                width: width as i32,
                height: height as i32,
                fps: fps as i32,
                bitrate: bitrate as i32,
                packet_size: packet_size as i32,
                streaming_remotely: StreamingConfig::Auto,
                audio_configuration: audio_decoder.config().0 as i32,
                supported_video_formats: video_decoder.supported_formats(),
                client_refresh_rate_x100: (fps * 100) as i32,
                color_space,
                color_range,
                encryption_flags: EncryptionFlags::all(),
                remote_input_aes_key: aes_key,
                remote_input_aes_iv: aes_iv,
            };

            instance.start_connection(
                server_info,
                stream_config,
                connection_listener,
                video_decoder,
                audio_decoder,
            )
        })?;

        Ok(connection)
    }
}
