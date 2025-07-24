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
