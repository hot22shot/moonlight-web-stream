use std::fmt::{Debug, Display};

/// A pin which contains four values in the range 0..10
#[derive(Clone, Copy)]
pub struct PairPin {
    numbers: [u8; 4],
}

impl PairPin {
    pub fn from_array(numbers: [u8; 4]) -> Option<Self> {
        let range = 0..10;

        if range.contains(&numbers[0])
            && range.contains(&numbers[1])
            && range.contains(&numbers[2])
            && range.contains(&numbers[3])
        {
            return Some(Self { numbers });
        }

        None
    }

    pub fn n(&self, index: usize) -> Option<u8> {
        self.numbers.get(index).copied()
    }
    pub fn n1(&self) -> u8 {
        self.numbers[0]
    }
    pub fn n2(&self) -> u8 {
        self.numbers[1]
    }
    pub fn n3(&self) -> u8 {
        self.numbers[2]
    }
    pub fn n4(&self) -> u8 {
        self.numbers[3]
    }

    pub fn array(&self) -> [u8; 4] {
        self.numbers
    }
}

impl Display for PairPin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}{}{}", self.n1(), self.n2(), self.n3(), self.n4())
    }
}
impl Debug for PairPin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PairPin(")?;
        Display::fmt(&self, f)?;
        write!(f, ")")?;

        Ok(())
    }
}

pub const SALT_LENGTH: usize = 16;
pub const CHALLENGE_LENGTH: usize = 16;

#[cfg(feature = "crypto")]
pub async fn host_pair(
    crypto: &crate::crypto::MoonlightCrypto,
    http_address: &str,
    client_info: crate::network::ClientInfo<'_>,
    server_version: crate::network::ServerVersion,
    device_name: &str,
    pin: PairPin,
) -> Result<crate::network::PairStatus, crate::network::ApiError> {
    use crate::{
        crypto::AES_BLOCK_SIZE,
        network::{
            ClientPairChallengeRequest, ClientPairRequest, PairStatus, host_pair_challenge,
            host_pair_initiate,
        },
    };

    let client_cert_pem = [0u8; 16];

    let hash_algorithm = crypto.hash_algorithm_for_server(server_version);
    // TODO: read already paired information
    let salt = crypto.generate_salt();
    let aes_key = crypto.generate_aes_key(hash_algorithm, salt, pin);

    let pair_response = host_pair_initiate(
        http_address,
        client_info,
        ClientPairRequest {
            device_name,
            salt,
            client_cert_pem,
        },
    )
    .await
    .unwrap();
    println!("{pair_response:#?}");

    assert_eq!(
        pair_response.paired,
        PairStatus::Paired,
        "Please try again and pair the client using the given values"
    );
    let Some(cert) = pair_response.cert else {
        panic!("Paired whilst another device was pairing!");
    };

    println!("-- Sending Challenge");
    let mut challenge = [0u8; CHALLENGE_LENGTH];
    crypto.generate_random(&mut challenge);

    let mut encrypted_challenge = [0u8; CHALLENGE_LENGTH];
    crypto
        .encrypt_aes(&aes_key, &challenge, &mut encrypted_challenge)
        .unwrap();

    let challenge_response = host_pair_challenge(
        http_address,
        client_info,
        ClientPairChallengeRequest {
            encrypted_challenge,
        },
    )
    .await
    .unwrap();
    println!("{challenge_response:#?}");

    let mut response = vec![0u8; challenge_response.encrypted_response.len() + AES_BLOCK_SIZE];
    crypto
        .decrypt_aes(
            &aes_key,
            &challenge_response.encrypted_response,
            &mut response,
        )
        .unwrap();

    let server_response = &response[0..hash_algorithm.hash_len()];
    let server_challenge =
        &response[hash_algorithm.hash_len()..hash_algorithm.hash_len() + CHALLENGE_LENGTH];

    println!("-- Challenge Response");
    let mut client_secret = [0u8; 16];
    crypto.generate_random(&mut client_secret);

    // TODO: this is made up of more than just this
    let mut server_challenge_response_hash = [0u8; 16];
    crypto.hash_size_uneq(
        hash_algorithm,
        &client_secret,
        &mut server_challenge_response_hash,
    );

    let mut server_challenge_response_encrypted = [0u8; 16];
    crypto
        .encrypt_aes(
            &aes_key,
            &server_challenge_response_hash,
            &mut server_challenge_response_encrypted,
        )
        .unwrap();

    todo!()
}
