use std::{ptr::null_mut, sync::Arc};

use bitflags::bitflags;
use moonlight_common_sys::crypto::{
    ALGORITHM_AES_CBC, ALGORITHM_AES_GCM, CIPHER_FLAG_FINISH, CIPHER_FLAG_PAD_TO_BLOCK_SIZE,
    CIPHER_FLAG_RESET_IV, PPLT_CRYPTO_CONTEXT, PltCreateCryptoContext, PltDecryptMessage,
    PltDestroyCryptoContext, PltEncryptMessage, PltGenerateRandomData,
};
use thiserror::Error;

use crate::{
    Handle, MoonlightInstance,
    network::ServerVersion,
    pair::{PairPin, SALT_LENGTH},
};

#[derive(Clone)]
pub(crate) struct CryptoContext {
    context: PPLT_CRYPTO_CONTEXT,
}

impl CryptoContext {
    fn new() -> Self {
        let context = unsafe { PltCreateCryptoContext() };

        Self { context }
    }
}

impl Drop for CryptoContext {
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

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptoAlgorithm {
    AesCbc = ALGORITHM_AES_CBC,
    AesGcm = ALGORITHM_AES_GCM,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    Sha1,
    Sha256,
}

impl HashAlgorithm {
    pub const MAX_HASH_LEN: usize = 32;

    pub fn hash_len(&self) -> usize {
        match self {
            Self::Sha1 => 20,
            Self::Sha256 => 32,
        }
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, Default)]
    pub struct CipherFlags: u32 {
        const RESET = CIPHER_FLAG_RESET_IV;
        const FINISH = CIPHER_FLAG_FINISH;
        const PAD_TO_BLOCK_SIZE = CIPHER_FLAG_PAD_TO_BLOCK_SIZE;
    }
}

pub struct MoonlightCrypto {
    #[allow(unused)]
    handle: Arc<Handle>,
}

impl MoonlightCrypto {
    pub(crate) fn new(instance: &MoonlightInstance) -> Self {
        Self {
            handle: instance.handle.clone(),
        }
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

    pub fn hash_algorithm_for_server(&self, server_version: ServerVersion) -> HashAlgorithm {
        if server_version.major >= 7 {
            HashAlgorithm::Sha256
        } else {
            HashAlgorithm::Sha1
        }
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

        if flags.contains(CipherFlags::PAD_TO_BLOCK_SIZE) {
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

        let context = CryptoContext::new();

        unsafe {
            if !PltEncryptMessage(
                context.context,
                algorithm as u32 as i32,
                flags.bits() as i32,
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

        if output_len > output.len() as i32 {
            panic!("output buffer was overwritten");
        }

        Ok(output_len as usize)
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
        let expected_max_output_len = if flags.contains(CipherFlags::PAD_TO_BLOCK_SIZE) {
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

        let context = CryptoContext::new();

        unsafe {
            if !PltDecryptMessage(
                context.context,
                algorithm as u32 as i32,
                flags.bits() as i32,
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

        if output_len > output.len() as i32 {
            panic!("output buffer was overwritten");
        }

        Ok(output_len as usize)
    }
}
