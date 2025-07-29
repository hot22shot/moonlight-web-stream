use std::{rc, str::FromStr};

use aes::Aes128;
use block_modes::{BlockMode, Ecb, block_padding::NoPadding};
use pem::Pem;
use rcgen::{Certificate, CertificateParams, KeyPair, PKCS_RSA_SHA256, SigningKey};
use rsa::{Pkcs1v15Sign, RsaPublicKey, pkcs8::DecodePublicKey};
use sha2::Sha256;
use x509_parser::{
    parse_x509_certificate,
    prelude::{FromDer, X509Certificate},
};

use crate::{
    crypto::{HashAlgorithm, MoonlightCrypto},
    network::{
        ApiError, ClientInfo, ClientPairRequest, ClientPairRequest1, ClientPairRequest2,
        ClientPairRequest3, ClientPairRequestFinal, PairStatus, ServerVersion, host_pair_final,
        host_pair_initiate, host_pair1, host_pair2, host_pair3,
    },
    pair::{CHALLENGE_LENGTH, PairPin, SALT_LENGTH},
};

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

fn decrypt_aes(key: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = Aes128Ecb::new_from_slices(key, &[]).unwrap();

    // Decrypt in place, so clone ciphertext to mutable vec
    let mut buffer = ciphertext.to_vec();

    // Decrypt and remove padding (NoPadding means no removal)
    cipher
        .decrypt(&mut buffer)
        .map_err(|e| format!("Decryption failed: {e:?}"))?;

    Ok(buffer)
}

fn encrypt_aes(key: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = Aes128Ecb::new_from_slices(key, &[])
        .map_err(|e| format!("Error initializing ECB cipher: {e:?}"))?;

    let mut buf = plaintext.to_vec();
    cipher
        .encrypt(&mut buf, 0)
        .map_err(|e| format!("Encryption failed: {e:?}"))?;

    Ok(buf)
}

fn verify_signature(
    server_secret: &[u8],
    server_signature: &[u8],
    server_cert: &X509Certificate,
) -> bool {
    const HASH_ALGO: HashAlgorithm = HashAlgorithm::Sha256;

    let public_key = RsaPublicKey::from_public_key_der(server_cert.public_key().raw).unwrap();

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
    key_pair.sign(data).unwrap()
}

pub fn generate_key_and_cert() -> Result<(KeyPair, Certificate), rcgen::Error> {
    let generated_signing_key = KeyPair::generate_for(&PKCS_RSA_SHA256)?;
    let generated_cert = CertificateParams::new(Vec::new())?.self_signed(&generated_signing_key)?;

    Ok((generated_signing_key, generated_cert))
}

pub enum PairResult {
    NotPaired,
    Paired { server_certificate: Pem },
}

// TODO: call unpair on error?
pub async fn host_pair(
    crypto: &MoonlightCrypto,
    http_address: &str,
    client_info: ClientInfo<'_>,
    client_private_key_pem: &Pem,
    client_certificate_pem: &Pem,
    device_name: &str,
    server_version: ServerVersion,
    pin: PairPin,
) -> Result<PairResult, ApiError> {
    let (_, client_cert) = X509Certificate::from_der(client_certificate_pem.contents()).unwrap();
    let client_key_pair = KeyPair::from_pem(&client_private_key_pem.to_string()).unwrap();
    // assert!(client_key_pair.algorithm() == &PKCS_RSA_SHA256);

    let client_cert_pem = client_certificate_pem.to_string();

    let hash_algorithm = crypto.hash_algorithm_for_server(server_version);
    // TODO: read already paired information
    let salt = crypto.generate_salt();
    let aes_key = generate_aes_key(hash_algorithm, salt, pin);

    let pair_response = host_pair_initiate(
        http_address,
        client_info,
        ClientPairRequest {
            device_name,
            salt,
            client_cert_pem: client_cert_pem.as_bytes(),
        },
    )
    .await
    .unwrap();
    println!("{pair_response:#?}");

    if !matches!(pair_response.paired, PairStatus::Paired) {
        panic!("Please try again and pair the client using the given values");
    }
    let Some(server_cert_str) = pair_response.cert else {
        panic!("Paired whilst another device was pairing!");
    };

    let server_cert_pem = Pem::from_str(&server_cert_str).unwrap();
    let (_, server_cert) = parse_x509_certificate(server_cert_pem.contents()).unwrap();

    // TODO: set cert?

    println!("-- Sending Challenge");
    let mut challenge = [0u8; CHALLENGE_LENGTH];
    crypto.generate_random(&mut challenge);

    let encrypted_challenge = encrypt_aes(&aes_key, &challenge).unwrap();

    let challenge_response = host_pair1(
        http_address,
        client_info,
        ClientPairRequest1 {
            device_name,
            encrypted_challenge: &encrypted_challenge,
        },
    )
    .await
    .unwrap();
    println!("{challenge_response:#?}");

    if !matches!(challenge_response.paired, PairStatus::Paired) {
        // TODO: unpair
        todo!()
    }

    let response = decrypt_aes(&aes_key, &challenge_response.encrypted_response).unwrap();

    let server_response_hash = &response[0..hash_algorithm.hash_len()];
    let server_challenge =
        &response[hash_algorithm.hash_len()..hash_algorithm.hash_len() + CHALLENGE_LENGTH];

    println!("-- Challenge Response");
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
    )
    .expect("encrypt challenge_response_hash with aes");

    let server_response2 = host_pair2(
        http_address,
        client_info,
        ClientPairRequest2 {
            device_name,
            encrypted_challenge_response_hash: &encrypted_challenge_response_hash,
        },
    )
    .await
    .unwrap();
    println!("{server_response2:#?}");

    if !matches!(server_response2.paired, PairStatus::Paired) {
        // TODO: unpair
        todo!()
    }

    let mut server_secret = [0u8; 16];
    server_secret.copy_from_slice(&server_response2.server_pairing_secret[0..16]);

    let mut server_signature = Vec::new();
    server_signature.extend_from_slice(&server_response2.server_pairing_secret[16..]);

    if !verify_signature(&server_secret, &server_signature, &server_cert) {
        // TODO: unpair

        // MITM likely
        todo!()
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

    let cmp1 = &expected_response_hash[0..hash_algorithm.hash_len()];
    if cmp1 != server_response_hash {
        // TODO: unpair and error
        println!("Error on:\n{cmp1:?}\n{server_response_hash:?}");

        // Probably wrong pin
        todo!()
    }

    // Send the server our signed secret
    let mut client_pairing_secret = Vec::new();
    client_pairing_secret.extend_from_slice(&client_secret);
    client_pairing_secret.extend_from_slice(&sign_data(&client_key_pair, &client_secret));

    let server_response3 = host_pair3(
        http_address,
        client_info,
        ClientPairRequest3 {
            device_name,
            client_pairing_secret: &client_pairing_secret,
        },
    )
    .await
    .unwrap();

    println!("{server_response3:#?}");
    if !matches!(server_response3.paired, PairStatus::Paired) {
        // TODO: unpair
        todo!()
    }

    // Required for us to show as paired
    let final_response = host_pair_final(
        http_address,
        client_info,
        ClientPairRequestFinal { device_name },
    )
    .await
    .unwrap();
    println!("{final_response:#?}");

    if !matches!(final_response.paired, PairStatus::Paired) {
        // TODO: unpair
        todo!()
    }

    Ok(PairResult::Paired {
        server_certificate: server_cert_pem,
    })
}
