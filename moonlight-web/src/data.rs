use std::{collections::HashMap, path::Path};

use actix_web::web::{Bytes, Data};
use log::warn;
use moonlight_common::{
    PairStatus,
    moonlight::{MoonlightInstance, crypto::MoonlightCrypto},
    network::reqwest::ReqwestMoonlightHost,
    pair::ClientAuth,
};
use serde::{Deserialize, Serialize};
use slab::Slab;
use tokio::{
    fs, spawn,
    sync::{
        Mutex, RwLock,
        mpsc::{Receiver, Sender, channel},
    },
};

use crate::Config;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ApiData {
    hosts: Vec<Host>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Host {
    address: String,
    http_port: u16,
    paired: Option<PairedHost>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedHost {
    pub client_private_key: String,
    pub client_certificate: String,
    pub server_certificate: String,
}

pub struct RuntimeApiHost {
    pub moonlight: ReqwestMoonlightHost,
    pub app_images_cache: HashMap<u32, Bytes>,
}

pub struct RuntimeApiData {
    pub(crate) file_writer: Sender<()>,
    pub(crate) instance: MoonlightInstance,
    pub(crate) crypto: MoonlightCrypto,
    pub(crate) hosts: RwLock<Slab<Mutex<RuntimeApiHost>>>,
}

impl RuntimeApiData {
    pub async fn load(config: &Config, data: ApiData, instance: MoonlightInstance) -> Data<Self> {
        // TODO: do this concurrently
        let mut hosts = Slab::new();
        for host_data in data.hosts {
            let mut host =
                match ReqwestMoonlightHost::new(host_data.address, host_data.http_port, None) {
                    Ok(value) => value,
                    Err(err) => {
                        warn!("[Load]: failed to load host: {err:?}");
                        continue;
                    }
                };
            try_pair_state(&mut host, host_data.paired.as_ref()).await;

            hosts.insert(Mutex::new(RuntimeApiHost {
                moonlight: host,
                app_images_cache: Default::default(),
            }));
        }

        // This channel only requires a capacity of 1:
        // 1. All sender will use try_send after finishing their write operations
        // 2. If the buffer would larger than 1 we would do multiple writes after each other without data changes
        // -> no extra write operations
        let (file_writer, file_writer_receiver) = channel(1);

        let this = Data::new(Self {
            file_writer,
            crypto: instance.crypto(),
            instance,
            hosts: RwLock::new(hosts),
        });

        spawn({
            let path = config.data_path.clone();
            let this = this.clone();

            async move { self::file_writer(file_writer_receiver, path, this).await }
        });

        this
    }

    pub async fn save(&self) -> ApiData {
        let hosts = self.hosts.read().await;

        let mut output = ApiData {
            hosts: Vec::with_capacity(hosts.len()),
        };

        for (_, host) in &*hosts {
            let host = host.lock().await;

            let paired = Self::extract_paired(&host.moonlight);

            output.hosts.push(Host {
                address: host.moonlight.address().to_string(),
                http_port: host.moonlight.http_port(),
                paired,
            });
        }

        output
    }
    fn extract_paired(host: &ReqwestMoonlightHost) -> Option<PairedHost> {
        let client_private_key = host.client_private_key()?;
        let client_certificate = host.client_certificate()?;
        let server_certificate = host.server_certificate()?;

        Some(PairedHost {
            client_private_key: client_private_key.to_string(),
            client_certificate: client_certificate.to_string(),
            server_certificate: server_certificate.to_string(),
        })
    }
}

async fn try_pair_state(host: &mut ReqwestMoonlightHost, paired: Option<&PairedHost>) {
    let Some(paired) = paired else {
        return;
    };

    let Ok(client_private_key) = pem::parse(&paired.client_private_key) else {
        warn!(
            "failed to parse client private key as pem. Client: {}",
            host.address()
        );
        return;
    };
    let Ok(client_certificate) = pem::parse(&paired.client_certificate) else {
        warn!(
            "failed to parse client certificate as pem. Client: {}",
            host.address()
        );
        return;
    };
    let Ok(server_certificate) = pem::parse(&paired.server_certificate) else {
        warn!(
            "failed to parse server certificate as pem. Client: {}",
            host.address()
        );
        return;
    };

    let status = match host
        .set_pairing_info(
            &ClientAuth {
                key_pair: client_private_key,
                certificate: client_certificate,
            },
            &server_certificate,
        )
        .await
    {
        Ok(value) => value,
        Err(err) => {
            warn!(
                "failed to pair client even though it has pair data: Client: {}, Error: {:?}",
                host.address(),
                err
            );
            return;
        }
    };

    if status != PairStatus::Paired {
        warn!(
            "failed to pair client even though it has pair data: Client: {}",
            host.address(),
        );
    }
}

async fn file_writer(
    mut receiver: Receiver<()>,
    path: impl AsRef<Path>,
    data: Data<RuntimeApiData>,
) {
    loop {
        if receiver.recv().await.is_none() {
            return;
        }

        let data = data.save().await;

        let text = match serde_json::to_string_pretty(&data) {
            Err(err) => {
                warn!("failed to save data: {err:?}");

                continue;
            }
            Ok(value) => value,
        };

        if let Err(err) = fs::write(&path, text).await {
            warn!("failed to save data: {err:?}");
        }
    }
}
