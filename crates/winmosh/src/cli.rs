use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use winmosh_config::{
    add_alias, default_config_path, list_aliases, load_config, remove_alias, rename_alias,
    save_config, show_alias, AddressFamily, AliasAdd, ConfigDocument, PortSpec, PredictionMode,
    ResolveOverrides,
};

use crate::doctor;
use crate::error::{Error, Result};
use crate::session;
use crate::uninstall;
use crate::update;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run<I>(args: I) -> Result<()>
where
    I: IntoIterator<Item = OsString>,
{
    let args = args
        .into_iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let invocation = Invocation::parse(&args)?;
    dispatch(invocation)
}

fn dispatch(invocation: Invocation) -> Result<()> {
    match invocation.command {
        Command::Help => {
            print_help();
            Ok(())
        }
        Command::Version => {
            println!("winmosh {VERSION}");
            Ok(())
        }
        Command::Uninstall => uninstall::run(),
        Command::Alias(command) => run_alias(invocation.global, command),
        Command::Config(command) => run_config(invocation.global, command),
        Command::Doctor(options) => doctor::run(invocation.global, options),
        Command::Update(mode) => update::run(mode),
        Command::Connect(target) => session::run(invocation.global, target),
    }
}

fn run_alias(global: GlobalOptions, command: AliasCommand) -> Result<()> {
    let path = global.config_path();
    let mut document = load_config(&path)?;
    match command {
        AliasCommand::Add(request) => {
            let name = request.name.clone();
            add_alias(&mut document.config, request)?;
            save_document(&document)?;
            println!("alias added: {name}");
        }
        AliasCommand::List => {
            for (name, host) in list_aliases(&document.config) {
                println!("{name}\t{}", host.ssh_target);
            }
            if document.config.hosts.is_empty() {
                println!("no aliases configured");
            }
        }
        AliasCommand::Show(name) => {
            let host = show_alias(&document.config, &name)?;
            print_host(&name, host);
        }
        AliasCommand::Remove(name) => {
            remove_alias(&mut document.config, &name)?;
            save_document(&document)?;
            println!("alias removed: {name}");
        }
        AliasCommand::Rename { old, new } => {
            rename_alias(&mut document.config, &old, &new)?;
            save_document(&document)?;
            println!("alias renamed: {old} -> {new}");
        }
    }
    Ok(())
}

fn run_config(global: GlobalOptions, command: ConfigCommand) -> Result<()> {
    let path = global.config_path();
    match command {
        ConfigCommand::Path => {
            println!("{}", path.display());
        }
        ConfigCommand::Show => {
            let document = load_config(&path)?;
            print_config(&document);
        }
        ConfigCommand::Validate => {
            let _document = load_config(&path)?;
            println!("config valid: {}", path.display());
        }
        ConfigCommand::Edit => {
            if let Some(editor) = std::env::var_os("EDITOR") {
                std::process::Command::new(editor).arg(&path).status()?;
            } else {
                println!("{}", path.display());
            }
        }
    }
    Ok(())
}

fn save_document(document: &ConfigDocument) -> Result<()> {
    save_config(document).map_err(Error::from)
}

fn print_host(name: &str, host: &winmosh_config::HostConfig) {
    println!("alias: {name}");
    println!("ssh target: {}", host.ssh_target);
    if let Some(value) = &host.udp_host {
        println!("udp host: {value}");
    }
    if let Some(value) = host.udp_port {
        println!("udp port: {value}");
    }
    if let Some(value) = &host.mosh_server {
        println!("mosh server: {value}");
    }
    if let Some(value) = &host.terminal {
        println!("terminal: {value}");
    }
    if let Some(value) = host.prediction {
        println!("prediction: {value}");
    }
}

fn print_config(document: &ConfigDocument) {
    println!("path: {}", document.path.display());
    println!("version: {}", document.config.version);
    println!("defaults:");
    println!(
        "  mosh server: {}",
        document
            .config
            .defaults
            .mosh_server
            .as_deref()
            .unwrap_or("mosh-server")
    );
    println!(
        "  udp port: {}",
        document
            .config
            .defaults
            .udp_port
            .unwrap_or(winmosh_config::DEFAULT_UDP_PORT)
    );
    println!(
        "  terminal: {}",
        document
            .config
            .defaults
            .terminal
            .as_deref()
            .unwrap_or("xterm-256color")
    );
    println!(
        "  prediction: {}",
        document
            .config
            .defaults
            .prediction
            .unwrap_or(winmosh_config::DEFAULT_PREDICTION)
    );
    println!("hosts: {}", document.config.hosts.len());
    for (name, host) in list_aliases(&document.config) {
        println!("  {name}: {}", host.ssh_target);
    }
}

