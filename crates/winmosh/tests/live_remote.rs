use std::error::Error;
use std::net::ToSocketAddrs;
use std::time::{Duration, Instant};

use winmosh_bootstrap::{start_bootstrap, BootstrapRequest};
use winmosh_config::{
    AddressFamily, EffectiveSshConfig, PortSpec, PredictionMode, ResolvedTarget, SocketCandidate,
};
use winmosh_protocol::crypto::{CryptoSession, SessionKey};
use winmosh_protocol::fragment::{FragmentAssembler, Fragmenter};
use winmosh_protocol::sequence::Direction;
use winmosh_protocol::statesync::{ReceiveResult, StateSyncReceiver, StateSyncSender};
use winmosh_protocol::timing::TimestampClock;
use winmosh_protocol::transport::encrypted_transport;
use winmosh_terminal::{CompleteTerminal, UserStream};

#[test]
#[ignore = "requires a reachable Linux mosh-server and SSH key setup"]
fn connects_to_live_mosh_server() -> Result<(), Box<dyn Error>> {
    let ssh_target = std::env::var("WINMOSH_LIVE_SSH_TARGET")?;
    let udp_host = std::env::var("WINMOSH_LIVE_UDP_HOST")?;
    let locale = std::env::var("WINMOSH_LIVE_LOCALE").unwrap_or_else(|_| "C.UTF-8".to_owned());
    let target = resolved_target(&ssh_target, &udp_host)?;
    let bootstrap = start_bootstrap(&BootstrapRequest {
        target: target.clone(),
        columns: 80,
        rows: 24,
        locale,
        ssh_path: None,
        timeout: Duration::from_secs(30),
    })?;

    let printable_key = bootstrap.session_key.as_str()?;
    let protocol_key = SessionKey::from_printable(printable_key)?;
    let remote = (udp_host.as_str(), bootstrap.udp_port)
        .to_socket_addrs()?
        .next()
        .ok_or("unable to resolve live UDP host")?;
    let mut transport = encrypted_transport(
        remote,
        CryptoSession::new(protocol_key),
        Direction::ToServer,
    )?;
    let clock = TimestampClock::new();
    let mut fragmenter = Fragmenter::default();
    let mut user_stream = UserStream::default();
    user_stream.push_resize(80, 24);
    let mut user_sender = StateSyncSender::new(UserStream::default());
    user_sender.set_state(user_stream);
    let instruction = user_sender.build_instruction(clock.timestamp())?;
    for fragment in fragmenter.make_fragments(&instruction, 1400)? {
        transport.send(clock.timestamp16(), 0, fragment.encode()?)?;
    }

    let mut assembler = FragmentAssembler::default();
    let mut receiver = StateSyncReceiver::new(CompleteTerminal::new(80, 24));
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut received_state = false;
    while Instant::now() < deadline {
        let Some((received, source)) = transport.receive(Duration::from_millis(250))? else {
            continue;
        };
        transport.note_roaming_source(source);
        let Some(instruction) = assembler.add_wire(&received.datagram.payload)? else {
            continue;
        };
        match receiver.apply_instruction(&instruction)? {
            ReceiveResult::Applied { .. } | ReceiveResult::Duplicate { .. } => {
                received_state = true;
                break;
            }
            ReceiveResult::Shutdown => break,
        }
    }

    assert!(
        received_state,
        "remote mosh-server did not return a decodable encrypted state"
    );
    println!(
        "live mosh-server version: {}",
        bootstrap.server_version.as_deref().unwrap_or("unknown")
    );
    Ok(())
}

fn resolved_target(ssh_target: &str, udp_host: &str) -> Result<ResolvedTarget, Box<dyn Error>> {
    let (user, host) = ssh_target
        .split_once('@')
        .ok_or("WINMOSH_LIVE_SSH_TARGET must be user@host")?;
    let port = PortSpec::Range {
        start: 60000,
        end: 61000,
    };
    Ok(ResolvedTarget {
        display_name: ssh_target.to_owned(),
        alias_name: None,
        ssh_target: ssh_target.to_owned(),
        effective_ssh: EffectiveSshConfig {
            hostname: host.to_owned(),
            user: user.to_owned(),
            port: 22,
            address_family: AddressFamily::Ipv4,
        },
        udp_candidates: vec![SocketCandidate {
            host: udp_host.to_owned(),
            port,
        }],
        mosh_server: "mosh-server".to_owned(),
        udp_port: port,
        terminal: "xterm-256color".to_owned(),
        prediction: PredictionMode::Off,
    })
}
