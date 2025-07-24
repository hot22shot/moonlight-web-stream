use std::sync::Arc;

use moonlight_common_sys::crypto::{
    PPLT_CRYPTO_CONTEXT, PltCreateCryptoContext, PltDestroyCryptoContext, PltGenerateRandomData,
};
use sha1::{Digest, Sha1};
use sha2::Sha256;

use crate::{
    Handle, MoonlightInstance,
    host::{
        network::ServerVersion,
        pair::{PairPin, SALT_LENGTH},
    },
};

#[derive(Clone)]
pub(crate) struct CryptoHandle {
    #[allow(unused)]
    handle: Arc<Handle>,
    context: PPLT_CRYPTO_CONTEXT,
}

impl Drop for CryptoHandle {
    fn drop(&mut self) {
        unsafe {
            PltDestroyCryptoContext(self.context);
        }
    }
}

pub struct MoonlightCrypto {
    #[allow(unused)]
    handle: CryptoHandle,
}

impl MoonlightCrypto {
    pub(crate) fn new(instance: &MoonlightInstance) -> Self {
        let context = unsafe { PltCreateCryptoContext() };

        let handle = CryptoHandle {
            handle: instance.handle.clone(),
            context,
        };

        Self { handle }
    }

    pub fn generate_random(&self, bytes: &mut [u8]) {
        unsafe {
            PltGenerateRandomData(bytes.as_mut_ptr(), bytes.len() as i32);
        }
    }

    pub fn generate_pin(&self) -> PairPin {
        let random_number = || {
            let mut byte = [0u8];
            self.generate_random(&mut byte);

            byte[0] % 10
        };

        let n1 = random_number();
        let n2 = random_number();
        let n3 = random_number();
        let n4 = random_number();

        PairPin::from_array([n1, n2, n3, n4]).expect("pair pin")
    }

    pub fn generate_salt(&self) -> [u8; SALT_LENGTH] {
        let mut salt = [0; _];
        self.generate_random(&mut salt);
        salt
    }
    pub fn generate_client_cert_pem(&self) -> [u8; 16] {
        let mut cert = [0; _];
        self.generate_random(&mut cert);
        cert
    }

    pub fn salt_pin(&self, salt: [u8; SALT_LENGTH], pin: PairPin) -> [u8; SALT_LENGTH + 4] {
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
