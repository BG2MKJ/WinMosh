use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, Read, Write};

use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;

use crate::proto::{ProtoError, TransportInstruction};

pub const FRAGMENT_HEADER_LENGTH: usize = 10;
pub const MAX_FRAGMENT_NUMBER: u16 = 0x7fff;
pub const MAX_COMPRESSED_INSTRUCTION_LENGTH: usize = 4 * 1024 * 1024;
pub const MAX_INSTRUCTION_LENGTH: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fragment {
    pub id: u64,
    pub fragment_number: u16,
    pub final_fragment: bool,
    pub contents: Vec<u8>,
}

#[derive(Debug)]
pub enum FragmentError {
    InvalidHeader,
    FragmentNumberTooLarge,
    ConflictingDuplicate,
    ConflictingFinalMarker,
    TooManyFragments,
    TooLarge,
    Compression(io::Error),
    Decompression(io::Error),
    Proto(ProtoError),
}

impl fmt::Display for FragmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHeader => formatter.write_str("invalid SSP fragment header"),
            Self::FragmentNumberTooLarge => formatter.write_str("SSP fragment number is too large"),
            Self::ConflictingDuplicate => formatter.write_str("conflicting duplicate SSP fragment"),
            Self::ConflictingFinalMarker => {
                formatter.write_str("conflicting SSP final fragment marker")
            }
            Self::TooManyFragments => formatter.write_str("too many SSP fragments"),
            Self::TooLarge => formatter.write_str("SSP instruction exceeds the size limit"),
            Self::Compression(error) => write!(formatter, "SSP compression failed: {error}"),
            Self::Decompression(error) => write!(formatter, "SSP decompression failed: {error}"),
            Self::Proto(error) => write!(formatter, "SSP protobuf decoding failed: {error}"),
        }
    }
}