fn print_help() {
    println!(
        "winmosh {VERSION}\n\
Native Windows Mosh-compatible client.\n\n\
USAGE:\n\
    winmosh [OPTIONS] <TARGET>\n\
    winmosh alias <COMMAND>\n\
    winmosh config <COMMAND>\n\
    winmosh doctor [TARGET]\n\
    winmosh update [--check|--download]\n\
    winmosh version\n\
    winmosh --uninstall\n\n\
OPTIONS:\n\
    --config <PATH>              Override %APPDATA%\\WinMosh\\config.toml\n\
    --ssh <PATH>                 Override ssh.exe path\n\
    --server <REMOTE_PATH>       Override remote mosh-server path\n\
    --udp-host <HOST>            Override UDP target host\n\
    --udp-port <PORT|START:END>  Override UDP port preference\n\
    --family <auto|ipv4|ipv6>    Override OpenSSH address family\n\
    --terminal <TERM>            Override TERM value\n\
    --prediction <MODE>          off, adaptive, always, or never\n\
    --connect-timeout <SECONDS>  Timeout for local ssh.exe probes\n\
    --bootstrap-only             Start mosh-server and print bootstrap details\n\
    --no-color                   Disable colored diagnostics\n\
    -h, --help                   Show help\n\
    -V, --version                Show version\n\n\
ALIAS COMMANDS:\n\
    add <NAME> <SSH_TARGET> [--udp-host HOST] [--udp-port PORT|START:END] [--server PATH]\n\
    list\n\
    show <NAME>\n\
    remove <NAME>\n\
    rename <OLD> <NEW>\n\n\
CONFIG COMMANDS:\n\
    path\n\
    show\n\
    validate\n\
    edit\n\n\
DOCTOR OPTIONS:\n\
    --console-guard              Manually enter and restore console mode"
    );
}

#[derive(Debug, Clone)]
pub struct GlobalOptions {
    pub config: Option<PathBuf>,
    pub overrides: ResolveOverrides,
    pub bootstrap_only: bool,
    pub no_color: bool,
}

impl GlobalOptions {
    pub fn config_path(&self) -> PathBuf {
        self.config.clone().unwrap_or_else(default_config_path)
    }
}

#[derive(Debug, Clone)]
struct Invocation {
    global: GlobalOptions,
    command: Command,
}

#[derive(Debug, Clone)]
enum Command {
    Help,
    Version,
    Uninstall,
    Connect(String),
    Alias(AliasCommand),
    Config(ConfigCommand),
    Doctor(DoctorCommand),
    Update(UpdateMode),
}

