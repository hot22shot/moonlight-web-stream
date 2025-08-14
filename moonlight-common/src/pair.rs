use std::str::FromStr;

use openssl::{
    cipher::Cipher,
    cipher_ctx::CipherCtx,
    error::ErrorStack,
    md::Md,
    md_ctx::MdCtx,
    pkey::{PKey, Private},
    sha::{sha1, sha256},
    x509::X509,
};
use pem::{Pem, PemError};
use rcgen::{CertificateParams, KeyPair, PKCS_RSA_SHA256};
use thiserror::Error;

use crate::{
    CHALLENGE_LENGTH, PairPin, PairStatus, SALT_LENGTH, ServerVersion,
    moonlight::crypto::{HashAlgorithm, MoonlightCrypto},
    network::{
        ApiError, ClientInfo,
        pair::{
            ClientPairRequest1, ClientPairRequest2, ClientPairRequest3, ClientPairRequest4,
            ClientPairRequest5, host_pair1, host_pair2, host_pair3, host_pair4, host_pair5,
            host_unpair,
        },
        request_client::RequestClient,
    },
};

// TODO: maybe migrate this pairing process to openssl?

fn hash(algorithm: HashAlgorithm, data: &[u8], output: &mut [u8]) {
    match algorithm {
        HashAlgorithm::Sha1 => {
            let digest = sha1(data);
            output.copy_from_slice(&digest);
        }
        HashAlgorithm::Sha256 => {
            let digest = sha256(data);
            output.copy_from_slice(&digest);
        }
    }
}
fn hash_size_uneq(algorithm: HashAlgorithm, data: &[u8], output: &mut [u8]) {
    let mut hash = [0u8; HashAlgorithm::MAX_HASH_LEN];
    self::hash(algorithm, data, &mut hash);

    output.copy_from_slice(&hash[0..output.len()]);
}

fn salt_pin(salt: [u8; SALT_LENGTH], pin: PairPin) -> [u8; SALT_LENGTH + 4] {
    let mut out = [0u8; SALT_LENGTH + 4];

    out[0..16].copy_from_slice(&salt);

    let pin_utf8 = pin
        .array()
        .map(|value| char::from_digit(value as u32, 10).expect("a pin digit between 0-9") as u8);

    out[16..].copy_from_slice(&pin_utf8);

    out
}

fn generate_aes_key(algorithm: HashAlgorithm, salt: [u8; SALT_LENGTH], pin: PairPin) -> [u8; 16] {
    let mut hash = [0u8; 16];

    let salted = self::salt_pin(salt, pin);
    hash_size_uneq(algorithm, &salted, &mut hash);

    hash
}

pub fn encrypt_aes(key: &[u8], plaintext: &[u8]) -> Vec<u8> {
    let mut cipher_ctx = CipherCtx::new().unwrap();

    cipher_ctx
        .encrypt_init(Some(Cipher::aes_128_ecb()), Some(key), None)
        .unwrap();
    cipher_ctx.set_padding(false);

    let mut output = Vec::new();
    cipher_ctx
        .cipher_update_vec(plaintext, &mut output)
        .unwrap();
    output
}

