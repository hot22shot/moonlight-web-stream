use moonlight_common::{
    MoonlightInstance,
    data::{ColorRange, Colorspace},
    high::MoonlightHost,
    pair::high::{ClientAuth, generate_new_client},
};
use rcgen::{Certificate, KeyPair};
use tokio::{
    fs::{read_to_string, try_exists, write},
    task::spawn_blocking,
};

#[tokio::main]
async fn main() {
    // Configuration
    let host_ip = "localhost";
    let host_http_port = 47989;
    let device_name = "TestDevice";

    // Configuration Authentication / Pairing
    let key_file = "client.key";
    let crt_file = "client.crt";

    // Initialize Moonlight
    let moonlight = MoonlightInstance::global().unwrap();
    let crypto = moonlight.crypto();

    // Create a host
    // - client_info = None -> Generates a client
    let host = MoonlightHost::new(host_ip.to_string(), host_http_port, None);

    // Load or Create a key pair and certificate
    let auth;

    if let Ok(true) = try_exists(key_file).await
        && let Ok(true) = try_exists(crt_file).await
    {
        // Load already valid pairing information
        let key_contents = read_to_string(key_file).await.unwrap();
        let crt_contents = read_to_string(crt_file).await.unwrap();

        auth = ClientAuth {
            key_pair: pem::parse(key_contents).unwrap(),
            certificate: pem::parse(crt_contents).unwrap(),
        };
    } else {
        auth = generate_new_client().unwrap();
    }

    assert_eq!(auth.key_pair.tag(), "PRIVATE KEY");
    assert_eq!(auth.certificate.tag(), "CERTIFICATE");

    // Get the current pair state
    let host = host
        .pair_state(Some(&auth))
        .await
        .map_err(|(_, err)| err)
        .unwrap();

    // See if we're already paired
    let mut host = match host.into_paired() {
        // The host is paired
        Ok(host) => host,
        // The host is unpaired
        Err(host) => {
            unreachable!(); // TODO: <--- remove

            // Generate pin for pairing
            let pin = crypto.generate_pin();

            println!("Pin: {pin}, Device Name: {device_name}");

            // Pair to the host
            host.pair(&crypto, &auth, device_name.to_string(), pin)
                .await
                .map_err(|(_, err)| err)
                .unwrap()
        }
    };

    // Save the pair information
    write(key_file, auth.key_pair.to_string()).await.unwrap();
    write(crt_file, auth.certificate.to_string()).await.unwrap();

    // Start the stream (only 1 stream per program is allowed)
    let stream = host
        .start_stream(
            &moonlight,
            0,
            1000,
            1000,
            60,
            Colorspace::Rec2020,
            ColorRange::Full,
        )
        .await
        .unwrap();

    // Stop the stream (drop will also just close the stream)
    spawn_blocking(move || {
        stream.stop();
    })
    .await
    .unwrap();
}
