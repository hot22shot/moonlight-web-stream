//!
//! The high level api of the moonlight wrapper
//!

use pem::Pem;
use tokio::task::spawn_blocking;
use uuid::Uuid;

use crate::{
    Error, MoonlightInstance,
    audio::AudioDecoder,
    connection::ConnectionListener,
    crypto::MoonlightCrypto,
    network::{
        ApiError, App, ClientAppBoxArtRequest, ClientInfo, ClientStreamRequest, DEFAULT_UNIQUE_ID,
        HostInfo, PairStatus, ServerAppListResponse, ServerState, ServerVersion, host_app_box_art,
        host_app_list, host_info, host_launch, host_resume, request_client::RequestClient,
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

#[derive(Debug, Error)]
pub enum HostError<RequestError> {
    #[error("{0}")]
    Moonlight(#[from] Error),
    #[error("this action requires pairing")]
    NotPaired,
    #[error("{0}")]
    Api(#[from] ApiError<RequestError>),
    #[error("{0}")]
    Pair(#[from] PairError<RequestError>),
}

// TODO: feature lock
pub type SimpleMoonlightHost = MoonlightHost<reqwest::Client>;

pub struct MoonlightHost<Client> {
    client_unique_id: String,
    client: Client,
    address: String,
    http_port: u16,
    cache_info: Option<HostInfo>,
    // Paired
    paired: Option<Paired>,
}

#[derive(Clone)]
pub struct Paired {
    client_private_key: Pem,
    client_certificate: Pem,
    server_certificate: Pem,
    cache_app_list: Option<ServerAppListResponse>,
}

// TODO: for futures return impl Future<Output = ?> + Send + Sync

impl<C> MoonlightHost<C>
where
    C: RequestClient,
{
    pub fn new(
        address: String,
        http_port: u16,
        unique_id: Option<String>,
    ) -> Result<Self, HostError<C::Error>> {
        Ok(Self {
            client: C::with_defaults()
                .map_err(|err| HostError::Api(ApiError::RequestClient(err)))?,
            client_unique_id: unique_id.unwrap_or_else(|| DEFAULT_UNIQUE_ID.to_string()),
            address,
            http_port,
            cache_info: None,
            paired: None,
        })
    }

    pub fn address(&self) -> &str {
        &self.address
    }
    pub fn http_port(&self) -> u16 {
        self.http_port
    }

    pub fn http_address(&self) -> String {
        format!("{}:{}", self.address, self.http_port)
    }

    async fn host_info(&mut self) -> Result<&HostInfo, HostError<C::Error>> {
        if self.cache_info.is_none() {
            self.clear_cache();
        }

        let has_cache = self.cache_info.is_some();
        let mut https_port = None;

        if !has_cache {
            let http_address = self.http_address();

            let client_info = ClientInfo {
                unique_id: &self.client_unique_id,
                uuid: Uuid::new_v4(),
            };

            let info = host_info(&mut self.client, false, &http_address, Some(client_info)).await?;

            https_port = Some(info.https_port);

            self.cache_info = Some(info);
        }
        if !has_cache
            && let Some(https_port) = https_port
            && self.is_paired() == PairStatus::Paired
        {
            let https_address = Self::build_https_address(&self.address, https_port);

            let client_info = ClientInfo {
                unique_id: &self.client_unique_id,
                uuid: Uuid::new_v4(),
            };
            self.cache_info =
                Some(host_info(&mut self.client, true, &https_address, Some(client_info)).await?);
        }

        let Some(info) = &self.cache_info else {
            unreachable!()
        };

        Ok(info)
    }
    pub fn clear_cache(&mut self) {
        self.cache_info = None;
        if let Some(paired) = self.paired.as_mut() {
            paired.cache_app_list = None;
        }
    }

    pub async fn https_port(&mut self) -> Result<u16, HostError<C::Error>> {
        let info = self.host_info().await?;
        Ok(info.https_port)
    }

    fn build_https_address(address: &str, https_port: u16) -> String {
        format!("{address}:{https_port}")
    }
    pub async fn https_address(&mut self) -> Result<String, HostError<C::Error>> {
        let https_port = self.https_port().await?;
        Ok(Self::build_https_address(&self.address, https_port))
    }
    pub async fn external_port(&mut self) -> Result<u16, HostError<C::Error>> {
        let info = self.host_info().await?;
        Ok(info.external_port)
    }

    pub async fn host_name(&mut self) -> Result<&str, HostError<C::Error>> {
        let info = self.host_info().await?;
        Ok(info.host_name.as_str())
    }
    pub async fn version(&mut self) -> Result<ServerVersion, HostError<C::Error>> {
        let info = self.host_info().await?;
        Ok(info.app_version)
    }

    pub async fn gfe_version(&mut self) -> Result<&str, HostError<C::Error>> {
        let info = self.host_info().await?;
        Ok(info.gfe_version.as_str())
    }
    pub async fn unique_id(&mut self) -> Result<Uuid, HostError<C::Error>> {
        let info = self.host_info().await?;
        Ok(info.unique_id)
    }

    pub async fn mac(&mut self) -> Result<&str, HostError<C::Error>> {
        let info = self.host_info().await?;
        Ok(info.mac.as_str())
    }
    pub async fn local_ip(&mut self) -> Result<&str, HostError<C::Error>> {
        let info = self.host_info().await?;
        Ok(info.local_ip.as_str())
    }

    pub async fn current_game(&mut self) -> Result<u32, HostError<C::Error>> {
        let info = self.host_info().await?;
        Ok(info.current_game)
    }

    pub async fn state(&mut self) -> Result<(&str, ServerState), HostError<C::Error>> {
        let info = self.host_info().await?;
        Ok((info.state_string.as_str(), info.state))
    }

    pub async fn max_luma_pixels_hevc(&mut self) -> Result<u32, HostError<C::Error>> {
        let info = self.host_info().await?;
        Ok(info.max_luma_pixels_hevc)
    }
    pub async fn server_codec_mode_support(
        &mut self,
    ) -> Result<ServerCodeModeSupport, HostError<C::Error>> {
        let info = self.host_info().await?;
        Ok(info.server_codec_mode_support)
    }

    pub async fn set_pairing_info(
        &mut self,
        client_auth: &ClientAuth,
        server_certificate: &Pem,
    ) -> Result<PairStatus, HostError<C::Error>> {
        self.client = C::with_certificates(
            &client_auth.key_pair,
            &client_auth.certificate,
            server_certificate,
        )
        .map_err(ApiError::RequestClient)?;

        self.paired = Some(Paired {
            client_private_key: client_auth.key_pair.clone(),
            client_certificate: client_auth.certificate.clone(),
            server_certificate: server_certificate.clone(),
            cache_app_list: None,
        });

        let https_address = match self.https_address().await {
            Err(err) => return Err(err),
            Ok(value) => value,
        };

        let client_info = ClientInfo {
            unique_id: &self.client_unique_id,
            uuid: Uuid::new_v4(),
        };

        let info = host_info(&mut self.client, true, &https_address, Some(client_info)).await?;

        let pair_status = info.pair_status;
        self.cache_info = Some(info);

        Ok(pair_status)
    }
    pub async fn clear_pairing_info(&mut self) -> Result<(), HostError<C::Error>> {
        self.client = C::with_defaults().map_err(ApiError::RequestClient)?;
        self.paired = None;

        Ok(())
    }

    pub fn is_paired(&self) -> PairStatus {
        if self.paired.is_some() {
            PairStatus::Paired
        } else {
            PairStatus::NotPaired
        }
    }

    pub fn client_private_key(&self) -> Option<&Pem> {
        self.paired.as_ref().map(|x| &x.client_private_key)
    }
    pub fn client_certificate(&self) -> Option<&Pem> {
        self.paired.as_ref().map(|x| &x.client_certificate)
    }
    pub fn server_certificate(&self) -> Option<&Pem> {
        self.paired.as_ref().map(|x| &x.server_certificate)
    }

    pub async fn pair(
        &mut self,
        crypto: &MoonlightCrypto,
        auth: &ClientAuth,
        device_name: String,
        pin: PairPin,
    ) -> Result<(), HostError<C::Error>> {
        let http_address = self.http_address();
        // TODO: convert error
        let server_version = self.version().await?;

        let client_info = ClientInfo {
            unique_id: &self.client_unique_id,
            uuid: Uuid::new_v4(),
        };

        let PairSuccess { server_certificate } = host_pair(
            crypto,
            &mut self.client,
            &http_address,
            client_info,
            &auth.key_pair,
            &auth.certificate,
            &device_name,
            server_version,
            pin,
        )
        .await?;

        self.client = C::with_certificates(&auth.key_pair, &auth.certificate, &server_certificate)
            .map_err(|err| HostError::Api(ApiError::RequestClient(err)))?;

        self.paired = Some(Paired {
            client_private_key: auth.key_pair.clone(),
            client_certificate: auth.certificate.clone(),
            server_certificate,
            cache_app_list: None,
        });

        let Some(info) = self.cache_info.as_mut() else {
            unreachable!()
        };
        info.pair_status = PairStatus::Paired;

        self.clear_cache();

        Ok(())
    }

    pub async fn app_list(&mut self) -> Result<&[App], HostError<C::Error>> {
        let https_address = self.https_address().await?;
        let client_info = ClientInfo {
            unique_id: &self.client_unique_id,
            uuid: Uuid::new_v4(),
        };

        let Some(paired) = self.paired.as_mut() else {
            return Err(HostError::NotPaired);
        };

        // Recache
        if paired.cache_app_list.is_none() {
            let response = host_app_list(&mut self.client, &https_address, client_info).await?;

            paired.cache_app_list = Some(response);
        }

        let Some(cache_app_list) = &paired.cache_app_list else {
            unreachable!()
        };

        Ok(cache_app_list.apps.as_slice())
    }

    pub async fn request_app_image(
        &mut self,
        app_id: u32,
    ) -> Result<C::Bytes, HostError<C::Error>> {
        let https_address = self.https_address().await?;

        let client_info = ClientInfo {
            unique_id: &self.client_unique_id,
            uuid: Uuid::new_v4(),
        };

        let response = host_app_box_art(
            &mut self.client,
            &https_address,
            client_info,
            ClientAppBoxArtRequest { app_id },
        )
        .await?;

        Ok(response)
    }

    // TODO: add a fn to create / correct streaming info: e.g. width, height, fps

    // TODO: maybe remove bounds on decoder and listener?
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
        connection_listener: impl ConnectionListener + Send + Sync + 'static,
        video_decoder: impl VideoDecoder + Send + Sync + 'static,
        audio_decoder: impl AudioDecoder + Send + Sync + 'static,
    ) -> Result<MoonlightStream, HostError<C::Error>> {
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

        let current_game = self.current_game().await?;

        let client_info = ClientInfo {
            unique_id: &self.client_unique_id,
            uuid: Uuid::new_v4(),
        };

        let rtsp_session_url = if current_game == 0 {
            let launch_response = host_launch(
                instance,
                &mut self.client,
                &https_address,
                client_info,
                request,
            )
            .await?;

            launch_response.rtsp_session_url
        } else {
            let resume_response = host_resume(
                instance,
                &mut self.client,
                &https_address,
                client_info,
                request,
            )
            .await?;

            resume_response.rtsp_session_url
        };

        let app_version = self.version().await?;
        let server_codec_mode_support = self.server_codec_mode_support().await?;
        let gfe_version = self.gfe_version().await?.to_owned();

        let instance_clone = instance.clone();
        let connection = spawn_blocking(move || {
            let server_info = ServerInfo {
                address: &address,
                app_version,
                gfe_version: &gfe_version,
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

            instance_clone.start_connection(
                server_info,
                stream_config,
                connection_listener,
                video_decoder,
                audio_decoder,
            )
        })
        .await
        // TODO: remove unwrap
        .unwrap()?;

        Ok(connection)
    }
}
