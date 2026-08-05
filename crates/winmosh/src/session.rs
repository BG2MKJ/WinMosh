use std::io::{self, Write};
use std::net::ToSocketAddrs;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use winmosh_bootstrap::{start_bootstrap, BootstrapRequest};
use winmosh_config::{load_config, resolve_target, ResolvedTarget, DEFAULT_CONNECT_TIMEOUT};
use winmosh_platform::windows::console::ConsoleGuard;
use winmosh_platform::windows::resize::{current_terminal_size, TerminalSize};
use winmosh_protocol::crypto::{CryptoSession, SessionKey as ProtocolSessionKey};
use winmosh_protocol::datagram::ReceiveDisposition;
use winmosh_protocol::fragment::{FragmentAssembler, FragmentError, Fragmenter};
use winmosh_protocol::proto::{HostInstruction, HostMessage, TransportInstruction};
use winmosh_protocol::sequence::Direction;
use winmosh_protocol::statesync::{ReceiveResult, StateSyncReceiver, StateSyncSender};
use winmosh_protocol::timing::{RttEstimator, TimestampClock};
use winmosh_protocol::transport::{encrypted_transport, EncryptedTransport, TransportError};
use winmosh_terminal::{CompleteTerminal, UserInput, UserStream};

use crate::cli::GlobalOptions;
use crate::error::{Error, Result};

pub fn run(global: GlobalOptions, target: String) -> Result<()> {
    let document = load_config(&global.config_path())?;
    let resolved = resolve_target(&document.config, &target, &global.overrides)?;
    if global.bootstrap_only {
        run_bootstrap(&global, &target, resolved)?;
        return Ok(());
    }

    print_resolved_target(&resolved);
    run_interactive(&global, resolved)
}

fn run_bootstrap(global: &GlobalOptions, target: &str, resolved: ResolvedTarget) -> Result<()> {
    let size = current_terminal_size()
        .map(|s| TerminalSize {
            columns: s.columns.max(1),
            rows: s.rows.max(1),
        })
        .unwrap_or(TerminalSize {
            columns: 80,
            rows: 24,
        });
    let request = bootstrap_request(global, resolved, size);
    let result = start_bootstrap(&request)?;
    println!("target: {target}");
    println!("ssh bootstrap: ok");
    println!("remote mosh-server: found");
    println!("udp port: {}", result.udp_port);
    println!("bootstrap succeeded");
    Ok(())
}

fn run_interactive(global: &GlobalOptions, resolved: ResolvedTarget) -> Result<()> {
    let size = current_terminal_size()
        .map(|s| TerminalSize {
            columns: s.columns.max(1),
            rows: s.rows.max(1),
        })
        .unwrap_or(TerminalSize {
            columns: 80,
            rows: 24,
        });
    let request = bootstrap_request(global, resolved.clone(), size);
    let bootstrap = start_bootstrap(&request)?;
    let printable_key = bootstrap
        .session_key
        .as_str()
        .map_err(|error| Error::Protocol(format!("invalid bootstrap session key: {error}")))?;
    let protocol_key = ProtocolSessionKey::from_printable(printable_key)
        .map_err(|error| Error::Protocol(error.to_string()))?;
    let remote = resolve_udp_address(&resolved, bootstrap.udp_port)?;
    let crypto = CryptoSession::new(protocol_key);
    let mut transport =
        encrypted_transport(remote, crypto, Direction::ToServer).map_err(protocol_error)?;
    let clock = TimestampClock::new();
    let mut fragmenter = Fragmenter::default();
    let mut assembler = FragmentAssembler::default();
    let terminal = CompleteTerminal::new(size.columns, size.rows);
    let mut terminal_receiver = StateSyncReceiver::new(terminal);
    let mut user_stream = UserStream::default();
    user_stream.push_resize(size.columns, size.rows);
    let mut user_sender = StateSyncSender::new(UserStream::default());
    user_sender.set_state(user_stream.clone());
    let mut last_remote_timestamp = 0_u16;
    let mut stdout = io::stdout();
    let _console = ConsoleGuard::enter()
        .map_err(|e| Error::Protocol(format!("console setup failed: {e}")))?;
    let _raw_mode = RawModeGuard::enter()?;
    stdout.write_all(b"\x1b[2J\x1b[H")?;
    stdout.flush()?;
    let initial_instruction = user_sender
        .build_instruction(clock.timestamp())
        .map_err(|error| Error::Protocol(error.to_string()))?;
    send_state_instruction(
        &mut transport,
        &mut fragmenter,
        &clock,
        last_remote_timestamp,
        &initial_instruction,
    )?;

    let mut rtt = RttEstimator::default();
    let mut running = true;
    while running {
        if event::poll(Duration::from_millis(10))? {
            running = handle_input_event(
                event::read()?,
                &mut user_stream,
                &mut user_sender,
                &mut transport,
                &mut fragmenter,
                &clock,
                last_remote_timestamp,
            )?;
        }

        if let Some((received, source)) = transport
            .receive(Duration::from_millis(10))
            .map_err(protocol_error)?
        {
            if received.disposition == ReceiveDisposition::Replayed {
                continue;
            }
            transport.note_roaming_source(source);
            last_remote_timestamp = received.datagram.timestamp;
            rtt.observe(Duration::from_millis(100));
            let maybe_instruction = assembler
                .add_wire(&received.datagram.payload)
                .map_err(fragment_error)?;
            if let Some(instruction) = maybe_instruction {
                if let Some(ack_number) = instruction.ack_num {
                    user_sender.acknowledge_local(ack_number);
                }
                let applied = match terminal_receiver.apply_instruction(&instruction) {
                    Ok(applied) => applied,
                    Err(_) => {
                        let rebase_ack = terminal_receiver.ack_instruction();
                        send_state_instruction(
                            &mut transport,
                            &mut fragmenter,
                            &clock,
                            last_remote_timestamp,
                            &rebase_ack,
                        )?;
                        continue;
                    }
                };
                user_sender.acknowledge_remote(terminal_receiver.latest_number());

                if matches!(applied, ReceiveResult::Applied { .. }) {
                    if let Some(diff) = &instruction.diff {
                        if !diff.is_empty() {
                            if let Ok(msg) = HostMessage::decode(diff) {
                                for inst in &msg.instructions {
                                    if let HostInstruction::HostBytes(bytes) = inst {
                                        stdout.write_all(bytes)?;
                                    }
                                }
                                stdout.flush()?;
                            }
                        }
                    }
                }
                let should_ack = matches!(
                    applied,
                    ReceiveResult::Applied { .. } | ReceiveResult::Duplicate { .. }
                );
                if should_ack {
                    let ack = user_sender
                        .build_instruction(clock.timestamp())
                        .map_err(|error| Error::Protocol(error.to_string()))?;
                    send_state_instruction(
                        &mut transport,
                        &mut fragmenter,
                        &clock,
                        last_remote_timestamp,
                        &ack,
                    )?;
                }
                if matches!(
                    terminal_receiver.latest_state(),
                    None
                ) {
                    running = false;
                }
            }
        }

        let timeout_ms = rtt.timeout().0.as_millis() as u64;
        if let Some(instruction) = user_sender.retransmission(clock.timestamp(), timeout_ms) {
            send_state_instruction(
                &mut transport,
                &mut fragmenter,
                &clock,
                last_remote_timestamp,
                &instruction,
            )?;
        }
    }
    Ok(())
}


