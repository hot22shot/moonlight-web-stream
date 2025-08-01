use std::time::Duration;

use moonlight_common::{
    MoonlightInstance,
    debug::{DebugHandler, NullHandler},
    high::MoonlightHost,
    pair::high::{ClientAuth, generate_new_client},
    stream::{ColorRange, Colorspace},
};
use tokio::{
    fs::{read_to_string, try_exists, write},
    task::spawn_blocking,
    time::sleep,
};

use crate::gstreamer::GStreamerVideoHandler;

mod gstreamer;

#[tokio::main]
async fn main() {
    // Configuration
    let host_ip = "127.0.0.1";
    let host_http_port = 47989;
    let device_name = "TestDevice";

    // Configuration Authentication / Pairing
    let key_file = "client.key";
    let crt_file = "client.crt";
    let server_crt_file = "server.crt";

    // Initialize Moonlight
    let moonlight = MoonlightInstance::global().unwrap();
    let crypto = moonlight.crypto();

    // Create a host
    // - client_info = None -> Generates a client
    let host = MoonlightHost::new(host_ip.to_string(), host_http_port, None);

    // Pair to the Host using Generated / Loaded Client Private Key, Certificate and Server Certificate
    let mut host = if let Ok(true) = try_exists(key_file).await
        && let Ok(true) = try_exists(crt_file).await
        && let Ok(true) = try_exists(server_crt_file).await
    {
        // Load already valid pairing information
        let key_contents = read_to_string(key_file).await.unwrap();
        let crt_contents = read_to_string(crt_file).await.unwrap();
        let server_crt_contents = read_to_string(server_crt_file).await.unwrap();

        let auth = ClientAuth {
            key_pair: pem::parse(key_contents).unwrap(),
            certificate: pem::parse(crt_contents).unwrap(),
        };
        let server_certificate = pem::parse(server_crt_contents).unwrap();

        // Get the current pair state
        let host = host
            .pair_state(Some((&auth, &server_certificate)))
            .await
            .map_err(|(_, err)| err)
            .unwrap();

        match host.try_into_paired() {
            Ok(host) => host,
            Err(_) => panic!(
                "host not paired even though we've already generated private key and certificates: Delete them to repair"
            ),
        }
    } else {
        // Generate new client
        let auth = generate_new_client().unwrap();

        // Generate pin for pairing
        let pin = crypto.generate_pin();

        println!("Pin: {pin}, Device Name: {device_name}");

        // Pair to the host
        let host = host
            .into_unpaired()
            .pair(&crypto, &auth, device_name.to_string(), pin)
            .await
            .map_err(|(_, err)| err)
            .unwrap();

        let server_certificate = host.server_certificate().clone();

        // Save the pair information
        write(key_file, auth.key_pair.to_string()).await.unwrap();
        write(crt_file, auth.certificate.to_string()).await.unwrap();
        write(server_crt_file, server_certificate.to_string())
            .await
            .unwrap();

        host
    };

    let apps = host.app_list().await.unwrap();

    println!("The host has {} apps:", apps.len());
    for app in apps {
        println!("- {app:?}");
    }

    assert!(!apps.is_empty(), "The host needs at least one app!");

    let app = &apps[0];
    let app_id = app.id;

    println!("Connecting to the first app: {app:?}");

    // Creating gstreamer stuff
    gstreamer::init();

    let video_decoder = GStreamerVideoHandler::new().unwrap();

    // Start the stream (only 1 stream per program is allowed)
    let stream = host
        .start_stream(
            &moonlight,
            &crypto,
            app_id,
            1920,
            1080,
            60,
            Colorspace::Rec2020,
            ColorRange::Full,
            40,
            1024,
            DebugHandler,
            video_decoder,
            NullHandler,
        )
        .await
        .unwrap();
    println!("Finished Connection");

    sleep(Duration::from_secs(60)).await;

    println!("Closing Connection");

    // Stop the stream (drop will also just close the stream)
    spawn_blocking(move || {
        stream.stop();
    })
    .await
    .unwrap();
    drop(host);

    sleep(Duration::from_secs(2)).await;
}
