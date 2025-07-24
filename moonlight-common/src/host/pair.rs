use std::fmt::{Debug, Display};

use rand::random_range;
use sha1::Sha1;
use sha2::{Digest, Sha256};

use crate::host::network::ServerVersion;

/// A pin which contains four values in the range 0..10
#[derive(Clone, Copy)]
pub struct PairPin {
    n1: u8,
    n2: u8,
    n3: u8,
    n4: u8,
}

impl PairPin {
    pub fn random() -> Self {
        let n1 = random_range(0..10);
        let n2 = random_range(0..10);
        let n3 = random_range(0..10);
        let n4 = random_range(0..10);

        Self { n1, n2, n3, n4 }
    }

    pub fn n(&self, index: usize) -> Option<u8> {
        match index {
            0 => Some(self.n1),
            1 => Some(self.n2),
            2 => Some(self.n3),
            3 => Some(self.n4),
            _ => None,
        }
    }
    pub fn n1(&self) -> u8 {
        self.n1
    }
    pub fn n2(&self) -> u8 {
        self.n2
    }
    pub fn n3(&self) -> u8 {
        self.n3
    }
    pub fn n4(&self) -> u8 {
        self.n4
    }

    pub fn array(&self) -> [u8; 4] {
        [self.n1, self.n2, self.n3, self.n4]
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