#[derive(Debug, Clone, Default)]
pub struct DoctorCommand {
    pub target: Option<String>,
    pub console_guard: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UpdateMode {
    Check,
    Download,
}

#[derive(Debug, Clone)]
enum AliasCommand {
    Add(AliasAdd),
    List,
    Show(String),
    Remove(String),
    Rename { old: String, new: String },
}

#[derive(Debug, Clone)]
enum ConfigCommand {
    Path,
    Show,
    Validate,
    Edit,
}

impl Invocation {
    fn parse(args: &[String]) -> Result<Self> {
        let mut parser = Parser::new(args);
        let mut global = GlobalOptions {
            config: None,
            overrides: ResolveOverrides::default(),
            bootstrap_only: false,
            no_color: false,
        };

        while parse_global_option(&mut parser, &mut global)? {}

        let command = match parser.next().as_deref() {
            None => Command::Help,
            Some("-h" | "--help") => {
                return Ok(Self {
                    global,
                    command: Command::Help,
                })
            }
            Some("-V" | "--version") => {
                return Ok(Self {
                    global,
                    command: Command::Version,
                })
            }
            Some("version") => Command::Version,
            Some("alias") => Command::Alias(parse_alias_command(&mut parser)?),
            Some("config") => Command::Config(parse_config_command(&mut parser)?),
            Some("doctor") => Command::Doctor(parse_doctor_command(&mut parser)?),
            Some("update") => Command::Update(parse_update_command(&mut parser)?),
            Some("--uninstall") | Some("-uninstall") => Command::Uninstall,
            Some(target) if target.starts_with('-') => {
                return Err(Error::Cli(format!("unknown option: {target}")))
            }
            Some(target) => {
                let target = target.to_owned();
                while parse_global_option(&mut parser, &mut global)? {}
                Command::Connect(target)
            }
        };

        if parser.peek().is_some() {
            return Err(Error::Cli(format!(
                "unexpected argument: {}",
                parser.rest().join(" ")
            )));
        }

        Ok(Self { global, command })
    }
}

fn parse_global_option(parser: &mut Parser<'_>, global: &mut GlobalOptions) -> Result<bool> {
    let Some(argument) = parser.peek() else {
        return Ok(false);
    };

    match argument {
        "--config" => {
            parser.next();
            global.config = Some(PathBuf::from(parser.value("--config")?));
        }
        "--ssh" => {
            parser.next();
            global.overrides.ssh_path = Some(PathBuf::from(parser.value("--ssh")?));
        }
        "--server" => {
            parser.next();
            global.overrides.mosh_server = Some(parser.value("--server")?);
        }
        "--udp-host" => {
            parser.next();
            global.overrides.udp_host = Some(parser.value("--udp-host")?);
        }
        "--udp-port" => {
            parser.next();
            global.overrides.udp_port = Some(PortSpec::parse(&parser.value("--udp-port")?)?);
        }
        "--family" => {
            parser.next();
            global.overrides.family = Some(AddressFamily::parse(&parser.value("--family")?)?);
        }
        "--terminal" => {
            parser.next();
            global.overrides.terminal = Some(parser.value("--terminal")?);
        }
        "--prediction" => {
            parser.next();
            global.overrides.prediction =
                Some(PredictionMode::parse(&parser.value("--prediction")?)?);
        }
        "--connect-timeout" => {
            parser.next();
            let seconds = parser
                .value("--connect-timeout")?
                .parse::<u64>()
                .map_err(|_| {
                    Error::Cli("--connect-timeout must be an integer number of seconds".to_owned())
                })?;
            global.overrides.connect_timeout = Some(Duration::from_secs(seconds));
        }
        "--bootstrap-only" => {
            parser.next();
            global.bootstrap_only = true;
        }
        "--no-color" => {
            parser.next();
            global.no_color = true;
        }
        "-h" | "--help" | "-V" | "--version" => return Ok(false),
        _ => return Ok(false),
    }

    Ok(true)
}

fn parse_alias_command(parser: &mut Parser<'_>) -> Result<AliasCommand> {
    match parser.next().as_deref() {
        Some("add") => {
            let name = parser.required("alias name")?;
            let ssh_target = parser.required("ssh target")?;
            let mut request = AliasAdd {
                name,
                ssh_target,
                udp_host: None,
                udp_port: None,
                mosh_server: None,
                terminal: None,
                prediction: None,
            };
            while let Some(argument) = parser.peek() {
                match argument {
                    "--udp-host" => {
                        parser.next();
                        request.udp_host = Some(parser.value("--udp-host")?);
                    }
                    "--udp-port" => {
                        parser.next();
                        request.udp_port = Some(PortSpec::parse(&parser.value("--udp-port")?)?);
                    }
                    "--server" => {
                        parser.next();
                        request.mosh_server = Some(parser.value("--server")?);
                    }
                    "--terminal" => {
                        parser.next();
                        request.terminal = Some(parser.value("--terminal")?);
                    }
                    "--prediction" => {
                        parser.next();
                        request.prediction =
                            Some(PredictionMode::parse(&parser.value("--prediction")?)?);
                    }
                    _ => return Err(Error::Cli(format!("unknown alias add option: {argument}"))),
                }
            }
            Ok(AliasCommand::Add(request))
        }
        Some("list") => Ok(AliasCommand::List),
        Some("show") => Ok(AliasCommand::Show(parser.required("alias name")?)),
        Some("remove") => Ok(AliasCommand::Remove(parser.required("alias name")?)),
        Some("rename") => Ok(AliasCommand::Rename {
            old: parser.required("old alias name")?,
            new: parser.required("new alias name")?,
        }),
        Some(other) => Err(Error::Cli(format!("unknown alias command: {other}"))),
        None => Err(Error::Cli("missing alias command".to_owned())),
    }
}

fn parse_config_command(parser: &mut Parser<'_>) -> Result<ConfigCommand> {
    match parser.next().as_deref() {
        Some("path") => Ok(ConfigCommand::Path),
        Some("show") => Ok(ConfigCommand::Show),
        Some("validate") => Ok(ConfigCommand::Validate),
        Some("edit") => Ok(ConfigCommand::Edit),
        Some(other) => Err(Error::Cli(format!("unknown config command: {other}"))),
        None => Err(Error::Cli("missing config command".to_owned())),
    }
}

fn parse_doctor_command(parser: &mut Parser<'_>) -> Result<DoctorCommand> {
    let mut command = DoctorCommand::default();
    while let Some(argument) = parser.peek() {
        match argument {
            "--console-guard" => {
                parser.next();
                command.console_guard = true;
            }
            value if value.starts_with('-') => {
                return Err(Error::Cli(format!("unknown doctor option: {value}")))
            }
            _ if command.target.is_none() => command.target = parser.next(),
            value => return Err(Error::Cli(format!("unexpected doctor argument: {value}"))),
        }
    }
    Ok(command)
}

fn parse_update_command(parser: &mut Parser<'_>) -> Result<UpdateMode> {
    let mut mode = UpdateMode::Check;
    while let Some(argument) = parser.next() {
        match argument.as_str() {
            "--check" => mode = UpdateMode::Check,
            "--download" => mode = UpdateMode::Download,
            value => return Err(Error::Cli(format!("unknown update option: {value}"))),
        }
    }
    Ok(mode)
}

#[derive(Debug)]
struct Parser<'a> {
    args: &'a [String],
    index: usize,
}

