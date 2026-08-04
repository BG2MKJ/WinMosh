use winmosh_protocol::proto::{UserInstruction, UserMessage};
use winmosh_protocol::statesync::{StateObject, StateSyncError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserInput {
    pub bytes: Vec<u8>,
}

impl UserInput {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    pub fn from_text(text: &str) -> Self {
        Self::new(text.as_bytes().to_vec())
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserEvent {
    Bytes(Vec<u8>),
    Resize { columns: u16, rows: u16 },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserStream {
    events: Vec<UserEvent>,
}

impl UserStream {
    pub fn push_input(&mut self, input: UserInput) {
        if input.is_empty() {
            return;
        }
        if let Some(UserEvent::Bytes(bytes)) = self.events.last_mut() {
            bytes.extend_from_slice(&input.bytes);
        } else {
            self.events.push(UserEvent::Bytes(input.bytes));
        }
    }

    pub fn push_resize(&mut self, columns: u16, rows: u16) {
        self.events.push(UserEvent::Resize { columns, rows });
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn events(&self) -> &[UserEvent] {
        &self.events
    }
}

impl StateObject for UserStream {
    fn diff_from(&self, existing: &Self) -> Result<Vec<u8>, StateSyncError> {
        match self.try_diff_from(existing) {
            Ok(diff) => Ok(diff),
            Err(error) => {
                eprintln!(
                    "warning: SSP user diff failed ({}), falling back to full state \
                     (existing {} events, current {} events)",
                    error,
                    existing.events.len(),
                    self.events.len(),
                );
                self.try_diff_from(&UserStream::default())
            }
        }
    }

    fn apply_diff(&mut self, diff: &[u8]) -> Result<(), StateSyncError> {
        let message = UserMessage::decode(diff)
            .map_err(|error| StateSyncError::InvalidDiff(error.to_string()))?;
        for instruction in message.instructions {
            match instruction {
                UserInstruction::Keystroke(bytes) => self.push_input(UserInput::new(bytes)),
                UserInstruction::Resize { width, height } => {
                    if width <= 0 || height <= 0 {
                        return Err(StateSyncError::InvalidDiff(
                            "resize dimensions must be positive".to_owned(),
                        ));
                    }
                    let columns = u16::try_from(width)
                        .map_err(|_| StateSyncError::InvalidDiff("invalid columns".to_owned()))?;
                    let rows = u16::try_from(height)
                        .map_err(|_| StateSyncError::InvalidDiff("invalid rows".to_owned()))?;
                    self.push_resize(columns, rows);
                }
            }
        }
        Ok(())
    }
}

impl UserStream {
    fn try_diff_from(&self, existing: &Self) -> Result<Vec<u8>, StateSyncError> {
        if existing.events.len() > self.events.len() {
            return Err(StateSyncError::InvalidDiff(
                "user stream base is not a prefix".to_owned(),
            ));
        }
        let mut instructions = Vec::new();
        let common_event_count = existing.events.len();
        for index in 0..common_event_count {
            let same_length_last_event =
                existing.events.len() == self.events.len() && index + 1 == existing.events.len();
            let shorter_last_event =
                existing.events.len() < self.events.len() && index + 1 == existing.events.len();
            if same_length_last_event {
                match (&existing.events[index], &self.events[index]) {
                    (UserEvent::Bytes(existing_bytes), UserEvent::Bytes(current_bytes))
                        if current_bytes.starts_with(existing_bytes) =>
                    {
                        append_keystrokes(
                            &mut instructions,
                            &current_bytes[existing_bytes.len()..],
                        );
                    }
                    (existing_event, current_event) if existing_event == current_event => {}
                    _ => {
                        return Err(StateSyncError::InvalidDiff(
                            "user stream base is not a prefix".to_owned(),
                        ));
                    }
                }
            } else if shorter_last_event {
                match (&existing.events[index], &self.events[index]) {
                    (UserEvent::Bytes(existing_bytes), UserEvent::Bytes(current_bytes))
                        if current_bytes.starts_with(existing_bytes) =>
                    {
                        append_keystrokes(
                            &mut instructions,
                            &current_bytes[existing_bytes.len()..],
                        );
                    }
                    (existing_event, current_event) if existing_event == current_event => {}
                    _ => {
                        return Err(StateSyncError::InvalidDiff(
                            "user stream base is not a prefix".to_owned(),
                        ));
                    }
                }
            } else if existing.events[index] != self.events[index] {
                return Err(StateSyncError::InvalidDiff(
                    "user stream base is not a prefix".to_owned(),
                ));
            }
        }
        for event in &self.events[existing.events.len()..] {
            match event {
                UserEvent::Bytes(bytes) => append_keystrokes(&mut instructions, bytes),
                UserEvent::Resize { columns, rows } => instructions.push(UserInstruction::Resize {
                    width: i32::from(*columns),
                    height: i32::from(*rows),
                }),
            }
        }
        Ok(UserMessage { instructions }.encode())
    }
}

fn append_keystrokes(instructions: &mut Vec<UserInstruction>, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    if let Some(UserInstruction::Keystroke(previous)) = instructions.last_mut() {
        previous.extend_from_slice(bytes);
    } else {
        instructions.push(UserInstruction::Keystroke(bytes.to_vec()));
    }
}

#[cfg(test)]
mod tests {
    use super::{UserEvent, UserInput, UserStream};
    use winmosh_protocol::proto::{UserInstruction, UserMessage};
    use winmosh_protocol::statesync::StateObject;

    #[test]
    fn coalesces_keystrokes_and_round_trips_wire_diff() -> Result<(), Box<dyn std::error::Error>> {
        let mut source = UserStream::default();
        source.push_input(UserInput::from_text("l"));
        source.push_input(UserInput::from_text("s\n"));
        source.push_resize(80, 24);
        let diff = source.diff_from(&UserStream::default())?;
        let mut target = UserStream::default();
        target.apply_diff(&diff)?;
        assert_eq!(target.events(), source.events());
        assert!(matches!(target.events()[0], UserEvent::Bytes(_)));
        Ok(())
    }

    #[test]
    fn allows_extending_the_last_keystroke_event() -> Result<(), Box<dyn std::error::Error>> {
        let mut existing = UserStream::default();
        existing.push_input(UserInput::from_text("l"));
        let mut current = existing.clone();
        current.push_input(UserInput::from_text("s"));

        let diff = current.diff_from(&existing)?;
        let message = UserMessage::decode(&diff)?;
        assert_eq!(
            message.instructions,
            vec![UserInstruction::Keystroke(b"s".to_vec())]
        );
        Ok(())
    }

    #[test]
    fn allows_bytes_extension_followed_by_new_events() -> Result<(), Box<dyn std::error::Error>> {
        let mut existing = UserStream::default();
        existing.push_input(UserInput::from_text("a"));
        let mut current = existing.clone();
        current.push_input(UserInput::from_text("b"));
        current.push_resize(100, 40);

        let diff = current.diff_from(&existing)?;
        let message = UserMessage::decode(&diff)?;
        assert_eq!(message.instructions.len(), 2);
        assert_eq!(
            message.instructions[0],
            UserInstruction::Keystroke(b"b".to_vec())
        );
        assert_eq!(
            message.instructions[1],
            UserInstruction::Resize {
                width: 100,
                height: 40
            }
        );
        Ok(())
    }

    #[test]
    fn diff_falls_back_to_empty_base_on_bad_prefix() -> Result<(), Box<dyn std::error::Error>> {
        let mut existing = UserStream::default();
        existing.push_input(UserInput::from_text("x"));
        let mut current = UserStream::default();
        current.push_resize(80, 24);
        current.push_input(UserInput::from_text("y"));

        let diff = current.diff_from(&existing)?;
        let message = UserMessage::decode(&diff)?;
        assert!(!message.instructions.is_empty());
        Ok(())
    }
}
