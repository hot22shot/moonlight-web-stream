use std::sync::{Mutex, RwLock};

use moonlight_common::{
    MoonlightInstance,
    crypto::MoonlightCrypto,
    high::{MaybePaired, MoonlightHost},
    pair::high::ClientAuth,
};
use serde::{Deserialize, Serialize};
use slab::Slab;

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
#[derive(Debug, Serialize, Deserialize)]
struct PairedHost {
    client_private_key: String,
    client_certificate: String,
    server_certificate: String,
}

// TODO: async aware rwlock and mutex
pub struct RuntimeApiData {
    pub(crate) instance: MoonlightInstance,
    pub(crate) crypto: MoonlightCrypto,
    pub(crate) hosts: RwLock<Slab<Mutex<MoonlightHost<MaybePaired>>>>,
}

impl RuntimeApiData {
    pub async fn load(data: ApiData, instance: MoonlightInstance) -> Self {
        // TODO: do this concurrently
        let mut hosts = Slab::new();
        for host_data in data.hosts {
            // TODO: warn on continue

            let host = MoonlightHost::new(host_data.address, host_data.http_port, None);
            let host = try_pair_state(host, host_data.paired.as_ref()).await;

            hosts.insert(Mutex::new(host));
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
        // TODO: print error
        return host.maybe_paired();
    };
    let Ok(client_certificate) = pem::parse(&paired.client_certificate) else {
        // TODO: print error
        return host.maybe_paired();
    };
    let Ok(server_certificate) = pem::parse(&paired.server_certificate) else {
        // TODO: print error
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
            // TODO: print error
            host.maybe_paired()
        }
    }
}