impl std::error::Error for FragmentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Compression(error) | Self::Decompression(error) => Some(error),
            Self::Proto(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ProtoError> for FragmentError {
    fn from(error: ProtoError) -> Self {
        Self::Proto(error)
    }
}

impl Fragment {
    pub fn encode(&self) -> Result<Vec<u8>, FragmentError> {
        if self.fragment_number > MAX_FRAGMENT_NUMBER {
            return Err(FragmentError::FragmentNumberTooLarge);
        }
        let combined = self.fragment_number | if self.final_fragment { 0x8000 } else { 0 };
        let mut output = Vec::with_capacity(FRAGMENT_HEADER_LENGTH + self.contents.len());
        output.extend_from_slice(&self.id.to_be_bytes());
        output.extend_from_slice(&combined.to_be_bytes());
        output.extend_from_slice(&self.contents);
        Ok(output)
    }

    pub fn decode(input: &[u8]) -> Result<Self, FragmentError> {
        if input.len() < FRAGMENT_HEADER_LENGTH {
            return Err(FragmentError::InvalidHeader);
        }
        let mut id_bytes = [0_u8; 8];
        id_bytes.copy_from_slice(&input[..8]);
        let mut number_bytes = [0_u8; 2];
        number_bytes.copy_from_slice(&input[8..10]);
        let combined = u16::from_be_bytes(number_bytes);
        Ok(Self {
            id: u64::from_be_bytes(id_bytes),
            fragment_number: combined & MAX_FRAGMENT_NUMBER,
            final_fragment: combined & 0x8000 != 0,
            contents: input[FRAGMENT_HEADER_LENGTH..].to_vec(),
        })
    }
}

#[derive(Debug, Default)]
pub struct Fragmenter {
    next_instruction_id: u64,
    last_instruction: Option<TransportInstruction>,
    last_mtu: Option<usize>,
}

impl Fragmenter {
    pub fn make_fragments(
        &mut self,
        instruction: &TransportInstruction,
        mtu: usize,
    ) -> Result<Vec<Fragment>, FragmentError> {
        let payload_mtu = mtu
            .checked_sub(FRAGMENT_HEADER_LENGTH)
            .ok_or(FragmentError::InvalidHeader)?;
        if payload_mtu == 0 {
            return Err(FragmentError::InvalidHeader);
        }
        if self.last_instruction.as_ref() != Some(instruction) || self.last_mtu != Some(mtu) {
            self.next_instruction_id = self.next_instruction_id.wrapping_add(1);
            self.last_instruction = Some(instruction.clone());
            self.last_mtu = Some(mtu);
        }

        let encoded = instruction.encode();
        let compressed = compress(&encoded)?;
        if compressed.len() > MAX_COMPRESSED_INSTRUCTION_LENGTH {
            return Err(FragmentError::TooLarge);
        }

        let mut fragments = Vec::new();
        for (index, contents) in compressed.chunks(payload_mtu).enumerate() {
            let fragment_number =
                u16::try_from(index).map_err(|_| FragmentError::TooManyFragments)?;
            if fragment_number > MAX_FRAGMENT_NUMBER {
                return Err(FragmentError::TooManyFragments);
            }
            fragments.push(Fragment {
                id: self.next_instruction_id,
                fragment_number,
                final_fragment: (index + 1) * payload_mtu >= compressed.len(),
                contents: contents.to_vec(),
            });
        }
        Ok(fragments)
    }
}

#[derive(Debug, Default)]
pub struct FragmentAssembler {
    current_id: Option<u64>,
    fragments: BTreeMap<u16, Fragment>,
    final_fragment: Option<u16>,
    compressed_length: usize,
}

impl FragmentAssembler {
    pub fn add_wire(
        &mut self,
        input: &[u8],
    ) -> Result<Option<TransportInstruction>, FragmentError> {
        self.add_fragment(Fragment::decode(input)?)
    }

    pub fn add_fragment(
        &mut self,
        fragment: Fragment,
    ) -> Result<Option<TransportInstruction>, FragmentError> {
        if fragment.fragment_number > MAX_FRAGMENT_NUMBER {
            return Err(FragmentError::FragmentNumberTooLarge);
        }
        if self.current_id != Some(fragment.id) {
            self.current_id = Some(fragment.id);
            self.fragments.clear();
            self.final_fragment = None;
            self.compressed_length = 0;
        }

        if let Some(final_fragment) = self.final_fragment {
            if fragment.fragment_number > final_fragment {
                return Err(FragmentError::ConflictingFinalMarker);
            }
        }
        if fragment.final_fragment {
            if let Some(final_fragment) = self.final_fragment {
                if final_fragment != fragment.fragment_number {
                    return Err(FragmentError::ConflictingFinalMarker);
                }
            }
            self.final_fragment = Some(fragment.fragment_number);
        }

        if let Some(existing) = self.fragments.get(&fragment.fragment_number) {
            if existing != &fragment {
                return Err(FragmentError::ConflictingDuplicate);
            }
        } else {
            self.compressed_length = self
                .compressed_length
                .checked_add(fragment.contents.len())
                .ok_or(FragmentError::TooLarge)?;
            if self.compressed_length > MAX_COMPRESSED_INSTRUCTION_LENGTH {
                return Err(FragmentError::TooLarge);
            }
            self.fragments.insert(fragment.fragment_number, fragment);
        }

        let Some(final_fragment) = self.final_fragment else {
            return Ok(None);
        };
        let expected_count = usize::from(final_fragment) + 1;
        if self.fragments.len() != expected_count
            || (0..=final_fragment).any(|number| !self.fragments.contains_key(&number))
        {
            return Ok(None);
        }

        let mut compressed = Vec::with_capacity(self.compressed_length);
        for number in 0..=final_fragment {
            compressed.extend_from_slice(
                &self
                    .fragments
                    .get(&number)
                    .ok_or(FragmentError::InvalidHeader)?
                    .contents,
            );
        }
        let decoded = decompress(&compressed)?;
        self.reset();
        Ok(Some(TransportInstruction::decode(&decoded)?))
    }

    fn reset(&mut self) {
        self.current_id = None;
        self.fragments.clear();
        self.final_fragment = None;
        self.compressed_length = 0;
    }
}

fn compress(input: &[u8]) -> Result<Vec<u8>, FragmentError> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(input)
        .map_err(FragmentError::Compression)?;
    encoder.finish().map_err(FragmentError::Compression)
}

