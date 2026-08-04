use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtoError {
    Truncated,
    InvalidVarint,
    InvalidWireType(u8),
    InvalidLength,
    InvalidField(&'static str),
}

impl fmt::Display for ProtoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => formatter.write_str("truncated protobuf message"),
            Self::InvalidVarint => formatter.write_str("invalid protobuf varint"),
            Self::InvalidWireType(value) => {
                write!(formatter, "invalid protobuf wire type: {value}")
            }
            Self::InvalidLength => formatter.write_str("invalid protobuf length"),
            Self::InvalidField(name) => write!(formatter, "invalid protobuf field: {name}"),
        }
    }
}

impl std::error::Error for ProtoError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserInstruction {
    Keystroke(Vec<u8>),
    Resize { width: i32, height: i32 },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserMessage {
    pub instructions: Vec<UserInstruction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostInstruction {
    HostBytes(Vec<u8>),
    Resize { width: i32, height: i32 },
    EchoAck(u64),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostMessage {
    pub instructions: Vec<HostInstruction>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransportInstruction {
    pub protocol_version: Option<u32>,
    pub old_num: Option<u64>,
    pub new_num: Option<u64>,
    pub ack_num: Option<u64>,
    pub throwaway_num: Option<u64>,
    pub diff: Option<Vec<u8>>,
    pub chaff: Option<Vec<u8>>,
}

impl UserMessage {
    pub fn encode(&self) -> Vec<u8> {
        let mut output = Vec::new();
        for instruction in &self.instructions {
            let body = encode_user_instruction(instruction);
            put_bytes(&mut output, 1, &body);
        }
        output
    }

    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        let fields = parse_fields(input)?;
        let mut message = Self::default();
        for field in fields {
            if field.number == 1 && field.wire_type == 2 {
                message
                    .instructions
                    .push(decode_user_instruction(field.bytes)?);
            }
        }
        Ok(message)
    }
}

impl HostMessage {
    pub fn encode(&self) -> Vec<u8> {
        let mut output = Vec::new();
        for instruction in &self.instructions {
            let body = encode_host_instruction(instruction);
            put_bytes(&mut output, 1, &body);
        }
        output
    }

    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        let fields = parse_fields(input)?;
        let mut message = Self::default();
        for field in fields {
            if field.number == 1 && field.wire_type == 2 {
                message
                    .instructions
                    .push(decode_host_instruction(field.bytes)?);
            }
        }
        Ok(message)
    }
}

impl TransportInstruction {
    pub fn encode(&self) -> Vec<u8> {
        let mut output = Vec::new();
        if let Some(value) = self.protocol_version {
            put_varint(&mut output, 1, value as u64);
        }
        if let Some(value) = self.old_num {
            put_varint(&mut output, 2, value);
        }
        if let Some(value) = self.new_num {
            put_varint(&mut output, 3, value);
        }
        if let Some(value) = self.ack_num {
            put_varint(&mut output, 4, value);
        }
        if let Some(value) = self.throwaway_num {
            put_varint(&mut output, 5, value);
        }
        if let Some(value) = &self.diff {
            put_bytes(&mut output, 6, value);
        }
        if let Some(value) = &self.chaff {
            put_bytes(&mut output, 7, value);
        }
        output
    }

    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        let fields = parse_fields(input)?;
        let mut message = Self::default();
        for field in fields {
            match (field.number, field.wire_type) {
                (1, 0) => message.protocol_version = Some(field.varint as u32),
                (2, 0) => message.old_num = Some(field.varint),
                (3, 0) => message.new_num = Some(field.varint),
                (4, 0) => message.ack_num = Some(field.varint),
                (5, 0) => message.throwaway_num = Some(field.varint),
                (6, 2) => message.diff = Some(field.bytes.to_vec()),
                (7, 2) => message.chaff = Some(field.bytes.to_vec()),
                _ => {}
            }
        }
        Ok(message)
    }
}

#[derive(Debug)]
struct Field<'a> {
    number: u32,
    wire_type: u8,
    varint: u64,
    bytes: &'a [u8],
}