impl<'a> Parser<'a> {
    fn new(args: &'a [String]) -> Self {
        Self { args, index: 1 }
    }

    fn peek(&self) -> Option<&str> {
        self.args.get(self.index).map(String::as_str)
    }

    fn next(&mut self) -> Option<String> {
        let value = self.args.get(self.index).cloned();
        if value.is_some() {
            self.index += 1;
        }
        value
    }

    fn value(&mut self, option: &str) -> Result<String> {
        self.next()
            .filter(|value| !value.starts_with('-'))
            .ok_or_else(|| Error::Cli(format!("{option} requires a value")))
    }

    fn required(&mut self, name: &str) -> Result<String> {
        self.next()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| Error::Cli(format!("missing {name}")))
    }

    fn rest(&self) -> Vec<String> {
        self.args[self.index..].to_vec()
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{Command, Invocation};

    fn parse(input: &[&str]) -> Result<Invocation, Box<dyn std::error::Error>> {
        let args = input.iter().map(OsString::from).collect::<Vec<_>>();
        Ok(Invocation::parse(
            &args
                .iter()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
        )?)
    }

    #[test]
    fn parses_target_with_global_options() -> Result<(), Box<dyn std::error::Error>> {
        let invocation = parse(&[
            "winmosh",
            "--config",
            "test.toml",
            "--udp-port",
            "60020:60030",
            "myserver",
        ])?;
        assert!(matches!(invocation.command, Command::Connect(_)));
        assert_eq!(
            invocation
                .global
                .config
                .as_ref()
                .and_then(|path| path.to_str()),
            Some("test.toml")
        );
        Ok(())
    }

    #[test]
    fn parses_target_with_trailing_overrides() -> Result<(), Box<dyn std::error::Error>> {
        let invocation = parse(&["winmosh", "myserver", "--udp-port", "60020"])?;
        assert!(matches!(invocation.command, Command::Connect(_)));
        assert_eq!(
            invocation.global.overrides.udp_port,
            Some(winmosh_config::PortSpec::Single(60020))
        );
        Ok(())
    }

    #[test]
    fn parses_console_guard_doctor_option() -> Result<(), Box<dyn std::error::Error>> {
        let invocation = parse(&["winmosh", "doctor", "--console-guard"])?;
        match invocation.command {
            Command::Doctor(command) => assert!(command.console_guard),
            _ => return Err("expected doctor command".into()),
        }
        Ok(())
    }

    #[test]
    fn parses_update_command_modes() -> Result<(), Box<dyn std::error::Error>> {
        let check = parse(&["winmosh", "update", "--check"])?;
        assert!(matches!(check.command, Command::Update(_)));
        let download = parse(&["winmosh", "update", "--download"])?;
        assert!(matches!(download.command, Command::Update(_)));
        Ok(())
    }
}