fn decompress(input: &[u8]) -> Result<Vec<u8>, FragmentError> {
    let mut decoder = ZlibDecoder::new(input);
    let mut output = Vec::new();
    decoder
        .by_ref()
        .take((MAX_INSTRUCTION_LENGTH + 1) as u64)
        .read_to_end(&mut output)
        .map_err(FragmentError::Decompression)?;
    if output.len() > MAX_INSTRUCTION_LENGTH {
        return Err(FragmentError::TooLarge);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::{Fragment, FragmentAssembler, Fragmenter, FRAGMENT_HEADER_LENGTH};
    use crate::proto::TransportInstruction;

    #[test]
    fn fragment_header_is_big_endian_and_round_trips() -> Result<(), Box<dyn std::error::Error>> {
        let fragment = Fragment {
            id: 0x0102_0304_0506_0708,
            fragment_number: 3,
            final_fragment: true,
            contents: b"payload".to_vec(),
        };
        let wire = fragment.encode()?;
        assert_eq!(wire.len(), FRAGMENT_HEADER_LENGTH + 7);
        assert_eq!(&wire[..10], &[1, 2, 3, 4, 5, 6, 7, 8, 0x80, 3]);
        assert_eq!(Fragment::decode(&wire)?, fragment);
        Ok(())
    }

    #[test]
    fn assembler_handles_reordering_and_duplicates() -> Result<(), Box<dyn std::error::Error>> {
        let instruction = TransportInstruction {
            protocol_version: Some(1),
            old_num: Some(8),
            new_num: Some(9),
            ack_num: Some(7),
            throwaway_num: None,
            diff: Some((0_u8..=255).collect()),
            chaff: None,
        };
        let mut fragmenter = Fragmenter::default();
        let fragments = fragmenter.make_fragments(&instruction, 32)?;
        assert!(fragments.len() > 1);

        let mut assembler = FragmentAssembler::default();
        let mut result = None;
        for fragment in fragments.iter().rev() {
            result = assembler.add_fragment(fragment.clone())?.or(result);
        }
        assert_eq!(result, Some(instruction.clone()));
        assert_eq!(assembler.add_fragment(fragments[0].clone())?, None);
        Ok(())
    }

    #[test]
    fn conflicting_duplicate_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let first = Fragment {
            id: 1,
            fragment_number: 0,
            final_fragment: false,
            contents: vec![1],
        };
        let mut assembler = FragmentAssembler::default();
        assert!(assembler.add_fragment(first.clone())?.is_none());
        let mut duplicate = first;
        duplicate.contents = vec![2];
        assert!(assembler.add_fragment(duplicate).is_err());
        Ok(())
    }

    #[test]
    fn large_instruction_is_fragmented_and_reassembled() -> Result<(), Box<dyn std::error::Error>> {
        let mut random_data = Vec::with_capacity(10000);
        let mut state: u32 = 1;
        for _ in 0..10000 {
            state = state.wrapping_mul(1103515245).wrapping_add(12345);
            random_data.push((state >> 16) as u8);
        }
        let instruction = TransportInstruction {
            protocol_version: Some(1),
            old_num: Some(1),
            new_num: Some(2),
            ack_num: Some(0),
            throwaway_num: None,
            diff: Some(random_data),
            chaff: None,
        };
        let mut fragmenter = Fragmenter::default();
        let mtu = 1400;
        let fragments = fragmenter.make_fragments(&instruction, mtu)?;
        assert!(
            fragments.len() > 3,
            "expected multiple fragments, got {}",
            fragments.len()
        );
        let mut assembler = FragmentAssembler::default();
        let mut result = None;
        for fragment in &fragments {
            result = assembler.add_fragment(fragment.clone())?.or(result);
        }
        assert_eq!(result, Some(instruction));
        Ok(())
    }

    #[test]
    fn single_fragment_instruction_passes_through() -> Result<(), Box<dyn std::error::Error>> {
        let instruction = TransportInstruction {
            protocol_version: Some(1),
            old_num: Some(1),
            new_num: Some(2),
            ack_num: Some(3),
            throwaway_num: None,
            diff: Some(b"small".to_vec()),
            chaff: None,
        };
        let mut fragmenter = Fragmenter::default();
        let fragments = fragmenter.make_fragments(&instruction, 1400)?;
        assert_eq!(fragments.len(), 1);
        assert!(fragments[0].final_fragment);
        let mut assembler = FragmentAssembler::default();
        let result = assembler.add_fragment(fragments[0].clone())?;
        assert_eq!(result, Some(instruction));
        Ok(())
    }

    #[test]
    fn assembler_resets_on_new_instruction_id() -> Result<(), Box<dyn std::error::Error>> {
        let f1 = Fragment {
            id: 1,
            fragment_number: 0,
            final_fragment: true,
            contents: vec![1],
        };
        let f2 = Fragment {
            id: 2,
            fragment_number: 0,
            final_fragment: true,
            contents: vec![2],
        };
        let mut assembler = FragmentAssembler::default();
        let r1 = assembler.add_fragment(f1)?;
        assert!(r1.is_some());
        let r2 = assembler.add_fragment(f2)?;
        assert!(r2.is_some());
        Ok(())
    }

    #[test]
    fn empty_diff_instruction_is_compressed_and_sent() -> Result<(), Box<dyn std::error::Error>> {
        let instruction = TransportInstruction {
            protocol_version: Some(1),
            old_num: Some(5),
            new_num: Some(5),
            ack_num: Some(5),
            throwaway_num: None,
            diff: Some(Vec::new()),
            chaff: None,
        };
        let mut fragmenter = Fragmenter::default();
        let fragments = fragmenter.make_fragments(&instruction, 1400)?;
        assert_eq!(fragments.len(), 1);
        let mut assembler = FragmentAssembler::default();
        let result = assembler.add_fragment(fragments[0].clone())?;
        assert_eq!(result, Some(instruction));
        Ok(())
    }

    #[test]
    fn assembler_waits_for_missing_fragments() -> Result<(), Box<dyn std::error::Error>> {
        let data: Vec<u8> = (0_u8..200).collect();
        let instruction = TransportInstruction {
            protocol_version: Some(1),
            old_num: Some(1),
            new_num: Some(2),
            ack_num: Some(0),
            throwaway_num: None,
            diff: Some(data),
            chaff: None,
        };
        let mut fragmenter = Fragmenter::default();
        let mtu = 60;
        let fragments = fragmenter.make_fragments(&instruction, mtu)?;
        assert!(
            fragments.len() >= 2,
            "need at least 2 fragments, got {}",
            fragments.len()
        );

        let mut assembler = FragmentAssembler::default();
        for &i in &[0_usize, 0_usize] {
            assert!(assembler.add_fragment(fragments[i].clone())?.is_none());
        }
        let mut result = None;
        for frag in &fragments[1..] {
            result = assembler.add_fragment(frag.clone())?.or(result);
        }
        assert_eq!(result, Some(instruction));
        Ok(())
    }

    #[test]
    fn retransmitted_fragments_merge_across_attempts() -> Result<(), Box<dyn std::error::Error>> {
        let data: Vec<u8> = (0_u8..200).collect();
        let instruction = TransportInstruction {
            protocol_version: Some(1),
            old_num: Some(1),
            new_num: Some(2),
            ack_num: Some(0),
            throwaway_num: None,
            diff: Some(data),
            chaff: None,
        };
        let mut fragmenter = Fragmenter::default();
        let mtu = 60;
        let first_tx = fragmenter.make_fragments(&instruction, mtu)?;
        assert!(first_tx.len() >= 2);
        let mut assembler = FragmentAssembler::default();
        assembler.add_fragment(first_tx[0].clone())?;
        let mut result = None;
        for frag in &first_tx[1..] {
            result = assembler.add_fragment(frag.clone())?.or(result);
        }
        assert_eq!(result, Some(instruction));
        Ok(())
    }
}