fn encode_user_instruction(instruction: &UserInstruction) -> Vec<u8> {
    let mut extension = Vec::new();
    match instruction {
        UserInstruction::Keystroke(keys) => {
            put_bytes(&mut extension, 2, &encode_bytes_field(4, keys));
        }
        UserInstruction::Resize { width, height } => {
            put_bytes(&mut extension, 3, &encode_resize_extension(*width, *height));
        }
    }
    extension
}

fn encode_host_instruction(instruction: &HostInstruction) -> Vec<u8> {
    let mut extension = Vec::new();
    match instruction {
        HostInstruction::HostBytes(bytes) => {
            put_bytes(&mut extension, 2, &encode_bytes_field(4, bytes));
        }
        HostInstruction::Resize { width, height } => {
            put_bytes(&mut extension, 3, &encode_resize_extension(*width, *height));
        }
        HostInstruction::EchoAck(value) => {
            let mut body = Vec::new();
            put_varint(&mut body, 8, *value);
            put_bytes(&mut extension, 7, &body);
        }
    }
    extension
}

fn decode_user_instruction(input: &[u8]) -> Result<UserInstruction, ProtoError> {
    let fields = parse_fields(input)?;
    for field in fields {
        match (field.number, field.wire_type) {
            (2, 2) => return decode_bytes_extension(field.bytes).map(UserInstruction::Keystroke),
            (3, 2) => {
                let (width, height) = decode_resize_extension(field.bytes)?;
                return Ok(UserInstruction::Resize { width, height });
            }
            _ => {}
        }
    }
    Err(ProtoError::InvalidField("user instruction extension"))
}

fn decode_host_instruction(input: &[u8]) -> Result<HostInstruction, ProtoError> {
    let fields = parse_fields(input)?;
    for field in fields {
        match (field.number, field.wire_type) {
            (2, 2) => return decode_bytes_extension(field.bytes).map(HostInstruction::HostBytes),
            (3, 2) => {
                let (width, height) = decode_resize_extension(field.bytes)?;
                return Ok(HostInstruction::Resize { width, height });
            }
            (7, 2) => {
                let nested = parse_fields(field.bytes)?;
                let value = nested
                    .into_iter()
                    .find(|nested_field| nested_field.number == 8 && nested_field.wire_type == 0)
                    .ok_or(ProtoError::InvalidField("echo ack"))?;
                return Ok(HostInstruction::EchoAck(value.varint));
            }
            _ => {}
        }
    }
    Err(ProtoError::InvalidField("host instruction extension"))
}

fn encode_bytes_field(number: u32, bytes: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    put_bytes(&mut output, number, bytes);
    output
}

fn encode_resize_extension(width: i32, height: i32) -> Vec<u8> {
    let mut output = Vec::new();
    put_varint(&mut output, 5, width as i64 as u64);
    put_varint(&mut output, 6, height as i64 as u64);
    output
}

fn decode_bytes_extension(input: &[u8]) -> Result<Vec<u8>, ProtoError> {
    let field = parse_fields(input)?
        .into_iter()
        .find(|field| field.number == 4 && field.wire_type == 2)
        .ok_or(ProtoError::InvalidField("bytes"))?;
    Ok(field.bytes.to_vec())
}

fn decode_resize_extension(input: &[u8]) -> Result<(i32, i32), ProtoError> {
    let fields = parse_fields(input)?;
    let width = fields
        .iter()
        .find(|field| field.number == 5 && field.wire_type == 0)
        .ok_or(ProtoError::InvalidField("resize width"))?
        .varint;
    let height = fields
        .iter()
        .find(|field| field.number == 6 && field.wire_type == 0)
        .ok_or(ProtoError::InvalidField("resize height"))?
        .varint;
    Ok((width as i32, height as i32))
}

fn put_varint(output: &mut Vec<u8>, number: u32, value: u64) {
    put_key(output, number, 0);
    put_raw_varint(output, value);
}

fn put_bytes(output: &mut Vec<u8>, number: u32, bytes: &[u8]) {
    put_key(output, number, 2);
    put_raw_varint(output, bytes.len() as u64);
    output.extend_from_slice(bytes);
}

fn put_key(output: &mut Vec<u8>, number: u32, wire_type: u8) {
    put_raw_varint(output, ((number << 3) | u32::from(wire_type)) as u64);
}

