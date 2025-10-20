//!
//! The high level api of the moonlight wrapper
//!

use std::{
    io,
    net::{Ipv4Addr, SocketAddrV4},
    sync::atomic::{AtomicBool, Ordering},
};

use pem::Pem;
use tokio::{
    net::UdpSocket,
    sync::{Mutex, RwLock},
    task::JoinError,
};
use uuid::Uuid;

use crate::{
    Error, MoonlightError, PairPin, PairStatus, ServerState, ServerVersion,
    mac::MacAddress,
    network::{
        ApiError, App, ClientAppBoxArtRequest, ClientInfo, DEFAULT_UNIQUE_ID, HostInfo,
        ServerAppListResponse, host_app_box_art, host_app_list, host_cancel, host_info,
        pair::host_unpair, request_client::RequestClient,
    },
    pair::{ClientAuth, PairError, PairSuccess, host_pair},
};

pub async fn broadcast_magic_packet(mac: MacAddress) -> Result<(), io::Error> {
    let mut magic_packet = [0u8; 6 * 17];

    magic_packet[0..6].copy_from_slice(&[255, 255, 255, 255, 255, 255]);
    for i in 1..17 {
        magic_packet[(i * 6)..((i + 1) * 6)].copy_from_slice(&mac.to_bytes());
    }

    let broadcast = SocketAddrV4::new(Ipv4Addr::new(255, 255, 255, 255), 9);

    let socket = UdpSocket::bind("0.0.0.0:0").await?;

    socket.set_broadcast(true)?;
    socket.send_to(&magic_packet, &broadcast).await?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum HostError<RequestError> {
    #[error("{0}")]
    Moonlight(#[from] MoonlightError),
    #[error("{0}")]
    BlockingJoin(#[from] JoinError),
    #[error("this action requires pairing")]
    NotPaired,
    #[error("{0}")]
    Api(#[from] ApiError<RequestError>),
    #[error("{0}")]
    Pair(#[from] PairError<RequestError>),
    #[error("{0}")]
    StreamConfig(#[from] StreamConfigError),
    #[error("the host is likely offline")]
    LikelyOffline,
}

#[derive(Debug, Error)]
pub enum StreamConfigError {
    #[error("hdr not supported")]
    NotSupportedHdr,
    #[error("4k not supported")]
    NotSupported4k,
    #[error("4k not supported: Your device must support HEVC or AV1 to stream at 4k")]
    NotSupported4kCodecMissing,
    #[error("4k not supported: Update GeForce Experience")]
    NotSupported4kUpdateGfe,
}

pub struct MoonlightHost<Client> {
    client_unique_id: String,
    client: RwLock<Client>,
    address: String,
    http_port: u16,
    cache: Cache,
    // Paired
    paired: Mutex<Option<PairInfo>>,
}

#[derive(Debug, Default)]
struct Cache {
    tried_connect: AtomicBool,
    info: RwLock<Option<HostInfo>>,
    app_list: RwLock<Option<ServerAppListResponse>>,
}

#[derive(Clone)]
pub struct PairInfo {
    pub client_private_key: Pem,
    pub client_certificate: Pem,
    pub server_certificate: Pem,
}

impl<C> MoonlightHost<C>
where
    C: RequestClient + Clone,
{
    pub fn new(
        address: String,
        http_port: u16,
        unique_id: Option<String>,
    ) -> Result<Self, HostError<C::Error>> {
        Ok(Self {
            client: RwLock::new(
                C::with_defaults().map_err(|err| HostError::Api(ApiError::RequestClient(err)))?,
            ),
            client_unique_id: unique_id.unwrap_or_else(|| DEFAULT_UNIQUE_ID.to_string()),
            address,
            http_port,
            cache: Default::default(),
            paired: Default::default(),
        })
    }

    async fn client(&self) -> C {
        self.client.read().await.clone()
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

    async fn host_info<R>(&self, f: impl FnOnce(&HostInfo) -> R) -> Result<R, HostError<C::Error>> {
        let has_cache = { self.cache.info.read().await.is_some() };
        let mut https_port = None;

        if self.cache.tried_connect.load(Ordering::Acquire) && !has_cache {
            return Err(HostError::LikelyOffline);
        }
        self.cache.tried_connect.store(true, Ordering::Release);

        let mut client = self.client().await;

        if !has_cache {
            let http_address = self.http_address();

            let client_info = ClientInfo {
                unique_id: &self.client_unique_id,
                uuid: Uuid::new_v4(),
            };

            let info = host_info(&mut client, false, &http_address, Some(client_info)).await?;

            https_port = Some(info.https_port);

            *self.cache.info.write().await = Some(info);
        }
        if !has_cache
            && let Some(https_port) = https_port
            && self.is_paired().await == PairStatus::Paired
        {
            let https_address = Self::build_https_address(&self.address, https_port);

            let client_info = ClientInfo {
                unique_id: &self.client_unique_id,
                uuid: Uuid::new_v4(),
            };

            *self.cache.info.write().await =
                Some(host_info(&mut client, true, &https_address, Some(client_info)).await?);
        }

        let info = self.cache.info.read().await;
        let Some(info) = info.as_ref() else {
            // TODO: some other error
            return Err(HostError::LikelyOffline);
        };

        Ok(f(info))
    }
    pub async fn clear_cache(&self) {
        // TODO: parallel?
        self.cache.tried_connect.store(false, Ordering::Release);
        {
            let mut info = self.cache.info.write().await;
            info.take();
        }
        {
            let mut app_list = self.cache.app_list.write().await;
            app_list.take();
        }
    }

    pub async fn https_port(&self) -> Result<u16, HostError<C::Error>> {
        self.host_info(|info| info.https_port).await
    }

    fn build_https_address(address: &str, https_port: u16) -> String {
        format!("{address}:{https_port}")
    }
    pub async fn https_address(&self) -> Result<String, HostError<C::Error>> {
        let https_port = self.https_port().await?;
        Ok(Self::build_https_address(&self.address, https_port))
    }
    pub async fn external_port(&self) -> Result<u16, HostError<C::Error>> {
        self.host_info(|info| info.external_port).await
    }

    pub async fn host_name(&self) -> Result<String, HostError<C::Error>> {
        self.host_info(|info| info.host_name.to_string()).await
    }
    pub async fn version(&self) -> Result<ServerVersion, HostError<C::Error>> {
        self.host_info(|info| info.app_version).await
    }

    pub async fn gfe_version(&self) -> Result<String, HostError<C::Error>> {
        self.host_info(|info| info.gfe_version.to_string()).await
    }
    pub async fn unique_id(&self) -> Result<Uuid, HostError<C::Error>> {
        self.host_info(|info| info.unique_id).await
    }

    /// Returns None if unpaired
    pub async fn mac(&self) -> Result<Option<MacAddress>, HostError<C::Error>> {
        self.host_info(|info| info.mac).await
    }
    pub async fn local_ip(&self) -> Result<String, HostError<C::Error>> {
        self.host_info(|info| info.local_ip.to_string()).await
    }

    pub async fn current_game(&self) -> Result<u32, HostError<C::Error>> {
        self.host_info(|info| info.current_game).await
    }

    pub async fn state(&self) -> Result<(String, ServerState), HostError<C::Error>> {
        self.host_info(|info| (info.state_string.to_string(), info.state))
            .await
    }
    pub async fn is_nvidia_software(&self) -> Result<bool, HostError<C::Error>> {
        let (state_str, _) = self.state().await?;
        // Real Nvidia host software (GeForce Experience and RTX Experience) both use the 'Mjolnir'
        // codename in the state field and no version of Sunshine does. We can use this to bypass
        // some assumptions about Nvidia hardware that don't apply to Sunshine hosts.
        Ok(state_str.contains("Mjolnir"))
    }

    pub async fn max_luma_pixels_hevc(&self) -> Result<u32, HostError<C::Error>> {
        self.host_info(|info| info.max_luma_pixels_hevc).await
    }
    pub async fn server_codec_mode_support_raw(&self) -> Result<u32, HostError<C::Error>> {
        self.host_info(|info| info.server_codec_mode_support).await
    }

    #[cfg(feature = "stream")]
    pub async fn server_codec_mode_support(
        &self,
    ) -> Result<crate::stream::bindings::ServerCodeModeSupport, HostError<C::Error>> {
        use crate::stream::bindings::ServerCodeModeSupport;

        let bits = self.server_codec_mode_support_raw().await?;
        Ok(ServerCodeModeSupport::from_bits(bits).expect("valid server code mode support"))
    }

    pub async fn set_pair_info(&self, pair_info: PairInfo) -> Result<(), HostError<C::Error>> {
        let mut paired = self.paired.lock().await;

        *self.client.write().await = C::with_certificates(
            &pair_info.client_private_key,
            &pair_info.client_certificate,
            &pair_info.server_certificate,
        )
        .map_err(ApiError::RequestClient)?;

        *paired = Some(pair_info);

        Ok(())
    }

    pub async fn verify_paired(&self) -> Result<PairStatus, HostError<C::Error>> {
        let https_address = match self.https_address().await {
            Err(err) => return Err(err),
            Ok(value) => value,
        };

        let client_info = ClientInfo {
            unique_id: &self.client_unique_id,
            uuid: Uuid::new_v4(),
        };

        let mut client = self.client().await;
        let info = host_info(&mut client, true, &https_address, Some(client_info)).await?;

        let pair_status = info.pair_status;

        Ok(pair_status)
    }

    pub async fn clear_pair_info(&self) -> Result<(), HostError<C::Error>> {
        let mut paired = self.paired.lock().await;

        *self.client.write().await = C::with_defaults().map_err(ApiError::RequestClient)?;
        *paired = None;

        Ok(())
    }

    pub async fn is_paired(&self) -> PairStatus {
        if self.paired.lock().await.is_some() {
            PairStatus::Paired
        } else {
            PairStatus::NotPaired
        }
    }

    pub async fn pair_info(&self) -> Option<PairInfo> {
        let paired = self.paired.lock().await;
        (*paired).clone()
    }

    pub async fn pair(
        &self,
        auth: &ClientAuth,
        device_name: String,
        pin: PairPin,
    ) -> Result<(), HostError<C::Error>> {
        let http_address = self.http_address();
        let server_version = self.version().await?;

        let client_info = ClientInfo {
            unique_id: &self.client_unique_id,
            uuid: Uuid::new_v4(),
        };

        let mut client = C::with_defaults_long_timeout().map_err(ApiError::RequestClient)?;

        let PairSuccess { server_certificate } = host_pair(
            &mut client,
            &http_address,
            client_info,
            &auth.private_key,
            &auth.certificate,
            &device_name,
            server_version,
            pin,
        )
        .await?;

        self.set_pair_info(PairInfo {
            client_private_key: auth.private_key.clone(),
            client_certificate: auth.certificate.clone(),
            server_certificate,
        })
        .await?;
        self.clear_cache().await;

        self.check_paired().await?;

        Ok(())
    }

    async fn check_paired(&self) -> Result<(), HostError<C::Error>> {
        if self.is_paired().await == PairStatus::Paired {
            Ok(())
        } else {
            Err(HostError::NotPaired)
        }
    }

    pub async fn unpair(&self) -> Result<(), HostError<C::Error>> {
        self.check_paired().await?;

        let http_address = self.http_address();
        let client_info = ClientInfo {
            unique_id: &self.client_unique_id,
            uuid: Uuid::new_v4(),
        };

        host_unpair(&mut self.client().await, &http_address, client_info).await?;

        self.clear_pair_info().await?;

        Ok(())
    }

    pub async fn app_list(&self) -> Result<Vec<App>, HostError<C::Error>> {
        self.check_paired().await?;

        let https_address = self.https_address().await?;
        let client_info = ClientInfo {
            unique_id: &self.client_unique_id,
            uuid: Uuid::new_v4(),
        };

        let app_list = self.cache.app_list.read().await;

        // Recache
        match app_list.as_ref() {
            None => {
                drop(app_list);

                let response =
                    host_app_list(&mut self.client().await, &https_address, client_info).await?;

                *self.cache.app_list.write().await = Some(response.clone());

                Ok(response.apps)
            }
            Some(app_list) => Ok(app_list.apps.clone()),
        }
    }

    pub async fn request_app_image(&self, app_id: u32) -> Result<C::Bytes, HostError<C::Error>> {
        self.check_paired().await?;

        let https_address = self.https_address().await?;

        let client_info = ClientInfo {
            unique_id: &self.client_unique_id,
            uuid: Uuid::new_v4(),
        };

        let response = host_app_box_art(
            &mut self.client().await,
            &https_address,
            client_info,
            ClientAppBoxArtRequest { app_id },
        )
        .await?;

        Ok(response)
    }

    pub async fn cancel(&self) -> Result<bool, HostError<C::Error>> {
        self.check_paired().await?;

        let https_hostport = self.https_address().await?;

        let client_info = ClientInfo {
            unique_id: &self.client_unique_id,
            uuid: Uuid::new_v4(),
        };

        let response = host_cancel(&mut self.client().await, &https_hostport, client_info).await?;

        self.clear_cache().await;

        let current_game = self.current_game().await?;
        if current_game != 0 {
            // We're not the device that opened this session
            return Ok(false);
        }

        Ok(response)
    }
}

#[cfg(feature = "stream")]
mod stream {
    use openssl::rand::rand_bytes;
    use tokio::task::spawn_blocking;
    use uuid::Uuid;

    use crate::{
        high::{HostError, MoonlightHost, StreamConfigError},
        network::{
            ClientInfo,
            launch::{ClientStreamRequest, host_launch, host_resume},
            request_client::RequestClient,
        },
        pair::PairError,
        stream::{
            MoonlightInstance, MoonlightStream, ServerInfo,
            audio::AudioDecoder,
            bindings::{
                ActiveGamepads, ColorRange, Colorspace, EncryptionFlags, ServerCodeModeSupport,
                StreamConfiguration, StreamingConfig, SupportedVideoFormats,
            },
            connection::ConnectionListener,
            video::VideoDecoder,
        },
    };

    impl<C> MoonlightHost<C>
    where
        C: RequestClient + Clone,
    {
        // Stream config correction
        pub async fn is_hdr_supported(&self) -> Result<bool, HostError<C::Error>> {
            let server_codec_mode_support = self.server_codec_mode_support().await?;

            Ok(
                server_codec_mode_support.contains(ServerCodeModeSupport::HEVC_MAIN10)
                    || server_codec_mode_support.contains(ServerCodeModeSupport::AV1_MAIN10),
            )
        }
        pub async fn is_4k_supported(&self) -> Result<bool, HostError<C::Error>> {
            let is_nvidia = self.is_nvidia_software().await?;
            let server_codec_mode_support = self.server_codec_mode_support().await?;

            Ok(
                server_codec_mode_support.contains(ServerCodeModeSupport::HEVC_MAIN10)
                    || !is_nvidia,
            )
        }
        pub async fn is_4k_supported_gfe(&self) -> Result<bool, HostError<C::Error>> {
            let gfe = self.gfe_version().await?;

            Ok(!gfe.starts_with("2."))
        }

        pub async fn is_resolution_supported(
            &self,
            width: usize,
            height: usize,
            supported_video_formats: SupportedVideoFormats,
        ) -> Result<(), HostError<C::Error>> {
            let resolution_above_4k = width > 4096 || height > 4096;

            if resolution_above_4k && !self.is_4k_supported().await? {
                return Err(StreamConfigError::NotSupported4k.into());
            } else if resolution_above_4k
                && supported_video_formats.contains(!SupportedVideoFormats::MASK_H264)
            {
                return Err(StreamConfigError::NotSupported4kCodecMissing.into());
            } else if height > 2160 && self.is_4k_supported_gfe().await? {
                return Err(StreamConfigError::NotSupported4kUpdateGfe.into());
            }

            Ok(())
        }

        pub async fn should_disable_sops(
            &self,
            width: usize,
            height: usize,
        ) -> Result<bool, HostError<C::Error>> {
            // Using an unsupported resolution (not 720p, 1080p, or 4K) causes
            // GFE to force SOPS to 720p60. This is fine for < 720p resolutions like
            // 360p or 480p, but it is not ideal for 1440p and other resolutions.
            // When we detect an unsupported resolution, disable SOPS unless it's under 720p.
            // FIXME: Detect support resolutions using the serverinfo response, not a hardcoded list
            const NVIDIA_SUPPORTED_RESOLUTIONS: &[(usize, usize)] =
                &[(1280, 720), (1920, 1080), (3840, 2160)];

            let is_nvidia = self.is_nvidia_software().await?;

            Ok(!NVIDIA_SUPPORTED_RESOLUTIONS.contains(&(width, height)) && is_nvidia)
        }

        pub async fn start_stream(
            &mut self,
            instance: &MoonlightInstance,
            app_id: u32,
            width: u32,
            height: u32,
            mut fps: u32,
            hdr: bool,
            mut sops: bool,
            local_audio_play_mode: bool,
            gamepads_attached: ActiveGamepads,
            gamepads_persist_after_disconnect: bool,
            color_space: Colorspace,
            color_range: ColorRange,
            bitrate: u32,
            packet_size: u32,
            connection_listener: impl ConnectionListener + Send + Sync + 'static,
            video_decoder: impl VideoDecoder + Send + Sync + 'static,
            audio_decoder: impl AudioDecoder + Send + Sync + 'static,
        ) -> Result<MoonlightStream, HostError<C::Error>> {
            // Change streaming options if required

            if hdr && !self.is_hdr_supported().await? {
                return Err(HostError::StreamConfig(StreamConfigError::NotSupportedHdr));
            }

            self.is_resolution_supported(
                width as usize,
                height as usize,
                video_decoder.supported_formats(),
            )
            .await?;

            if self.is_nvidia_software().await? {
                // Using an FPS value over 60 causes SOPS to default to 720p60,
                // so force it to 0 to ensure the correct resolution is set. We
                // used to use 60 here but that locked the frame rate to 60 FPS
                // on GFE 3.20.3. We don't need this hack for Sunshine.
                if fps > 60 {
                    fps = 0;
                }

                if self
                    .should_disable_sops(width as usize, height as usize)
                    .await?
                {
                    sops = false;
                }
            }

            // Clearing cache so we refresh and can see if there's a game -> launch or resume?
            self.clear_cache().await;

            let address = self.address.clone();
            let https_address = self.https_address().await?;

            let current_game = self.current_game().await?;

            let mut aes_key = [0u8; 16];
            rand_bytes(&mut aes_key).map_err(PairError::from)?;

            let mut aes_iv = [0u8; 4];
            rand_bytes(&mut aes_iv).map_err(PairError::from)?;
            let aes_iv = i32::from_be_bytes(aes_iv);

            let request = ClientStreamRequest {
                app_id,
                mode_width: width,
                mode_height: height,
                mode_fps: fps,
                hdr,
                sops,
                local_audio_play_mode,
                gamepads_attached_mask: gamepads_attached.bits() as i32,
                gamepads_persist_after_disconnect,
                ri_key: aes_key,
                ri_key_id: aes_iv,
            };

            let client_info = ClientInfo {
                unique_id: &self.client_unique_id,
                uuid: Uuid::new_v4(),
            };

            let rtsp_session_url = if current_game == 0 {
                let launch_response = host_launch(
                    instance,
                    &mut self.client().await,
                    &https_address,
                    client_info,
                    request,
                )
                .await?;

                launch_response.rtsp_session_url
            } else {
                let resume_response = host_resume(
                    instance,
                    &mut self.client().await,
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
                    encryption_flags: EncryptionFlags::empty(),
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
            .await??;

            // Clear cache because now there's an active app
            self.clear_cache().await;

            Ok(connection)
        }
    }
}
