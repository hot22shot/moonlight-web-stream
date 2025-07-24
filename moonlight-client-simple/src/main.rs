use moonlight_common::{
    MoonlightInstance,
    data::{
        ColorRange, Colorspace, EncryptionFlags, ServerInfo, StreamConfiguration, StreamingConfig,
        SupportedVideoFormats,
    },
    host::{
        network::{
            ClientInfo, ClientPairChallengeRequest, ClientPairRequest, ClientStreamRequest,
            PairStatus, get_host_apps, get_host_info, host_pair_challenge, host_pair_initiate,
            launch_host,
        },
        pair::{PairPin, generate_client_cert_pem, generate_salt},
    },
};
use rand::random;
use tokio::task::spawn_blocking;

#[tokio::main]
async fn main() {
    let host_ip = "localhost";
    let device_name = "TestDevice";
    let client_info = ClientInfo::default();

    println!("-- Host Details");
    let http_address = format!("{host_ip}:47989");
    let host_info = get_host_info(false, &http_address, Some(client_info))
        .await
        .unwrap();
    let https_address = format!("{host_ip}:{}", host_info.https_port);
    println!("{host_info:#?}");

    println!("- Stage: Pairing");

    println!("-- Initiate Pairing");
    let pin = PairPin::random();
    println!("Pin {pin}, Device Name: {device_name}");

    // TODO: read already paired information
    let salt = generate_salt();
    let client_cert_pem = generate_client_cert_pem();

    if true {
        let pair_response = host_pair_initiate(
            &http_address,
            client_info,
            ClientPairRequest {
                device_name,
                salt,
                client_cert_pem,
            },
        )
        .await
        .unwrap();
        println!("{pair_response:#?}");

        assert_eq!(
            pair_response.paired,
            PairStatus::Paired,
            "Please try again and pair the client using the given values"
        );
        let Some(cert) = pair_response.cert else {
            panic!("Paired whilst another device was pairing!");
        };

        //         let challenge: [] = random();
        //
        //         let challenge_response = host_pair_challenge(&http_address, client_info, ClientPairChallengeRequest {
        // encrypted_challenge:
        //         })
        //             .await
        //             .unwrap();
    }

    println!("-- ");

    println!("-- Host Details Secure");
    let host_info = get_host_info(true, &https_address, Some(client_info))
        .await
        .unwrap();
    println!("{host_info:#?}");

    println!("-- Host Apps");
    let host_apps = get_host_apps(&https_address, client_info).await.unwrap();
    println!("{host_apps:#?}");

    println!("- Stage: Streaming");
    println!("-- Host Launch");
    let moonlight = MoonlightInstance::global().unwrap();

    let launch_response = launch_host(
        &moonlight,
        &https_address,
        client_info,
        ClientStreamRequest {
            app_id: 0,
            mode_width: 1000,
            mode_height: 1000,
            mode_fps: 60,
            ri_key: [0u8; 16],
            ri_key_id: [0u8; 16],
        },
    )
    .await
    .unwrap();
    println!("{launch_response:#?}");

    let connection = spawn_blocking(move || {
        let server_info = ServerInfo {
            address: "127.0.0.1:47989",
            app_version: &host_info.app_version,
            gfe_version: &host_info.gfe_version,
            rtsp_session_url: &launch_response.rtsp_session_url,
            server_codec_mode_support: host_info.server_codec_mode_support as i32,
        };

        let stream_config = StreamConfiguration {
            width: 1000,
            height: 1000,
            fps: 60,
            bitrate: 10,
            packet_size: 1024,
            streaming_remotely: StreamingConfig::Remote,
            audio_configuration: 0,
            supported_video_formats: SupportedVideoFormats::default(),
            client_refresh_rate_x100: 60,
            color_space: Colorspace::Rec2020,
            color_range: ColorRange::Full,
            encryption_flags: EncryptionFlags::all(),
            remote_input_aes_key: [0u8; 16usize],
            remote_input_aes_iv: [0u8; 16usize],
        };

        moonlight
            .start_connection(server_info, stream_config)
            .unwrap()
    })
    .await
    .unwrap();

    println!("-- Host Features");
    let host_features = connection.host_features();
    println!("{host_features:?}");

    connection.stop();
}
