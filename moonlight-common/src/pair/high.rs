use std::str::FromStr;

use aes::Aes128;
use block_modes::{BlockMode, BlockModeError, Ecb, block_padding::NoPadding};
use pem::{Pem, PemError};
use rcgen::{CertificateParams, KeyPair, PKCS_RSA_SHA256};
use reqwest::Client;
use rsa::{
    Pkcs1v15Sign, RsaPrivateKey, RsaPublicKey,
    pkcs8::{DecodePrivateKey, DecodePublicKey},
};
use sha2::Sha256;
use thiserror::Error;
use x509_parser::{
    error::X509Error,
    parse_x509_certificate,
    prelude::{FromDer, X509Certificate},
};

use crate::{
    crypto::{HashAlgorithm, MoonlightCrypto},
    network::{
        ApiError, ClientInfo, ClientPairRequest1, ClientPairRequest2, ClientPairRequest3,
        ClientPairRequest4, ClientPairRequest5, PairStatus, ServerVersion, host_pair1, host_pair2,
        host_pair3, host_pair4, host_pair5, host_unpair,
    },
    pair::{CHALLENGE_LENGTH, PairPin, SALT_LENGTH},
};

// TODO: maybe migrate this pairing process to openssl?

fn hash(algorithm: HashAlgorithm, data: &[u8], output: &mut [u8]) {
    use sha1::Digest;

    match algorithm {
        HashAlgorithm::Sha1 => {
            let digest = sha1::Sha1::digest(data);
            output.copy_from_slice(&digest);
        }
        HashAlgorithm::Sha256 => {
            let digest = sha2::Sha256::digest(data);
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

type Aes128Ecb = Ecb<Aes128, NoPadding>;

pub fn encrypt_aes(key: &[u8], plaintext: &[u8]) -> Vec<u8> {
    let cipher = Aes128Ecb::new_from_slices(key, &[]).expect("valid iv key (the key is &[])");

    // Clone plaintext into a mutable buffer
    let mut buf = plaintext.to_vec();
    // Encrypt in place, specifying the full plaintext length
    cipher
        .encrypt(&mut buf, plaintext.len())
        .expect("no required padding for encryption");

    buf
}

pub fn decrypt_aes(key: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, PairError> {
    let cipher = Aes128Ecb::new_from_slices(key, &[]).expect("a valid iv key (the key is &[])");

    let mut buf = ciphertext.to_vec();
    cipher.decrypt(&mut buf)?;

    Ok(buf)
}

fn verify_signature(
    server_secret: &[u8],
    server_signature: &[u8],
    server_cert: &X509Certificate,
) -> bool {
    const HASH_ALGO: HashAlgorithm = HashAlgorithm::Sha256;

    let public_key = RsaPublicKey::from_public_key_der(server_cert.public_key().raw)
        .expect("a valid server certificate public key");

    let mut hashed = [0u8; HashAlgorithm::MAX_HASH_LEN];
    hash(HASH_ALGO, server_secret, &mut hashed);

    public_key
        .verify(
            Pkcs1v15Sign::new::<Sha256>(),
            &hashed[0..HASH_ALGO.hash_len()],
            server_signature,
        )
        .is_ok()
}

fn sign_data(key_pair: &KeyPair, data: &[u8]) -> Vec<u8> {
    const HASH_ALGO: HashAlgorithm = HashAlgorithm::Sha256;

    let private_key = RsaPrivateKey::from_pkcs8_der(key_pair.serialized_der())
        .expect("a valid pkcs8 private key");

    let mut hashed = [0u8; HashAlgorithm::MAX_HASH_LEN];
    hash(HASH_ALGO, data, &mut hashed);

    private_key
        .sign(Pkcs1v15Sign::new::<Sha256>(), &hashed)
        .expect("sign the data")
}

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
pub enum PairError {
    #[error("{0}")]
    Api(#[from] ApiError),
    // Client
    #[error("incorrect client certificate: {0}")]
    ClientPrivateKeyPem(rcgen::Error),
    #[error("incorrect client certificate: {0}")]
    ClientCertificateError(nom::Err<X509Error>),
    #[error("incorrect private key: make sure it's a PKCS_RSA_SHA256 key")]
    IncorrectPrivateKey,
    // Server
    #[error("{0}")]
    Decrypt(#[from] BlockModeError),
    #[error("incorrect server certificate pem: {0}")]
    ServerCertificatePem(PemError),
    #[error("incorrect server certificate: {0}")]
    ServerCertificateParse(nom::Err<X509Error>),
    // Pairing failures
    #[error("the pin was wrong")]
    IncorrectPin,
    #[error("there's another pairing procedure currently")]
    AlreadyInProgress,
    #[error("pairing failed")]
    Failed,
}

pub async fn host_pair(
    crypto: &MoonlightCrypto,
    client: &Client,
    http_address: &str,
    client_info: ClientInfo<'_>,
    client_private_key_pem: &Pem,
    client_certificate_pem: &Pem,
    device_name: &str,
    server_version: ServerVersion,
    pin: PairPin,
) -> Result<PairSuccess, PairError> {
    let (_, client_cert) = X509Certificate::from_der(client_certificate_pem.contents())
        .map_err(PairError::ClientCertificateError)?;
    let client_key_pair = KeyPair::from_pem(&client_private_key_pem.to_string())
        .map_err(PairError::ClientPrivateKeyPem)?;

    if client_key_pair.algorithm() != &PKCS_RSA_SHA256 {
        return Err(PairError::IncorrectPrivateKey);
    }

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
    let (_, server_cert) = parse_x509_certificate(server_cert_pem.contents())
        .map_err(PairError::ServerCertificateParse)?;

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

    let response = decrypt_aes(&aes_key, &server_response2.encrypted_response)?;

    let server_response_hash = &response[0..hash_algorithm.hash_len()];
    let server_challenge =
        &response[hash_algorithm.hash_len()..hash_algorithm.hash_len() + CHALLENGE_LENGTH];

    let mut client_secret = [0u8; 16];
    crypto.generate_random(&mut client_secret);

    let mut challenge_response = Vec::new();
    challenge_response.extend_from_slice(server_challenge);
    challenge_response.extend_from_slice(&client_cert.signature_value.data);
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
    expected_response.extend_from_slice(&server_cert.signature_value.data);
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
    client_pairing_secret.extend_from_slice(&sign_data(&client_key_pair, &client_secret));

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