pub fn decrypt_aes<C: RequestClient>(
    key: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, PairError<C::Error>> {
    let mut cipher_ctx = CipherCtx::new().unwrap();

    cipher_ctx
        .decrypt_init(Some(Cipher::aes_128_ecb()), Some(key), None)
        .unwrap();
    cipher_ctx.set_padding(false);

    let mut decrypted = Vec::new();
    cipher_ctx
        .cipher_update_vec(ciphertext, &mut decrypted)
        .unwrap();

    Ok(decrypted)
}

fn verify_signature(server_secret: &[u8], server_signature: &[u8], server_cert: &X509) -> bool {
    let public_key = server_cert.public_key().unwrap();

    let mut md_ctx = MdCtx::new().unwrap();

    md_ctx
        .digest_verify_init(Some(Md::sha256()), &public_key)
        .unwrap();
    md_ctx.digest_verify_update(server_secret).unwrap();
    md_ctx.digest_verify_final(server_signature).unwrap()
}

fn sign_data(private_key: &PKey<Private>, data: &[u8]) -> Vec<u8> {
    let mut md_ctx = MdCtx::new().unwrap();

    md_ctx
        .digest_sign_init(Some(Md::sha256()), &private_key)
        .unwrap();
    md_ctx.digest_sign_update(data).unwrap();

    let mut out = Vec::new();
    md_ctx.digest_sign_final_to_vec(&mut out).unwrap();
    out
}

// TOOD: maybe remove this struct?
#[derive(Clone)]
pub struct ClientAuth {
    pub key_pair: Pem,
    pub certificate: Pem,
}

pub fn generate_new_client() -> Result<ClientAuth, rcgen::Error> {
    let generated_signing_key = KeyPair::generate_for(&PKCS_RSA_SHA256)?;
    let generated_cert = CertificateParams::new(Vec::new())?.self_signed(&generated_signing_key)?;

    Ok(ClientAuth {
        key_pair: pem::parse(generated_signing_key.serialize_pem()).expect("valid private key"),
        certificate: pem::parse(generated_cert.pem()).expect("valid certificate"),
    })
}

pub struct PairSuccess {
    pub server_certificate: Pem,
}

#[derive(Debug, Error)]
pub enum PairError<RequestError> {
    #[error("{0}")]
    Api(#[from] ApiError<RequestError>),
    // Client
    #[error("incorrect client certificate: {0}")]
    ClientPrivateKeyPem(rcgen::Error),
    #[error("incorrect private key: make sure it's a PKCS_RSA_SHA256 key")]
    IncorrectPrivateKey,
    // Server
    #[error("")]
    OpenSSL(#[from] ErrorStack),
    #[error("incorrect server certificate pem: {0}")]
    ServerCertificatePem(PemError),
    // Pairing failures
    #[error("the pin was wrong")]
    IncorrectPin,
    #[error("there's another pairing procedure currently")]
    AlreadyInProgress,
    #[error("pairing failed")]
    Failed,
}

pub async fn host_pair<C: RequestClient>(
    crypto: &MoonlightCrypto,
    client: &mut C,
    http_address: &str,
    client_info: ClientInfo<'_>,
    client_private_key_pem: &Pem,
    client_certificate_pem: &Pem,
    device_name: &str,
    server_version: ServerVersion,
    pin: PairPin,
) -> Result<PairSuccess, PairError<C::Error>> {
    let client_cert = X509::from_der(client_certificate_pem.contents())?;
    let client_private_key = PKey::private_key_from_der(client_private_key_pem.contents())?;

    // TODO: check
    // if client_private_key. != &PKCS_RSA_SHA256 {
    //     return Err(PairError::IncorrectPrivateKey);
    // }

    let client_cert_pem = client_certificate_pem.to_string();

    let hash_algorithm = crypto.hash_algorithm_for_server(server_version);

    let salt = crypto.generate_salt();
    let aes_key = generate_aes_key(hash_algorithm, salt, pin);

    let server_response1 = host_pair1(
        client,
        http_address,
        client_info,
        ClientPairRequest1 {
            device_name,
            salt,
            client_cert_pem: client_cert_pem.as_bytes(),
        },
    )
    .await?;

    if !matches!(server_response1.paired, PairStatus::Paired) {
        return Err(PairError::Failed);
    }
    let Some(server_cert_str) = server_response1.cert else {
        return Err(PairError::AlreadyInProgress);
    };

    let server_cert_pem =
        Pem::from_str(&server_cert_str).map_err(PairError::ServerCertificatePem)?;
    let server_cert = X509::from_der(server_cert_pem.contents())?;

    let mut challenge = [0u8; CHALLENGE_LENGTH];
    crypto.generate_random(&mut challenge);

    let encrypted_challenge = encrypt_aes(&aes_key, &challenge);

    let server_response2 = host_pair2(
        client,
        http_address,
        client_info,
        ClientPairRequest2 {
            device_name,
            encrypted_challenge: &encrypted_challenge,
        },
    )
    .await?;

    if !matches!(server_response2.paired, PairStatus::Paired) {
        host_unpair(client, http_address, client_info).await?;

        return Err(PairError::Failed);
    }

    let response = decrypt_aes::<C>(&aes_key, &server_response2.encrypted_response)?;

    let server_response_hash = &response[0..hash_algorithm.hash_len()];
    let server_challenge =
        &response[hash_algorithm.hash_len()..hash_algorithm.hash_len() + CHALLENGE_LENGTH];

    let mut client_secret = [0u8; 16];
    crypto.generate_random(&mut client_secret);

    let mut challenge_response = Vec::new();
    challenge_response.extend_from_slice(server_challenge);
    challenge_response.extend_from_slice(client_cert.signature().as_slice());
    challenge_response.extend_from_slice(&client_secret);

    let mut challenge_response_hash = [0u8; HashAlgorithm::MAX_HASH_LEN];
    hash_size_uneq(
        hash_algorithm,
        &challenge_response,
        &mut challenge_response_hash,
    );

    let encrypted_challenge_response_hash = encrypt_aes(
        &aes_key,
        &challenge_response_hash[0..hash_algorithm.hash_len()],
    );

    let server_response3 = host_pair3(
        client,
        http_address,
        client_info,
        ClientPairRequest3 {
            device_name,
            encrypted_challenge_response_hash: &encrypted_challenge_response_hash,
        },
    )
    .await?;

    if !matches!(server_response3.paired, PairStatus::Paired) {
        host_unpair(client, http_address, client_info).await?;

        return Err(PairError::Failed);
    }

    let mut server_secret = [0u8; 16];
    server_secret.copy_from_slice(&server_response3.server_pairing_secret[0..16]);

    let mut server_signature = Vec::new();
    server_signature.extend_from_slice(&server_response3.server_pairing_secret[16..]);

    if !verify_signature(&server_secret, &server_signature, &server_cert) {
        host_unpair(client, http_address, client_info).await?;

        // MITM likely
        return Err(PairError::Failed);
    }

    let mut expected_response = Vec::new();
    expected_response.extend_from_slice(&challenge);
    expected_response.extend_from_slice(server_cert.signature().as_slice());
    expected_response.extend_from_slice(&server_secret);

    let mut expected_response_hash = [0u8; HashAlgorithm::MAX_HASH_LEN];
    hash_size_uneq(
        hash_algorithm,
        &expected_response,
        &mut expected_response_hash,
    );

    let expected_response_hash = &expected_response_hash[0..hash_algorithm.hash_len()];
    if expected_response_hash != server_response_hash {
        host_unpair(client, http_address, client_info).await?;

        // Probably wrong pin
        return Err(PairError::IncorrectPin);
    }

    // Send the server our signed secret
    let mut client_pairing_secret = Vec::new();
    client_pairing_secret.extend_from_slice(&client_secret);
    client_pairing_secret.extend_from_slice(&sign_data(&client_private_key, &client_secret));

    let server_response4 = host_pair4(
        client,
        http_address,
        client_info,
        ClientPairRequest4 {
            device_name,
            client_pairing_secret: &client_pairing_secret,
        },
    )
    .await?;

    if !matches!(server_response4.paired, PairStatus::Paired) {
        host_unpair(client, http_address, client_info).await?;

        return Err(PairError::Failed);
    }

    // Required for us to show as paired
    let server_response5 = host_pair5(
        client,
        http_address,
        client_info,
        ClientPairRequest5 { device_name },
    )
    .await?;

    if !matches!(server_response5.paired, PairStatus::Paired) {
        host_unpair(client, http_address, client_info).await?;

        return Err(PairError::Failed);
    }

    Ok(PairSuccess {
        server_certificate: server_cert_pem,
    })
}
