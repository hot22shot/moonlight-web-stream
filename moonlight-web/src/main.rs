use std::{
    fs::{self},
    io::ErrorKind,
    path::Path,
};

use actix_web::{
    App, HttpServer,
    web::{Data, scope},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{api::api_service, auth::AuthGuard, web::web_service};

mod api;
mod api_bindings;
mod auth;
mod web;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let address = "127.0.0.1";
    let port = 8080;

    println!("Starting server on http://{address}:{port}");

    let config = read_or_default::<Config>("./server/config.json");
    if config.credentials == "default" {
        panic!("enter your credentials in the config (server/config.json)");
    }
    let config = Data::new(config);

    HttpServer::new(move || {
        App::new()
            .app_data(config.clone())
            .service(scope("/api").guard(AuthGuard).service(api_service()))
            .service(web_service())
    })
    .bind((address, port))?
    .run()
    .await
}

fn read_or_default<T>(path: impl AsRef<Path>) -> T
where
    T: DeserializeOwned + Serialize + Default,
{
    match fs::read_to_string(path.as_ref()) {
        Ok(value) => serde_json::from_str(&value).expect("invalid file"),
        Err(err) if err.kind() == ErrorKind::NotFound => {
            let value = T::default();

            let value_str = serde_json::to_string_pretty(&value).expect("failed to serialize file");

            if let Some(parent) = path.as_ref().parent() {
                fs::create_dir_all(parent).expect("failed to create directories to file");
            }
            fs::write(path.as_ref(), value_str).expect("failed to write default file");

            value
        }
        Err(err) => panic!("failed to read file: {err}"),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    credentials: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            credentials: "default".to_string(),
        }
    }
}
