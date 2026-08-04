use std::time::Duration;

use winmosh_config::{load_config, resolve_target, ssh_config};
use winmosh_platform::windows::console::ConsoleGuard;
use winmosh_platform::windows::input::detect_input_capability;
use winmosh_platform::windows::output::detect_output_capability;

use crate::cli::DoctorCommand;
use crate::cli::GlobalOptions;
use crate::error::Result;

pub fn run(global: GlobalOptions, command: DoctorCommand) -> Result<()> {
    println!("WinMosh doctor");
    println!("os: {}", std::env::consts::OS);
    println!("arch: {}", std::env::consts::ARCH);
    println!("interactive input: {:?}", detect_input_capability());
    println!("output capability: {:?}", detect_output_capability());

    let config_path = global.config_path();
    println!("config path: {}", config_path.display());
    let document = load_config(&config_path)?;
    println!("config: readable");

    match ssh_config::probe_ssh(
        global.overrides.ssh_path.as_deref(),
        global
            .overrides
            .connect_timeout
            .unwrap_or(Duration::from_secs(10)),
    ) {
        Ok(probe) => {
            println!("ssh.exe: {}", probe.path.display());
            if let Some(version) = probe.version {
                println!("ssh version: {version}");
            }
        }
        Err(error) => println!("ssh.exe: unavailable ({error})"),
    }

    if command.console_guard {
        match ConsoleGuard::enter() {
            Ok(guard) => println!("console guard: active={}", guard.is_active()),
            Err(error) => println!("console guard: unavailable ({error})"),
        }
    }

    if let Some(target) = command.target {
        match resolve_target(&document.config, &target, &global.overrides) {
            Ok(resolved) => {
                println!("target: {target}");
                println!("effective host: {}", resolved.effective_ssh.hostname);
                println!("effective user: {}", resolved.effective_ssh.user);
                println!("effective ssh port: {}", resolved.effective_ssh.port);
                println!("udp port preference: {}", resolved.udp_port);
            }
            Err(error) => println!("target resolution: failed ({error})"),
        }
    }

    println!("protocol status: {}", winmosh_protocol::protocol_status());
    Ok(())
}
