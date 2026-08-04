use std::fmt;

pub const DIRECTION_MASK: u64 = 1_u64 << 63;
pub const SEQUENCE_MASK: u64 = !DIRECTION_MASK;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    ToServer,
    ToClient,
}

impl Direction {
    pub fn as_wire_bit(self) -> u64 {
        match self {
            Self::ToServer => 0,
            Self::ToClient => DIRECTION_MASK,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SequenceNumber(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceError {
    OutOfRange(u64),
    Exhausted,
}

impl fmt::Display for SequenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRange(value) => write!(formatter, "sequence number out of range: {value}"),
            Self::Exhausted => formatter.write_str("sequence number exhausted"),
        }
    }
}

impl std::error::Error for SequenceError {}

impl SequenceNumber {
    pub const ZERO: Self = Self(0);
    pub const MAX: Self = Self(SEQUENCE_MASK);

    pub fn new(value: u64) -> Result<Self, SequenceError> {
        if value & DIRECTION_MASK != 0 {
            return Err(SequenceError::OutOfRange(value));
        }
        Ok(Self(value))
    }

    pub fn from_wire(value: u64) -> (Direction, Self) {
        let direction = if value & DIRECTION_MASK == 0 {
            Direction::ToServer
        } else {
            Direction::ToClient
        };
        (direction, Self(value & SEQUENCE_MASK))
    }

    pub fn value(self) -> u64 {
        self.0
    }

    pub fn to_wire(self, direction: Direction) -> u64 {
        self.0 | direction.as_wire_bit()
    }

    pub fn next(self) -> Result<Self, SequenceError> {
        if self.0 == SEQUENCE_MASK {
            return Err(SequenceError::Exhausted);
        }
        Ok(Self(self.0 + 1))
    }
}

#[cfg(test)]
mod tests {
    use super::{Direction, SequenceNumber, DIRECTION_MASK};

    #[test]
    fn encodes_direction_in_high_bit() -> Result<(), Box<dyn std::error::Error>> {
        let sequence = SequenceNumber::new(42)?;
        let wire = sequence.to_wire(Direction::ToClient);
        assert_eq!(wire, DIRECTION_MASK | 42);
        assert_eq!(
            SequenceNumber::from_wire(wire),
            (Direction::ToClient, sequence)
        );
        Ok(())
    }

    #[test]
    fn rejects_high_bit_as_plain_sequence() {
        assert!(SequenceNumber::new(DIRECTION_MASK).is_err());
    }
}