fn put_raw_varint(output: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        output.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    output.push(value as u8);
}

fn parse_fields(mut input: &[u8]) -> Result<Vec<Field<'_>>, ProtoError> {
    let mut fields = Vec::new();
    while !input.is_empty() {
        let key = read_varint(&mut input)?;
        let number = u32::try_from(key >> 3).map_err(|_| ProtoError::InvalidVarint)?;
        let wire_type = (key & 0x07) as u8;
        if number == 0 {
            return Err(ProtoError::InvalidVarint);
        }
        match wire_type {
            0 => fields.push(Field {
                number,
                wire_type,
                varint: read_varint(&mut input)?,
                bytes: &[],
            }),
            1 => {
                if input.len() < 8 {
                    return Err(ProtoError::Truncated);
                }
                let bytes = &input[..8];
                input = &input[8..];
                fields.push(Field {
                    number,
                    wire_type,
                    varint: 0,
                    bytes,
                });
            }
            2 => {
                let length = usize::try_from(read_varint(&mut input)?)
                    .map_err(|_| ProtoError::InvalidLength)?;
                if input.len() < length {
                    return Err(ProtoError::Truncated);
                }
                let bytes = &input[..length];
                input = &input[length..];
                fields.push(Field {
                    number,
                    wire_type,
                    varint: 0,
                    bytes,
                });
            }
            5 => {
                if input.len() < 4 {
                    return Err(ProtoError::Truncated);
                }
                let bytes = &input[..4];
                input = &input[4..];
                fields.push(Field {
                    number,
                    wire_type,
                    varint: 0,
                    bytes,
                });
            }
            other => return Err(ProtoError::InvalidWireType(other)),
        }
    }
    Ok(fields)
}

fn read_varint(input: &mut &[u8]) -> Result<u64, ProtoError> {
    let mut value = 0_u64;
    for shift in (0..64).step_by(7) {
        let byte = *input.first().ok_or(ProtoError::Truncated)?;
        *input = &input[1..];
        let chunk = u64::from(byte & 0x7f);
        value |= chunk << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(ProtoError::InvalidVarint)
}

#[cfg(test)]
mod tests {
    use super::{HostInstruction, HostMessage, TransportInstruction, UserInstruction, UserMessage};

    #[test]
    fn user_message_matches_proto2_extension_shape() -> Result<(), Box<dyn std::error::Error>> {
        let message = UserMessage {
            instructions: vec![UserInstruction::Keystroke(b"ls\n".to_vec())],
        };
        let encoded = message.encode();
        assert_eq!(
            encoded,
            vec![0x0a, 0x07, 0x12, 0x05, 0x22, 0x03, b'l', b's', b'\n']
        );
        assert_eq!(UserMessage::decode(&encoded)?, message);
        Ok(())
    }

    #[test]
    fn host_message_round_trips_all_instruction_types() -> Result<(), Box<dyn std::error::Error>> {
        let message = HostMessage {
            instructions: vec![
                HostInstruction::HostBytes(b"output".to_vec()),
                HostInstruction::Resize {
                    width: 80,
                    height: 24,
                },
                HostInstruction::EchoAck(42),
            ],
        };
        assert_eq!(HostMessage::decode(&message.encode())?, message);
        Ok(())
    }

    #[test]
    fn transport_instruction_round_trips() -> Result<(), Box<dyn std::error::Error>> {
        let message = TransportInstruction {
            protocol_version: Some(1),
            old_num: Some(4),
            new_num: Some(5),
            ack_num: Some(3),
            throwaway_num: None,
            diff: Some(vec![1, 2, 3]),
            chaff: Some(vec![4, 5]),
        };
        assert_eq!(TransportInstruction::decode(&message.encode())?, message);
        Ok(())
    }

    #[test]
    fn resize_accepts_signed_int32_values() -> Result<(), Box<dyn std::error::Error>> {
        let message = UserMessage {
            instructions: vec![UserInstruction::Resize {
                width: -1,
                height: 24,
            }],
        };
        assert_eq!(UserMessage::decode(&message.encode())?, message);
        Ok(())
    }
}
