use moonlight_common::{
    MoonlightInstance,
    data::{ColorRange, Colorspace},
    high::MoonlightHost,
};
use rcgen::{CertifiedKey, KeyPair};
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
    let signing_key;
    let cert_pem;

    if let Ok(true) = try_exists(key_file).await
        && let Ok(true) = try_exists(crt_file).await
    {
        // Load already valid pairing information
        let key_contents = read_to_string(key_file).await.unwrap();
        signing_key = KeyPair::from_pem(&key_contents).unwrap();

        let crt_contents = read_to_string(crt_file).await.unwrap();
        cert_pem = pem::parse(crt_contents).unwrap();
    } else {
        // Generate new private key and certificate
        let CertifiedKey {
            signing_key: generated_signing_key,
            cert: generated_cert,
        } = rcgen::generate_simple_self_signed(Vec::new()).unwrap();

        signing_key = generated_signing_key;
        cert_pem = pem::parse(generated_cert.pem()).unwrap();
    }

    // Get the current pair state
    let host = host.pair_state().await.map_err(|(_, err)| err).unwrap();

    // See if we're already paired
    let mut host = match host.into_paired() {
        // The host is paired
        Ok(host) => host,
        // The host is unpaired
        Err(host) => {
            // Generate pin for pairing
            let pin = crypto.generate_pin();

            println!("Pin: {pin}, Device Name: {device_name}");

            // Pair to the host
            host.pair(&crypto, &cert_pem, device_name.to_string(), pin)
                .await
                .map_err(|(_, err)| err)
                .unwrap()
        }
    };

    // Save the pair information
    write(key_file, signing_key.serialize_pem()).await.unwrap();
    write(crt_file, cert_pem.to_string()).await.unwrap();

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