fn bootstrap_request(
    global: &GlobalOptions,
    target: ResolvedTarget,
    size: TerminalSize,
) -> BootstrapRequest {
    BootstrapRequest {
        target,
        columns: size.columns,
        rows: size.rows,
        locale: bootstrap_locale(),
        ssh_path: global.overrides.ssh_path.clone(),
        timeout: global
            .overrides
            .connect_timeout
            .unwrap_or(DEFAULT_CONNECT_TIMEOUT),
    }
}

fn resolve_udp_address(resolved: &ResolvedTarget, port: u16) -> Result<std::net::SocketAddr> {
    let host = resolved
        .udp_candidates
        .first()
        .map(|candidate| candidate.host.as_str())
        .ok_or_else(|| Error::Protocol("no UDP host candidate was resolved".to_owned()))?;
    (host, port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| Error::Protocol(format!("unable to resolve UDP host: {host}")))
}

fn send_state_instruction(
    transport: &mut EncryptedTransport,
    fragmenter: &mut Fragmenter,
    clock: &TimestampClock,
    timestamp_reply: u16,
    instruction: &TransportInstruction,
) -> Result<()> {
    let fragments = fragmenter
        .make_fragments(instruction, 1400)
        .map_err(fragment_error)?;
    for fragment in fragments {
        let wire = fragment.encode().map_err(fragment_error)?;
        transport
            .send(clock.timestamp16(), timestamp_reply, wire)
            .map_err(protocol_error)?;
    }
    Ok(())
}

