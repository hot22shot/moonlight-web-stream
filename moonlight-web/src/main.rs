use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};
use std::{
    io::ErrorKind,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    path::Path,
};
use tokio::fs;
use webrtc::ice_transport::ice_server::RTCIceServer;

use actix_web::{App, HttpServer, web::Data};
use log::{LevelFilter, info};
use moonlight_common::moonlight::MoonlightInstance;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use simplelog::{ColorChoice, TermLogger, TerminalMode};

use crate::{
    api::api_service,
    data::{ApiData, RuntimeApiData},
    web::web_service,
};

mod api;
mod api_bindings;
mod api_bindings_consts;
mod data;
mod web;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    TermLogger::init(
        LevelFilter::Debug,
        simplelog::Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )
    .expect("failed to init logger");

    // Load Config
    let config = read_or_default::<Config>("./server/config.json").await;
    if config.credentials == "default" {
        panic!("enter your credentials in the config (server/config.json)");
    }
    let config = Data::new(config);
    let bind_address = config.bind_address;

    // Load Data
    let data = read_or_default::<ApiData>(&config.data_path).await;
    let data = RuntimeApiData::load(
        &config,
        data,
        MoonlightInstance::global().expect("failed to initialize moonlight"),
    )
    .await;

    let server = HttpServer::new({
        let config = config.clone();

        move || {
            App::new()
                .app_data(config.clone())
                .service(api_service(data.clone(), config.credentials.to_string()))
                .service(web_service())
        }
    });

    if let Some(certificate) = config.certificate.as_ref() {
        info!("[Server]: Running Https Server with ssl tls");

        let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls())
            .expect("failed to create ssl tls acceptor");
        builder
            .set_private_key_file(&certificate.private_key_pem, SslFiletype::PEM)
            .expect("failed to set private key");
        builder
            .set_certificate_chain_file(&certificate.certificate_pem)
            .expect("failed to set certificate");

        server.bind_openssl(bind_address, builder)?.run().await
    } else {
        server.bind(bind_address)?.run().await
    }
}

async fn read_or_default<T>(path: impl AsRef<Path>) -> T
where
    T: DeserializeOwned + Serialize + Default,
{
    match fs::read_to_string(path.as_ref()).await {
        Ok(value) => serde_json::from_str(&value).expect("invalid file"),
        Err(err) if err.kind() == ErrorKind::NotFound => {
            let value = T::default();

            let value_str = serde_json::to_string_pretty(&value).expect("failed to serialize file");

            if let Some(parent) = path.as_ref().parent() {
                fs::create_dir_all(parent)
                    .await
                    .expect("failed to create directories to file");
            }
            fs::write(path.as_ref(), value_str)
                .await
                .expect("failed to write default file");

            value
        }
        Err(err) => panic!("failed to read file: {err}"),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    credentials: String,
    #[serde(default = "data_path_default")]
    data_path: String,
    #[serde(default = "default_bind_address")]
    bind_address: SocketAddr,
    #[serde(default = "moonlight_default_http_port_default")]
    moonlight_default_http_port: u16,
    #[serde(default = "default_pair_device_name")]
    pair_device_name: String,
    #[serde(default = "default_ice_servers")]
    webrtc_ice_servers: Vec<RTCIceServer>,
    #[serde(default)]
    webrtc_port_range: Option<PortRange>,
    #[serde(default)]
    webrtc_nat_1to1_ips: Vec<String>,
    certificate: Option<ConfigSsl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSsl {
    private_key_pem: String,
    certificate_pem: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortRange {
    min: u16,
    max: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            credentials: "default".to_string(),
            data_path: data_path_default(),
            bind_address: default_bind_address(),
            moonlight_default_http_port: moonlight_default_http_port_default(),
            webrtc_ice_servers: default_ice_servers(),
            webrtc_port_range: Default::default(),
            webrtc_nat_1to1_ips: Default::default(),
            pair_device_name: default_pair_device_name(),
            certificate: None,
        }
    }
}

fn data_path_default() -> String {
    "server/data.json".to_string()
}

fn default_bind_address() -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8080))
}

fn moonlight_default_http_port_default() -> u16 {
    47989
}

fn default_ice_servers() -> Vec<RTCIceServer> {
    vec![
        RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_owned()],
            ..Default::default()
        },
        RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_owned()],
            ..Default::default()
        },
        RTCIceServer {
            urls: vec!["stun:stun.l.google.com:5349".to_owned()],
            ..Default::default()
        },
        RTCIceServer {
            urls: vec!["stun:stun1.l.google.com:3478".to_owned()],
            ..Default::default()
        },
        RTCIceServer {
            urls: vec!["stun:stun1.l.google.com:5349".to_owned()],
            ..Default::default()
        },
        RTCIceServer {
            urls: vec!["stun:stun2.l.google.com:19302".to_owned()],
            ..Default::default()
        },
        RTCIceServer {
            urls: vec!["stun:stun2.l.google.com:5349".to_owned()],
            ..Default::default()
        },
        RTCIceServer {
            urls: vec!["stun:stun3.l.google.com:3478".to_owned()],
            ..Default::default()
        },
        RTCIceServer {
            urls: vec!["stun:stun3.l.google.com:5349".to_owned()],
            ..Default::default()
        },
        RTCIceServer {
            urls: vec!["stun:stun4.l.google.com:19302".to_owned()],
            ..Default::default()
        },
        RTCIceServer {
            urls: vec!["stun:stun4.l.google.com:5349".to_owned()],
            ..Default::default()
        },
    ]
}

fn default_pair_device_name() -> String {
    "roth".to_string()
}
