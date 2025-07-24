use std::sync::Arc;

use sha1::{Digest, Sha1};
use sha2::Sha256;

use crate::{
    Handle,
    host::{
        network::ServerVersion,
        pair::{PairPin, SALT_LENGTH},
    },
};

#[derive(Clone)]
pub(crate) struct CryptoHandle {
    handle: Arc<Handle>,
}

pub struct MoonlightCrypto {
    handle: CryptoHandle,
}

impl MoonlightCrypto {
    pub fn generate_salt(&self) -> [u8; SALT_LENGTH] {
        rand::random()
    }
    pub fn generate_client_cert_pem(&self) -> [u8; 16] {
        rand::random()
    }

    fn salt_pin(&self, salt: [u8; SALT_LENGTH], pin: PairPin) -> [u8; SALT_LENGTH + 4] {
        let mut out = [0u8; SALT_LENGTH + 4];

        out[0..16].copy_from_slice(&salt);
        out[16..].copy_from_slice(&pin.array());

        out
    }
    pub fn generate_aes_key(
        &self,
        server_version: ServerVersion,
        salt: [u8; SALT_LENGTH],
        pin: PairPin,
    ) -> [u8; 16] {
        let mut hash = [0u8; 16];

        let salted = self.salt_pin(salt, pin);

        if server_version.major >= 7 {
            let digest = Sha256::digest(salted);
            hash.copy_from_slice(&digest[0..16]);
        } else {
            let digest = Sha1::digest(salted);
            hash.copy_from_slice(&digest[0..16]);
        }

        hash
    }
}
