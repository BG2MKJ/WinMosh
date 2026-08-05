use crate::framebuffer::Framebuffer;
use crate::rendition::{Color, Intensity, Rendition};
use winmosh_protocol::proto::{HostInstruction, HostMessage};
use winmosh_protocol::statesync::{StateObject, StateSyncError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParserState {
    Ground,
    Escape,
    Csi,
    Osc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteTerminal {
    pub framebuffer: Framebuffer,
    parser_state: ParserState,
    csi_parameters: String,
    utf8_pending: Vec<u8>,
    saved_cursor: Option<(u16, u16)>,
    echo_ack: u64,
}

impl CompleteTerminal {
    pub fn new(columns: u16, rows: u16) -> Self {
        Self {
            framebuffer: Framebuffer::new(columns, rows),
            parser_state: ParserState::Ground,
            csi_parameters: String::new(),
            utf8_pending: Vec::new(),
            saved_cursor: None,
            echo_ack: 0,
        }
    }

    pub fn act(&mut self, input: &[u8]) -> Vec<u8> {
        let mut response = Vec::new();
        for byte in input {
            match self.parser_state {
                ParserState::Ground => self.handle_ground_byte(*byte),
                ParserState::Escape => self.handle_escape_byte(*byte),
                ParserState::Csi => self.handle_csi_byte(*byte, &mut response),
                ParserState::Osc => {
                    if *byte == 0x07 {
                        self.parser_state = ParserState::Ground;
                    }
                }
            }
        }
        self.flush_utf8_pending();
        response
    }

    pub fn resize(&mut self, columns: u16, rows: u16) {
        self.framebuffer.resize(columns, rows);
    }

    pub fn reset(&mut self) {
        let size = self.framebuffer.size;
        self.framebuffer = Framebuffer::new(size.columns, size.rows);
        self.parser_state = ParserState::Ground;
        self.csi_parameters.clear();
        self.utf8_pending.clear();
        self.saved_cursor = None;
        self.echo_ack = 0;
    }

    pub fn echo_ack(&self) -> u64 {
        self.echo_ack
    }

    pub fn set_echo_ack(&mut self, value: u64) {
        self.echo_ack = self.echo_ack.max(value);
    }

    fn handle_ground_byte(&mut self, byte: u8) {
        match byte {
            0x1b => self.parser_state = ParserState::Escape,
            b'\r' => self.framebuffer.cursor.column = 0,
            b'\n' => self.framebuffer.line_feed(),
            0x08 => self.framebuffer.backspace(),
            b'\t' => {
                let next = ((self.framebuffer.cursor.column / 8) + 1) * 8;
                self.framebuffer.cursor.column =
                    next.min(self.framebuffer.size.columns.saturating_sub(1));
            }
            0x07 | 0x00..=0x1f | 0x7f => {}
            _ => self.push_utf8_byte(byte),
        }
    }

    fn handle_escape_byte(&mut self, byte: u8) {
        match byte {
            b'[' => {
                self.csi_parameters.clear();
                self.parser_state = ParserState::Csi;
            }
            b']' => self.parser_state = ParserState::Osc,
            b'7' => {
                self.saved_cursor =
                    Some((self.framebuffer.cursor.column, self.framebuffer.cursor.row));
                self.parser_state = ParserState::Ground;
            }
            b'8' => {
                if let Some((column, row)) = self.saved_cursor {
                    self.framebuffer.set_cursor(column, row);
                }
                self.parser_state = ParserState::Ground;
            }
            b'c' => {
                self.reset();
            }
            0x1b => {}
            _ => self.parser_state = ParserState::Ground,
        }
    }

    fn handle_csi_byte(&mut self, byte: u8, response: &mut Vec<u8>) {
        if byte.is_ascii_digit() || matches!(byte, b';' | b'?' | b'>') {
            self.csi_parameters.push(char::from(byte));
            if self.csi_parameters.len() > 64 {
                self.csi_parameters.clear();
                self.parser_state = ParserState::Ground;
            }
            return;
        }
        if (0x40..=0x7e).contains(&byte) {
            self.execute_csi(byte as char, response);
        } else {
            self.parser_state = ParserState::Ground;
            self.csi_parameters.clear();
        }
    }

    fn execute_csi(&mut self, command: char, response: &mut Vec<u8>) {
        let private = self.csi_parameters.starts_with('?');
        let parameters = self
            .csi_parameters
            .trim_start_matches(['?', '>'])
            .split(';')
            .map(|value| value.parse::<u16>().unwrap_or(0))
            .collect::<Vec<_>>();
        let first = parameter(&parameters, 0, 1);
        let second = parameter(&parameters, 1, 1);
        match command {
            'A' => self.framebuffer.cursor.row = self.framebuffer.cursor.row.saturating_sub(first),
            'B' | 'e' => {
                self.framebuffer.cursor.row = (self.framebuffer.cursor.row + first)
                    .min(self.framebuffer.size.rows.saturating_sub(1));
            }
            'C' | 'a' => {
                self.framebuffer.cursor.column = (self.framebuffer.cursor.column + first)
                    .min(self.framebuffer.size.columns.saturating_sub(1));
            }
            'D' => {
                self.framebuffer.cursor.column =
                    self.framebuffer.cursor.column.saturating_sub(first)
            }
            'G' | '`' => {
                self.framebuffer.cursor.column = first
                    .saturating_sub(1)
                    .min(self.framebuffer.size.columns.saturating_sub(1))
            }
            'd' => {
                self.framebuffer.cursor.row = first
                    .saturating_sub(1)
                    .min(self.framebuffer.size.rows.saturating_sub(1))
            }
            'H' | 'f' => self
                .framebuffer
                .set_cursor(second.saturating_sub(1), first.saturating_sub(1)),
            'J' => match first {
                0 => self.framebuffer.clear_screen_from_cursor(),
                1 => self.framebuffer.clear_screen_to_cursor(),
                2 | 3 => self.framebuffer.clear(),
                _ => {}
            },
            'K' => match first {
                0 => self.framebuffer.clear_line_from_cursor(),
                1 => self.framebuffer.clear_line_to_cursor(),
                2 => self.framebuffer.clear_line(),
                _ => {}
            },
            'm' => self.apply_sgr(&parameters),
            'n' if first == 6 => {
                response.extend_from_slice(
                    format!(
                        "\x1b[{};{}R",
                        self.framebuffer.cursor.row + 1,
                        self.framebuffer.cursor.column + 1
                    )
                    .as_bytes(),
                );
            }
            's' => {
                self.saved_cursor =
                    Some((self.framebuffer.cursor.column, self.framebuffer.cursor.row));
            }
            'u' => {
                if let Some((column, row)) = self.saved_cursor {
                    self.framebuffer.set_cursor(column, row);
                }
            }
            'h' if private && first == 25 => self.framebuffer.cursor.visible = true,
            'l' if private && first == 25 => self.framebuffer.cursor.visible = false,
            'L' => self.framebuffer.insert_lines(first),
            'M' => self.framebuffer.delete_lines(first),
            'P' => self.delete_characters(first),
            '@' => self.insert_characters(first),
            'X' => self.framebuffer.erase_characters(first),
            _ => {}
        }
        self.csi_parameters.clear();
        self.parser_state = ParserState::Ground;
    }

    fn apply_sgr(&mut self, parameters: &[u16]) {
        if parameters.is_empty() || parameters == [0] {
            self.framebuffer.rendition = Rendition::default();
            return;
        }
        let mut index = 0;
        while index < parameters.len() {
            match parameters[index] {
                0 => self.framebuffer.rendition = Rendition::default(),
                1 => self.framebuffer.rendition.intensity = Intensity::Bold,
                22 => self.framebuffer.rendition.intensity = Intensity::Normal,
                4 => self.framebuffer.rendition.underline = true,
                24 => self.framebuffer.rendition.underline = false,
                7 => self.framebuffer.rendition.inverse = true,
                27 => self.framebuffer.rendition.inverse = false,
                30..=37 => {
                    self.framebuffer.rendition.foreground =
                        Color::Indexed((parameters[index] - 30) as u8)
                }
                39 => self.framebuffer.rendition.foreground = Color::Default,
                40..=47 => {
                    self.framebuffer.rendition.background =
                        Color::Indexed((parameters[index] - 40) as u8)
                }
                49 => self.framebuffer.rendition.background = Color::Default,
                90..=97 => {
                    self.framebuffer.rendition.foreground =
                        Color::Indexed((parameters[index] - 90 + 8) as u8)
                }
                100..=107 => {
                    self.framebuffer.rendition.background =
                        Color::Indexed((parameters[index] - 100 + 8) as u8)
                }
                38 | 48 if index + 2 < parameters.len() && parameters[index + 1] == 5 => {
                    let color = Color::Indexed(parameters[index + 2] as u8);
                    if parameters[index] == 38 {
                        self.framebuffer.rendition.foreground = color;
                    } else {
                        self.framebuffer.rendition.background = color;
                    }
                    index += 2;
                }
                38 | 48 if index + 4 < parameters.len() && parameters[index + 1] == 2 => {
                    let r = (parameters[index + 2] & 0xff) as u8;
                    let g = (parameters[index + 3] & 0xff) as u8;
                    let b = (parameters[index + 4] & 0xff) as u8;
                    let color = Color::Indexed(if r == g && g == b {
                        232 + (u16::from(r) * 24 / 256) as u8
                    } else {
                        16 + 36 * (r / 51) + 6 * (g / 51) + (b / 51)
                    });
                    if parameters[index] == 38 {
                        self.framebuffer.rendition.foreground = color;
                    } else {
                        self.framebuffer.rendition.background = color;
                    }
                    index += 4;
                }
                _ => {}
            }
            index += 1;
        }
    }

    fn delete_characters(&mut self, count: u16) {
        let row = self.framebuffer.cursor.row;
        let start = self.framebuffer.cursor.column;
        let count = count.min(self.framebuffer.size.columns.saturating_sub(start));
        for column in start..(self.framebuffer.size.columns - count) {
            let source = self
                .framebuffer
                .cell(column + count, row)
                .cloned()
                .unwrap_or_default();
            self.framebuffer.set_cursor(column, row);
            self.framebuffer.put(source.text);
        }
        self.framebuffer.set_cursor(start, row);
        self.framebuffer.clear_line_from_cursor();
    }

    fn insert_characters(&mut self, count: u16) {
        let row = self.framebuffer.cursor.row;
        let start = self.framebuffer.cursor.column;
        let count = count.min(self.framebuffer.size.columns.saturating_sub(start));
        for column in (start..(self.framebuffer.size.columns - count)).rev() {
            let source = self
                .framebuffer
                .cell(column, row)
                .cloned()
                .unwrap_or_default();
            self.framebuffer.set_cursor(column + count, row);
            self.framebuffer.put(source.text);
        }
        self.framebuffer.set_cursor(start, row);
        self.framebuffer.clear_line_from_cursor();
    }

    fn push_utf8_byte(&mut self, byte: u8) {
        self.utf8_pending.push(byte);
        loop {
            match std::str::from_utf8(&self.utf8_pending) {
                Ok(text) => {
                    let text = text.to_owned();
                    self.utf8_pending.clear();
                    for character in text.chars() {
                        self.framebuffer.put(character.to_string());
                    }
                    break;
                }
                Err(error) if error.error_len().is_none() => break,
                Err(error) => {
                    let valid = error.valid_up_to();
                    if valid > 0 {
                        let text = String::from_utf8_lossy(&self.utf8_pending[..valid]).to_string();
                        for character in text.chars() {
                            self.framebuffer.put(character.to_string());
                        }
                    }
                    self.utf8_pending.drain(..valid.saturating_add(1));
                    self.framebuffer.put("�".to_owned());
                    if self.utf8_pending.is_empty() {
                        break;
                    }
                }
            }
        }
    }

    fn flush_utf8_pending(&mut self) {
        if !self.utf8_pending.is_empty() {
            let pending = std::mem::take(&mut self.utf8_pending);
            let text = String::from_utf8_lossy(&pending);
            for character in text.chars() {
                self.framebuffer.put(character.to_string());
            }
        }
    }
}

impl StateObject for CompleteTerminal {
    fn diff_from(&self, existing: &Self) -> Result<Vec<u8>, StateSyncError> {
        let mut instructions = Vec::new();
        if existing.framebuffer.size != self.framebuffer.size {
            instructions.push(HostInstruction::Resize {
                width: i32::from(self.framebuffer.size.columns),
                height: i32::from(self.framebuffer.size.rows),
            });
        }
        if existing.framebuffer != self.framebuffer {
            instructions.push(HostInstruction::HostBytes(
                crate::renderer::render_framebuffer(&self.framebuffer).into_bytes(),
            ));
        }
        if existing.echo_ack != self.echo_ack {
            instructions.push(HostInstruction::EchoAck(self.echo_ack));
        }
        Ok(HostMessage { instructions }.encode())
    }

    fn apply_diff(&mut self, diff: &[u8]) -> Result<(), StateSyncError> {
        let message = HostMessage::decode(diff)
            .map_err(|error| StateSyncError::InvalidDiff(error.to_string()))?;
        for instruction in message.instructions {
            match instruction {
                HostInstruction::HostBytes(bytes) => {
                    self.act(&bytes);
                }
                HostInstruction::Resize { width, height } => {
                    let columns = u16::try_from(width)
                        .map_err(|_| StateSyncError::InvalidDiff("invalid columns".to_owned()))?;
                    let rows = u16::try_from(height)
                        .map_err(|_| StateSyncError::InvalidDiff("invalid rows".to_owned()))?;
                    self.resize(columns, rows);
                }
                HostInstruction::EchoAck(value) => self.set_echo_ack(value),
            }
        }
        Ok(())
    }
}

fn parameter(parameters: &[u16], index: usize, default: u16) -> u16 {
    parameters
        .get(index)
        .copied()
        .filter(|value| *value != 0)
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::CompleteTerminal;
    use crate::rendition::{Color, Intensity};
    use winmosh_protocol::proto::{HostInstruction, HostMessage};

    #[test]
    fn renders_text_and_cursor_motion() {
        let mut terminal = CompleteTerminal::new(10, 2);
        terminal.act(b"hello\x1b[2;3Hworld");
        assert_eq!(
            terminal
                .framebuffer
                .cell(2, 1)
                .map(|cell| cell.text.as_str()),
            Some("w")
        );
        assert_eq!(terminal.framebuffer.cursor.column, 7);
        assert_eq!(terminal.framebuffer.cursor.row, 1);
    }

    #[test]
    fn applies_sgr_and_visibility() {
        let mut terminal = CompleteTerminal::new(10, 1);
        terminal.act(b"\x1b[1;38;5;42mX\x1b[?25l");
        let cell = terminal.framebuffer.cell(0, 0).expect("cell");
        assert_eq!(cell.rendition.intensity, Intensity::Bold);
        assert_eq!(cell.rendition.foreground, Color::Indexed(42));
        assert!(!terminal.framebuffer.cursor.visible);
    }

    #[test]
    fn responds_to_cursor_query() {
        let mut terminal = CompleteTerminal::new(10, 2);
        let response = terminal.act(b"\x1b[6n");
        assert_eq!(response, b"\x1b[1;1R");
    }

    #[test]
    fn terminal_state_round_trips_as_host_message() -> Result<(), Box<dyn std::error::Error>> {
        use winmosh_protocol::statesync::StateObject;

        let mut source = CompleteTerminal::new(10, 2);
        source.act(b"hello");
        let diff = source.diff_from(&CompleteTerminal::new(10, 2))?;
        let message = HostMessage::decode(&diff)?;
        assert!(matches!(
            &message.instructions[0],
            HostInstruction::HostBytes(bytes) if bytes.contains(&b'h')
        ));
        let mut target = CompleteTerminal::new(10, 2);
        target.apply_diff(&diff)?;
        assert_eq!(
            target.framebuffer.cell(0, 0).map(|cell| cell.text.as_str()),
            Some("h")
        );
        Ok(())
    }

    #[test]
    fn insert_lines_shifts_content_down() {
        let mut t = CompleteTerminal::new(4, 3);
        t.act(b"top\x1b[2;1Hmid\x1b[3;1Hbot");
        t.act(b"\x1b[2;1H\x1b[L");
        assert_eq!(t.framebuffer.cell(0, 0).map(|c| c.text.as_str()), Some("t"));
        assert_eq!(t.framebuffer.cell(0, 2).map(|c| c.text.as_str()), Some("m"));
    }

    #[test]
    fn delete_lines_shifts_content_up() {
        let mut t = CompleteTerminal::new(4, 3);
        t.act(b"top\x1b[2;1Hmid\x1b[3;1Hbot");
        t.act(b"\x1b[2;1H\x1b[M");
        assert_eq!(t.framebuffer.cell(0, 0).map(|c| c.text.as_str()), Some("t"));
        assert_eq!(t.framebuffer.cell(0, 1).map(|c| c.text.as_str()), Some("b"));
    }

    #[test]
    fn erase_characters_clears_without_moving() {
        let mut t = CompleteTerminal::new(6, 1);
        t.act(b"abcdef\x1b[1;1H\x1b[3X");
        assert_eq!(t.framebuffer.cell(0, 0).map(|c| c.text.as_str()), Some(" "));
        assert_eq!(t.framebuffer.cell(3, 0).map(|c| c.text.as_str()), Some("d"));
    }

    #[test]
    fn truecolor_sgr_sets_color() {
        let mut t = CompleteTerminal::new(10, 1);
        t.act(b"\x1b[38;2;255;0;0m\x1b[48;2;0;128;0mX");
        let cell = t.framebuffer.cell(0, 0).expect("cell");
        assert_ne!(cell.rendition.foreground, Color::Default);
        assert_ne!(cell.rendition.background, Color::Default);
    }
}
