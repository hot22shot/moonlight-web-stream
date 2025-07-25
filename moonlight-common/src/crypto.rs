use std::{ptr::null_mut, sync::Arc};

use moonlight_common_sys::crypto::{
    ALGORITHM_AES_CBC, ALGORITHM_AES_GCM, CIPHER_FLAG_FINISH, CIPHER_FLAG_PAD_TO_BLOCK_SIZE,
    CIPHER_FLAG_RESET_IV, PPLT_CRYPTO_CONTEXT, PltCreateCryptoContext, PltDecryptMessage,
    PltDestroyCryptoContext, PltEncryptMessage, PltGenerateRandomData,
};
use sha1::{Digest, Sha1};
use sha2::Sha256;
use thiserror::Error;

use crate::{
    Handle, MoonlightInstance, flag_if,
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

pub const AES_BLOCK_SIZE: usize = 16;

#[derive(Debug, Error)]
#[error("error with moonlight crypto")]
pub struct CryptoError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptoAlgorithm {
    AesCbc,
    AesGcm,
}

impl CryptoAlgorithm {
    fn raw(&self) -> i32 {
        match self {
            Self::AesCbc => ALGORITHM_AES_CBC as i32,
            Self::AesGcm => ALGORITHM_AES_GCM as i32,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CipherFlags {
    reset: bool,
    finish: bool,
    pad_to_block_size: bool,
}

impl CipherFlags {
    fn raw(&self) -> i32 {
        let mut flags = 0x0;

        // TODO: others
        flag_if(&mut flags, CIPHER_FLAG_RESET_IV, self.reset);
        flag_if(&mut flags, CIPHER_FLAG_FINISH, self.finish);
        flag_if(
            &mut flags,
            CIPHER_FLAG_PAD_TO_BLOCK_SIZE,
            self.pad_to_block_size,
        );

        flags as i32
    }
}

pub struct MoonlightCrypto {
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

    pub fn encrypt_message(
        &self,
        algorithm: CryptoAlgorithm,
        flags: CipherFlags,
        key: &[u8],
        iv: &[u8],
        tag: Option<&[u8]>,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, CryptoError> {
        let mut expected_output_len = input.len();

        if flags.pad_to_block_size {
            expected_output_len = ((input.len() / AES_BLOCK_SIZE) + 1) * AES_BLOCK_SIZE;
        } else {
            // Without padding, input must be block-aligned
            assert!(
                input.len().is_multiple_of(AES_BLOCK_SIZE),
                "Input length must be multiple of 16 when no padding is used"
            );
        }

        if algorithm == CryptoAlgorithm::AesGcm {
            expected_output_len += 16; // Tag size
        }

        assert!(
            output.len() >= expected_output_len,
            "Output buffer is too small: need {}, got {}",
            expected_output_len,
            output.len()
        );

        let mut output_len = 0;

        unsafe {
            if !PltEncryptMessage(
                self.handle.context,
                algorithm.raw(),
                flags.raw(),
                key.as_ptr() as *mut _,
                key.len() as i32,
                iv.as_ptr() as *mut _,
                iv.len() as i32,
                tag.map(|tag| tag.as_ptr() as *mut _).unwrap_or(null_mut()),
                tag.map(|tag| tag.len()).unwrap_or(0) as i32,
                input.as_ptr() as *mut _,
                input.len() as i32,
                output.as_mut_ptr(),
                &mut output_len as *mut _,
            ) {
                return Err(CryptoError);
            }
        }

        Ok(output_len as usize)
    }
    pub fn encrypt_aes(
        &self,
        key: &[u8],
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, CryptoError> {
        let iv = [0u8; 16];

        self.encrypt_message(
            CryptoAlgorithm::AesCbc,
            CipherFlags {
                finish: true,
                ..Default::default()
            },
            key,
            &iv,
            None,
            input,
            output,
        )
    }

    pub fn decrypt_message(
        &self,
        algorithm: CryptoAlgorithm,
        flags: CipherFlags,
        key: &[u8],
        iv: &[u8],
        tag: Option<&[u8]>,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, CryptoError> {
        // CBC with padding: decrypted output could be up to input size
        // (padding is removed after decryption, but we don’t know how much in advance)
        let expected_max_output_len = if flags.pad_to_block_size {
            // Output could be input.len(), as padding will be stripped at the end
            input.len()
        } else {
            // Without padding, input must be block-aligned
            assert!(
                input.len().is_multiple_of(AES_BLOCK_SIZE),
                "Input length must be multiple of 16 when no padding is used"
            );
            input.len()
        };

        // In AES-GCM, the tag is often part of the input or passed separately
        // But the actual plaintext will be shorter than ciphertext if tag is included in input
        if algorithm == CryptoAlgorithm::AesGcm {
            // If tag is passed separately, the ciphertext should already exclude it
            // If tag is included in `input`, you'd subtract it here
            // We'll assume tag is separate, so do nothing
        }

        assert!(
            output.len() >= expected_max_output_len,
            "Output buffer is too small: need at least {}, got {}",
            expected_max_output_len,
            output.len()
        );

        let mut output_len = 0;

        unsafe {
            if !PltDecryptMessage(
                self.handle.context,
                algorithm.raw(),
                flags.raw(),
                key.as_ptr() as *mut _,
                key.len() as i32,
                iv.as_ptr() as *mut _,
                iv.len() as i32,
                tag.map(|tag| tag.as_ptr() as *mut _).unwrap_or(null_mut()),
                tag.map(|tag| tag.len()).unwrap_or(0) as i32,
                input.as_ptr() as *mut _,
                input.len() as i32,
                output.as_mut_ptr(),
                &mut output_len as *mut _,
            ) {
                return Err(CryptoError);
            }
        }

        Ok(output_len as usize)
    }
    pub fn decrypt_aes(
        &self,
        key: &[u8],
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, CryptoError> {
        let iv = [0u8; 16];

        self.decrypt_message(
            CryptoAlgorithm::AesCbc,
            CipherFlags {
                finish: true,
                ..Default::default()
            },
            key,
            &iv,
            None,
            input,
            output,
        )
    }
}
