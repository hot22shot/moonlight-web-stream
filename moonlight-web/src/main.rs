use std::{
    io::ErrorKind,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    path::Path,
};
use tokio::fs;

use actix_web::{
    App, HttpServer, middleware,
    web::{Data, scope},
};
use log::LevelFilter;
use moonlight_common::MoonlightInstance;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use simplelog::{ColorChoice, TermLogger, TerminalMode};

use crate::{
    api::api_service,
    auth::auth_middleware,
    data::{ApiData, RuntimeApiData},
    web::web_service,
};

mod api;
mod api_bindings;
mod api_bindings_consts;
mod auth;
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

    // Get socket address

    HttpServer::new(move || {
        App::new()
            .app_data(config.clone())
            .service(
                scope("/api")
                    .app_data(data.clone())
                    .wrap(middleware::from_fn(auth_middleware))
                    .service(api_service()),
            )
            .service(web_service())
    })
    .bind(bind_address)?
    .run()
    .await
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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            credentials: "default".to_string(),
            data_path: data_path_default(),
            bind_address: default_bind_address(),
            moonlight_default_http_port: moonlight_default_http_port_default(),
            pair_device_name: default_pair_device_name(),
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

fn default_pair_device_name() -> String {
    "roth".to_string()
}