fn handle_input_event(
    event: Event,
    user_stream: &mut UserStream,
    user_sender: &mut StateSyncSender<UserStream>,
    transport: &mut EncryptedTransport,
    fragmenter: &mut Fragmenter,
    clock: &TimestampClock,
    timestamp_reply: u16,
) -> Result<bool> {
    match event {
        Event::Paste(text) => {
            if text.is_empty() {
                return Ok(true);
            }
            let mut bytes = Vec::with_capacity(text.len() + 12);
            bytes.extend_from_slice(b"\x1b[200~");
            bytes.extend_from_slice(text.as_bytes());
            bytes.extend_from_slice(b"\x1b[201~");
            user_stream.push_input(UserInput::new(bytes));
            user_sender.set_state(user_stream.clone());
            let instruction = user_sender
                .build_instruction(clock.timestamp())
                .map_err(|error| Error::Protocol(error.to_string()))?;
            send_state_instruction(transport, fragmenter, clock, timestamp_reply, &instruction)?;
        }
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            if is_local_quit(&key) {
                return Ok(false);
            }
            if let Some(bytes) = key_event_bytes(key) {
                user_stream.push_input(UserInput::new(bytes));
                user_sender.set_state(user_stream.clone());
                let instruction = user_sender
                    .build_instruction(clock.timestamp())
                    .map_err(|error| Error::Protocol(error.to_string()))?;
                send_state_instruction(
                    transport,
                    fragmenter,
                    clock,
                    timestamp_reply,
                    &instruction,
                )?;
            }
        }
        Event::Resize(width, height) => {
            let columns = width.max(1);
            let rows = height.max(1);
            user_stream.push_resize(columns, rows);
            user_sender.set_state(user_stream.clone());
            let instruction = user_sender
                .build_instruction(clock.timestamp())
                .map_err(|error| Error::Protocol(error.to_string()))?;
            send_state_instruction(transport, fragmenter, clock, timestamp_reply, &instruction)?;
        }
        _ => {}
    }
    Ok(true)
}

fn is_local_quit(key: &KeyEvent) -> bool {
    key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL)
}

fn key_event_bytes(key: KeyEvent) -> Option<Vec<u8>> {
    let mut bytes = match key.code {
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::Char(character) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                vec![control_byte(character)?]
            } else {
                character.to_string().into_bytes()
            }
        }
        _ => return None,
    };
    if key.modifiers.contains(KeyModifiers::ALT)
        && !matches!(key.code, KeyCode::Esc | KeyCode::Char(_))
    {
        bytes.insert(0, 0x1b);
    }
    Some(bytes)
}

fn control_byte(character: char) -> Option<u8> {
    if !character.is_ascii() {
        return None;
    }
    let byte = character as u8;
    match byte {
        b'@' | b'`' => Some(0x00),
        b'A'..=b'Z' => Some(byte & 0x1f),
        b'a'..=b'z' => Some(byte & 0x1f),
        b'[' => Some(0x1b),
        b'\\' => Some(0x1c),
        b']' => Some(0x1d),
        b'^' | b'~' => Some(0x1e),
        b'_' => Some(0x1f),
        b'?' => Some(0x7f),
        b' ' => Some(0x00),
        _ => None,
    }
}

fn protocol_error(error: TransportError) -> Error {
    Error::Protocol(error.to_string())
}

fn fragment_error(error: FragmentError) -> Error {
    Error::Protocol(error.to_string())
}

struct RawModeGuard;

impl RawModeGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = stdout.write_all(b"\x1b[0m\x1b[?25h\r\n");
        let _ = stdout.flush();
    }
}

fn bootstrap_locale() -> String {
    ["LC_ALL", "LANG"]
        .into_iter()
        .find_map(|name| std::env::var(name).ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "en_US.UTF-8".to_owned())
}

fn print_resolved_target(resolved: &ResolvedTarget) {
    if let Some(alias) = &resolved.alias_name {
        eprintln!("alias: {alias}");
    } else {
        eprintln!("alias: <none>");
    }
    eprintln!("ssh target: {}", resolved.ssh_target);
    eprintln!("effective host: {}", resolved.effective_ssh.hostname);
    eprintln!("effective user: {}", resolved.effective_ssh.user);
    eprintln!("effective ssh port: {}", resolved.effective_ssh.port);
    eprintln!("address family: {}", resolved.effective_ssh.address_family);
    eprintln!("mosh server: {}", resolved.mosh_server);
    eprintln!("udp port preference: {}", resolved.udp_port);
    if let Some(candidate) = resolved.udp_candidates.first() {
        eprintln!("udp host candidate: {}", candidate.host);
    }
    eprintln!("terminal: {}", resolved.terminal);
    eprintln!("prediction: {}", resolved.prediction);
    eprintln!("protocol status: {}", winmosh_protocol::protocol_status());
}

#[cfg(test)]
mod tests {
    use super::{control_byte, is_local_quit, key_event_bytes};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn maps_control_characters_to_terminal_bytes() {
        assert_eq!(control_byte('c'), Some(3));
        assert_eq!(control_byte('Z'), Some(26));
        assert_eq!(control_byte(' '), Some(0));
        assert_eq!(control_byte('['), Some(0x1b));
        assert_eq!(control_byte('?'), Some(0x7f));
        assert_eq!(control_byte('\x00'), None);
    }

    #[test]
    fn maps_navigation_and_enter_keys() {
        assert_eq!(
            key_event_bytes(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
            Some(b"\x1b[D".to_vec())
        );
        assert_eq!(
            key_event_bytes(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some(vec![b'\r'])
        );
    }

    #[test]
    fn reserves_control_q_for_local_shutdown() {
        assert!(is_local_quit(&KeyEvent::new(
            KeyCode::Char('q'),
            KeyModifiers::CONTROL
        )));
        assert!(!is_local_quit(&KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL
        )));
    }
}
