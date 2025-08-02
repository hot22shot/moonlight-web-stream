use std::sync::{Mutex, RwLock};

use anyhow::anyhow;
use log::warn;
use moonlight_common::{
    MoonlightInstance,
    crypto::MoonlightCrypto,
    high::{MaybePaired, MoonlightHost},
    pair::high::ClientAuth,
};
use serde::{Deserialize, Serialize};
use slab::Slab;
use tokio::fs;

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
    pub moonlight: MoonlightHost<MaybePaired>,
    pub pair_info: Option<PairedHost>,
}

// TODO: async aware rwlock and mutex
pub struct RuntimeApiData {
    pub(crate) instance: MoonlightInstance,
    pub(crate) crypto: MoonlightCrypto,
    pub(crate) hosts: RwLock<Slab<Mutex<RuntimeApiHost>>>,
}

impl RuntimeApiData {
    pub async fn load(data: ApiData, instance: MoonlightInstance) -> Self {
        // TODO: do this concurrently
        let mut hosts = Slab::new();
        for host_data in data.hosts {
            let host = MoonlightHost::new(host_data.address, host_data.http_port, None);
            let host = try_pair_state(host, host_data.paired.as_ref()).await;

            hosts.insert(Mutex::new(RuntimeApiHost {
                moonlight: host,
                pair_info: host_data.paired,
            }));
        }

        Self {
            crypto: instance.crypto(),
            instance,
            hosts: RwLock::new(hosts),
        }
    }

    pub async fn save(&self) -> ApiData {
        todo!()
    }
}

async fn try_pair_state<Pair>(
    host: MoonlightHost<Pair>,
    paired: Option<&PairedHost>,
) -> MoonlightHost<MaybePaired> {
    let host = host.into_unpaired();

    let Some(paired) = paired else {
        return host.maybe_paired();
    };

    let Ok(client_private_key) = pem::parse(&paired.client_private_key) else {
        warn!(
            "failed to parse client private key as pem. Client: {}",
            host.address()
        );
        return host.maybe_paired();
    };
    let Ok(client_certificate) = pem::parse(&paired.client_certificate) else {
        warn!(
            "failed to parse client certificate as pem. Client: {}",
            host.address()
        );
        return host.maybe_paired();
    };
    let Ok(server_certificate) = pem::parse(&paired.server_certificate) else {
        warn!(
            "failed to parse server certificate as pem. Client: {}",
            host.address()
        );
        return host.maybe_paired();
    };

    match host
        .pair_state(Some((
            &ClientAuth {
                key_pair: client_private_key,
                certificate: client_certificate,
            },
            &server_certificate,
        )))
        .await
    {
        Ok(host) => host,
        Err((host, err)) => {
            warn!(
                "failed to pair client even though it has pair data: Client: {}, Error: {:?}",
                host.address(),
                err
            );
            host.maybe_paired()
        }
    }
}

// TODO: maybe make a seperate thread for syncing the data so we don't get two file writes at once?
pub async fn save_data(config: &Config, data: &RuntimeApiData) -> Result<(), anyhow::Error> {
    let hosts = data.hosts.read().map_err(|err| anyhow!("{err}"))?;

    let mut output = ApiData {
        hosts: Vec::with_capacity(hosts.len()),
    };

    for (_, host) in &*hosts {
        let host = host.lock().map_err(|err| anyhow!("{err}"))?;

        output.hosts.push(Host {
            address: host.moonlight.address().to_string(),
            http_port: host.moonlight.http_port(),
            paired: host.pair_info.clone(),
        });
    }

    drop(hosts);

    let text = serde_json::to_string_pretty(&output)?;

    fs::write(&config.data_path, text).await?;

    Ok(())
}
