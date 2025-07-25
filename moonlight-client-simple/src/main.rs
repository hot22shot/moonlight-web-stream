use moonlight_common::{
    MoonlightInstance,
    data::{ColorRange, Colorspace},
    high::MoonlightHost,
};

#[tokio::main]
async fn main() {
    let host_ip = "localhost";
    let device_name = "TestDevice";

    let moonlight = MoonlightInstance::global().unwrap();
    let crypto = moonlight.crypto();

    // Create a host
    // - http_port = None -> Default Port
    // - client_info = None -> Generates a client
    let host = MoonlightHost::new(host_ip.to_string(), None, None);

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
            host.pair(&crypto, pin, device_name.to_string())
                .await
                .map_err(|(_, err)| err)
                .unwrap()
        }
    };

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
    stream.stop();
}
