use clap::Parser;

pub fn evaluate_cli() -> Cli {
    Cli::parse()
}

fn parse_canonical_path(path: &str) -> Result<String, std::io::Error> {
    Ok(std::fs::canonicalize(path)?.to_string_lossy().to_string())
}

fn parse_ensured_directory(path: &str) -> Result<String, std::io::Error> {
    std::fs::create_dir_all(path)?;
    Ok(std::fs::canonicalize(path)?.to_string_lossy().to_string())
}

macro_rules! possible_values {
    ($t: ty, $options: ident) => {{
        use clap::builder::{PossibleValuesParser, TypedValueParser};
        PossibleValuesParser::new(<$t>::$options).map(|s| s.parse::<$t>().unwrap())
    }};
}

#[derive(Debug)]
pub enum CliError {
    InvalidEnvVar { specification: String },
}

impl std::error::Error for CliError {}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidEnvVar { specification } => {
                write!(f, "Invalid KEY=value formatting in '{}'", specification)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Priority {
    Realtime,
    High,
    AboveNormal,
    #[default]
    Normal,
    BelowNormal,
    Idle,
}

impl Priority {
    pub const ALL: &'static [&'static str] = &["realtime", "high", "above-normal", "normal", "below-normal", "idle"];
}

impl Priority {
    pub fn to_cli(self) -> String {
        match self {
            Self::Realtime => "realtime",
            Self::High => "high",
            Self::AboveNormal => "above-normal",
            Self::Normal => "normal",
            Self::BelowNormal => "below-normal",
            Self::Idle => "idle",
        }
        .to_string()
    }

    pub fn to_windows(self) -> windows::Win32::System::Threading::PROCESS_CREATION_FLAGS {
        match self {
            Self::Realtime => windows::Win32::System::Threading::REALTIME_PRIORITY_CLASS,
            Self::High => windows::Win32::System::Threading::HIGH_PRIORITY_CLASS,
            Self::AboveNormal => windows::Win32::System::Threading::ABOVE_NORMAL_PRIORITY_CLASS,
            Self::Normal => windows::Win32::System::Threading::NORMAL_PRIORITY_CLASS,
            Self::BelowNormal => windows::Win32::System::Threading::BELOW_NORMAL_PRIORITY_CLASS,
            Self::Idle => windows::Win32::System::Threading::IDLE_PRIORITY_CLASS,
        }
    }
}

impl std::str::FromStr for Priority {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "realtime" => Ok(Self::Realtime),
            "high" => Ok(Self::High),
            "above-normal" => Ok(Self::AboveNormal),
            "normal" => Ok(Self::Normal),
            "below-normal" => Ok(Self::BelowNormal),
            "idle" => Ok(Self::Idle),
            _ => Err(format!("invalid priority: {}", s)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogRotation {
    Bytes(u64),
    Daily,
    Hourly,
}

impl LogRotation {
    pub fn to_cli(self) -> String {
        match self {
            LogRotation::Bytes(bytes) => format!("bytes={}", bytes),
            LogRotation::Daily => "daily".to_string(),
            LogRotation::Hourly => "hourly".to_string(),
        }
    }
}

impl std::str::FromStr for LogRotation {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "daily" => return Ok(Self::Daily),
            "hourly" => return Ok(Self::Hourly),
            _ => {}
        }

        if s.starts_with("bytes=") {
            let parts: Vec<_> = s.splitn(2, '=').collect();
            match parts[1].parse::<u64>() {
                Ok(bytes) => return Ok(Self::Bytes(bytes)),
                Err(e) => return Err(format!("Unable to parse log rotation as bytes: {:?}", e)),
            }
        }

        Err(format!("Unable to parse log rotation: {}", s))
    }
}

impl Default for LogRotation {
    fn default() -> Self {
        Self::Bytes(1024 * 1024 * 2)
    }
}

fn parse_env_var(value: &str) -> Result<(String, String), CliError> {
    let parts: Vec<&str> = value.splitn(2, '=').collect();
    if parts.len() != 2 {
        return Err(CliError::InvalidEnvVar {
            specification: value.to_string(),
        });
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

fn styles() -> clap::builder::styling::Styles {
    use clap::builder::styling::{AnsiColor, Effects, Styles};

    Styles::styled()
        .header(AnsiColor::Yellow.on_default() | Effects::BOLD)
        .usage(AnsiColor::Yellow.on_default() | Effects::BOLD)
        .literal(AnsiColor::Green.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::Green.on_default())
}

#[derive(clap::Parser, Clone, Debug, Default, PartialEq, Eq)]
pub struct CommonOpts {
    /// Exit codes that should be considered successful (comma-separated) [default: 0]
    #[clap(
        long,
        value_name = "codes",
        value_delimiter = ',',
        number_of_values = 1,
        allow_hyphen_values(true)
    )]
    pub pass: Option<Vec<i32>>,

    /// Always restart the command regardless of the exit code
    #[clap(
        long,
        conflicts_with("no_restart"),
        conflicts_with("restart_if"),
        conflicts_with("restart_if_not")
    )]
    pub restart: bool,

    /// Never restart the command regardless of the exit code
    #[clap(
        long,
        conflicts_with("restart"),
        conflicts_with("restart_if"),
        conflicts_with("restart_if_not")
    )]
    pub no_restart: bool,

    /// Restart the command if the exit code is one of these (comma-separated)
    #[clap(
        long,
        conflicts_with("restart"),
        conflicts_with("no_restart"),
        conflicts_with("restart_if_not"),
        value_name = "codes",
        value_delimiter = ',',
        number_of_values = 1,
        allow_hyphen_values(true)
    )]
    pub restart_if: Vec<i32>,

    /// Restart the command if the exit code is not one of these (comma-separated)
    #[clap(
        long,
        conflicts_with("restart"),
        conflicts_with("no_restart"),
        conflicts_with("restart_if"),
        value_name = "codes",
        value_delimiter = ',',
        number_of_values = 1,
        allow_hyphen_values(true)
    )]
    pub restart_if_not: Vec<i32>,

    /// How long to wait in milliseconds between sending the wrapped process
    /// a ctrl-C event and forcibly killing it [default: 3000]
    #[clap(long, value_name = "ms")]
    pub stop_timeout: Option<u64>,

    /// Disable all of Shawl's logging
    #[clap(long)]
    pub no_log: bool,

    /// Disable logging of output from the command running as a service
    #[clap(long)]
    pub no_log_cmd: bool,

    /// Write log file to a custom directory. This directory will be created if it doesn't exist.
    #[clap(long, value_name = "path", value_parser = parse_ensured_directory)]
    pub log_dir: Option<String>,

    /// Use a different name for the main log file.
    /// Set this to just the desired base name of the log file.
    /// For example, `--log-as shawl` would result in a log file named `shawl_rCURRENT.log`
    /// instead of the normal `shawl_for_<name>_rCURRENT.log` pattern.
    #[clap(long)]
    pub log_as: Option<String>,

    /// Use a separate log file for the wrapped command's stdout and stderr.
    /// Set this to just the desired base name of the log file.
    /// For example, `--log-cmd-as foo` would result in a log file named `foo_rCURRENT.log`.
    /// The output will be logged as-is without any additional log template.
    #[clap(long)]
    pub log_cmd_as: Option<String>,

    /// Threshold for rotating log files. Valid options:
    /// `daily`, `hourly`, `bytes=n` (every N bytes)
    /// [default: bytes=2097152]
    #[clap(long)]
    pub log_rotate: Option<LogRotation>,

    /// How many old log files to retain [default: 2]
    #[clap(long)]
    pub log_retain: Option<usize>,

    /// Append the service start arguments to the command
    #[clap(long)]
    pub pass_start_args: bool,

    /// Additional environment variable in the format 'KEY=value' (repeatable)
    #[clap(long, number_of_values = 1, value_parser = parse_env_var)]
    pub env: Vec<(String, String)>,

    /// Additional directory to append to the PATH environment variable (repeatable)
    #[clap(long, number_of_values = 1, value_parser = parse_canonical_path)]
    pub path: Vec<String>,

    /// Additional directory to prepend to the PATH environment variable (repeatable)
    #[clap(long, number_of_values = 1, value_parser = parse_canonical_path)]
    pub path_prepend: Vec<String>,

    /// Process priority of the command to run as a service
    #[clap(long, value_parser = possible_values!(Priority, ALL))]
    pub priority: Option<Priority>,

    /// Command to run as a service
    #[clap(required(true), last(true))]
    pub command: Vec<String>,
}

#[derive(clap::Subcommand, Clone, Debug, PartialEq, Eq)]
pub enum Subcommand {
    #[clap(about = "Add a new service")]
    Add {
        #[clap(flatten)]
        common: CommonOpts,

        /// Working directory in which to run the command. You may provide a
        /// relative path, and it will be converted to an absolute one
        #[clap(long, value_name = "path", value_parser = parse_canonical_path)]
        cwd: Option<String>,

        /// Other services that must be started first (comma-separated)
        #[clap(long, value_delimiter = ',')]
        dependencies: Vec<String>,

        /// Name of the service to create
        #[clap(long)]
        name: String,
    },
    #[clap(about = "Run a command as a service; only works when launched by the Windows service manager")]
    Run {
        #[clap(flatten)]
        common: CommonOpts,

        /// Working directory in which to run the command. Must be an absolute path
        #[clap(long, value_name = "path")]
        cwd: Option<String>,

        /// Name of the service; used in logging, but does not need to match real name
        #[clap(long, default_value = "Shawl")]
        name: String,
    },
}

#[derive(clap::Parser, Clone, Debug, PartialEq, Eq)]
#[clap(
    name = "shawl",
    version,
    about = "Wrap arbitrary commands as Windows services",
    max_term_width = 100,
    subcommand_negates_reqs = true,
    next_line_help = true,
    styles = styles()
)]
pub struct Cli {
    #[clap(subcommand)]
    pub sub: Subcommand,
}

#[cfg(test)]
speculate::speculate! {
    fn check_args(args: &[&str], expected: Cli) {
        assert_eq!(
            expected,
            Cli::parse_from(args)
        );
    }

    fn check_args_err(args: &[&str], error: clap::error::ErrorKind) {
        let result = Cli::try_parse_from(args);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), error);
    }

    fn s(text: &str) -> String {
        text.to_string()
    }

    fn p(path: &str) -> String {
        std::fs::canonicalize(&path).unwrap().to_string_lossy().to_string()
    }

    describe "run subcommand" {
        it "works with minimal arguments" {
            check_args(
                &["shawl", "run", "--", "foo"],
                Cli {
                    sub: Subcommand::Run {
                        name: s("Shawl"),
                        cwd: None,
                        common: CommonOpts {
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "requires a command" {
            check_args_err(
                &["shawl", "run"],
                clap::error::ErrorKind::MissingRequiredArgument,
            );
        }

        it "accepts --pass" {
            check_args(
                &["shawl", "run", "--pass", "1,2", "--", "foo"],
                Cli {
                    sub: Subcommand::Run {
                        name: s("Shawl"),
                        cwd: None,
                        common: CommonOpts {
                            pass: Some(vec![1, 2]),
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "accepts --pass with leading negative" {
            check_args(
                &["shawl", "run", "--pass", "-1", "--", "foo"],
                Cli {
                    sub: Subcommand::Run {
                        name: s("Shawl"),
                        cwd: None,
                        common: CommonOpts {
                            pass: Some(vec![-1]),
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "rejects --pass without value" {
            check_args_err(
                &["shawl", "run", "--pass", "--", "foo"],
                clap::error::ErrorKind::UnknownArgument,
            );
        }

        it "accepts --restart" {
            check_args(
                &["shawl", "run", "--restart", "--", "foo"],
                Cli {
                    sub: Subcommand::Run {
                        name: s("Shawl"),
                        cwd: None,
                        common: CommonOpts {
                            restart: true,
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "rejects --restart with conflicting options" {
            for case in [
                vec!["shawl", "run", "--restart", "--no-restart", "--", "foo"],
                vec!["shawl", "run", "--restart", "--restart-if", "1", "--", "foo"],
                vec!["shawl", "run", "--restart", "--restart-if-not", "1", "--", "foo"],
            ] {
                check_args_err(
                    &case,
                    clap::error::ErrorKind::ArgumentConflict,
                );
            }
        }

        it "accepts --no-restart" {
            check_args(
                &["shawl", "run", "--no-restart", "--", "foo"],
                Cli {
                    sub: Subcommand::Run {
                        name: s("Shawl"),
                        cwd: None,
                        common: CommonOpts {
                            no_restart: true,
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "rejects --no-restart with conflicting options" {
            for case in [
                vec!["shawl", "run", "--no-restart", "--restart", "--", "foo"],
                vec!["shawl", "run", "--no-restart", "--restart-if", "1", "--", "foo"],
                vec!["shawl", "run", "--no-restart", "--restart-if-not", "1", "--", "foo"],
            ] {
                check_args_err(
                    &case,
                    clap::error::ErrorKind::ArgumentConflict,
                );
            }
        }

        it "accepts --restart-if" {
            check_args(
                &["shawl", "run", "--restart-if", "1,2", "--", "foo"],
                Cli {
                    sub: Subcommand::Run {
                        name: s("Shawl"),
                        cwd: None,
                        common: CommonOpts {
                            restart_if: vec![1, 2],
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "accepts --restart-if with leading negative" {
            check_args(
                &["shawl", "run", "--restart-if", "-1", "--", "foo"],
                Cli {
                    sub: Subcommand::Run {
                        name: s("Shawl"),
                        cwd: None,
                        common: CommonOpts {
                            restart_if: vec![-1],
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "rejects --restart-if without value" {
            check_args_err(
                &["shawl", "run", "--restart-if", "--", "foo"],
                clap::error::ErrorKind::UnknownArgument,
            );
        }

        it "rejects --restart-if with conflicting options" {
            for case in [
                vec!["shawl", "run", "--restart-if", "0", "--restart", "--", "foo"],
                vec!["shawl", "run", "--restart-if", "0", "--no-restart", "--", "foo"],
                vec!["shawl", "run", "--restart-if", "0", "--restart-if-not", "1", "--", "foo"],
            ] {
                check_args_err(
                    &case,
                    clap::error::ErrorKind::ArgumentConflict,
                );
            }
        }

        it "accepts --restart-if-not" {
            check_args(
                &["shawl", "run", "--restart-if-not", "1,2", "--", "foo"],
                Cli {
                    sub: Subcommand::Run {
                        name: s("Shawl"),
                        cwd: None,
                        common: CommonOpts {
                            restart_if_not: vec![1, 2],
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "accepts --restart-if-not with leading negative" {
            check_args(
                &["shawl", "run", "--restart-if-not", "-1", "--", "foo"],
                Cli {
                    sub: Subcommand::Run {
                        name: s("Shawl"),
                        cwd: None,
                        common: CommonOpts {
                            restart_if_not: vec![-1],
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "rejects --restart-if-not without value" {
            check_args_err(
                &["shawl", "run", "--restart-if-not", "--", "foo"],
                clap::error::ErrorKind::UnknownArgument,
            );
        }

        it "rejects --restart-if-not with conflicting options" {
            for case in [
                vec!["shawl", "run", "--restart-if-not", "0", "--restart", "--", "foo"],
                vec!["shawl", "run", "--restart-if-not", "0", "--no-restart", "--", "foo"],
                vec!["shawl", "run", "--restart-if-not", "0", "--restart-if", "1", "--", "foo"],
            ] {
                check_args_err(
                    &case,
                    clap::error::ErrorKind::ArgumentConflict,
                );
            }
        }

        it "accepts --stop-timeout" {
            check_args(
                &["shawl", "run", "--stop-timeout", "500", "--", "foo"],
                Cli {
                    sub: Subcommand::Run {
                        name: s("Shawl"),
                        cwd: None,
                        common: CommonOpts {
                            stop_timeout: Some(500),
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "accepts --name" {
            check_args(
                &["shawl", "run", "--name", "custom-name", "--", "foo"],
                Cli {
                    sub: Subcommand::Run {
                        name: s("custom-name"),
                        cwd: None,
                        common: CommonOpts {
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }
    }

    describe "add subcommand" {
        it "works with minimal arguments" {
            check_args(
                &["shawl", "add", "--name", "custom-name", "--", "foo"],
                Cli {
                    sub: Subcommand::Add {
                        name: s("custom-name"),
                        cwd: None,
                        dependencies: vec![],
                        common: CommonOpts {
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "requires a command" {
            check_args_err(
                &["shawl", "add", "--name", "foo"],
                clap::error::ErrorKind::MissingRequiredArgument,
            );
        }

        it "requires a name" {
            check_args_err(
                &["shawl", "add", "--", "foo"],
                clap::error::ErrorKind::MissingRequiredArgument,
            );
        }

        it "accepts --pass" {
            check_args(
                &["shawl", "add", "--pass", "1,2", "--name", "foo", "--", "foo"],
                Cli {
                    sub: Subcommand::Add {
                        name: s("foo"),
                        cwd: None,
                        dependencies: vec![],
                        common: CommonOpts {
                            pass: Some(vec![1, 2]),
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "accepts --restart" {
            check_args(
                &["shawl", "add", "--restart", "--name", "foo", "--", "foo"],
                Cli {
                    sub: Subcommand::Add {
                        name: s("foo"),
                        cwd: None,
                        dependencies: vec![],
                        common: CommonOpts {
                            restart: true,
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "accepts --no-restart" {
            check_args(
                &["shawl", "add", "--no-restart", "--name", "foo", "--", "foo"],
                Cli {
                    sub: Subcommand::Add {
                        name: s("foo"),
                        cwd: None,
                        dependencies: vec![],
                        common: CommonOpts {
                            no_restart: true,
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "accepts --restart-if" {
            check_args(
                &["shawl", "add", "--restart-if", "1,2", "--name", "foo", "--", "foo"],
                Cli {
                    sub: Subcommand::Add {
                        name: s("foo"),
                        cwd: None,
                        dependencies: vec![],
                        common: CommonOpts {
                            restart_if: vec![1, 2],
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "accepts --restart-if-not" {
            check_args(
                &["shawl", "add", "--restart-if-not", "1,2", "--name", "foo", "--", "foo"],
                Cli {
                    sub: Subcommand::Add {
                        name: s("foo"),
                        cwd: None,
                        dependencies: vec![],
                        common: CommonOpts {
                            restart_if_not: vec![1, 2],
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "accepts --stop-timeout" {
            check_args(
                &["shawl", "add", "--stop-timeout", "500", "--name", "foo", "--", "foo"],
                Cli {
                    sub: Subcommand::Add {
                        name: s("foo"),
                        cwd: None,
                        dependencies: vec![],
                        common: CommonOpts {
                            stop_timeout: Some(500),
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "accepts --no-log" {
            check_args(
                &["shawl", "run", "--no-log", "--", "foo"],
                Cli {
                    sub: Subcommand::Run {
                        name: s("Shawl"),
                        cwd: None,
                        common: CommonOpts {
                            no_log: true,
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "accepts --no-log-cmd" {
            check_args(
                &["shawl", "run", "--no-log-cmd", "--", "foo"],
                Cli {
                    sub: Subcommand::Run {
                        name: s("Shawl"),
                        cwd: None,
                        common: CommonOpts {
                            no_log_cmd: true,
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "accepts --log-as" {
            check_args(
                &["shawl", "run", "--log-as", "foo", "--", "foo"],
                Cli {
                    sub: Subcommand::Run {
                        name: s("Shawl"),
                        cwd: None,
                        common: CommonOpts {
                            log_as: Some("foo".to_string()),
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "accepts --log-cmd-as" {
            check_args(
                &["shawl", "run", "--log-cmd-as", "foo", "--", "foo"],
                Cli {
                    sub: Subcommand::Run {
                        name: s("Shawl"),
                        cwd: None,
                        common: CommonOpts {
                            log_cmd_as: Some("foo".to_string()),
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "accepts --log-rotate bytes=n" {
            check_args(
                &["shawl", "run", "--log-rotate", "bytes=123", "--", "foo"],
                Cli {
                    sub: Subcommand::Run {
                        name: s("Shawl"),
                        cwd: None,
                        common: CommonOpts {
                            log_rotate: Some(LogRotation::Bytes(123)),
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "accepts --log-rotate daily" {
            check_args(
                &["shawl", "run", "--log-rotate", "daily", "--", "foo"],
                Cli {
                    sub: Subcommand::Run {
                        name: s("Shawl"),
                        cwd: None,
                        common: CommonOpts {
                            log_rotate: Some(LogRotation::Daily),
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "accepts --log-rotate hourly" {
            check_args(
                &["shawl", "run", "--log-rotate", "hourly", "--", "foo"],
                Cli {
                    sub: Subcommand::Run {
                        name: s("Shawl"),
                        cwd: None,
                        common: CommonOpts {
                            log_rotate: Some(LogRotation::Hourly),
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "accepts --log-retain" {
            check_args(
                &["shawl", "run", "--log-retain", "5", "--", "foo"],
                Cli {
                    sub: Subcommand::Run {
                        name: s("Shawl"),
                        cwd: None,
                        common: CommonOpts {
                            log_retain: Some(5),
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "accepts --log-dir" {
            let path = env!("CARGO_MANIFEST_DIR");
            check_args(
                &["shawl", "run", "--log-dir", path, "--", "foo"],
                Cli {
                    sub: Subcommand::Run {
                        name: s("Shawl"),
                        cwd: None,
                        common: CommonOpts {
                            log_dir: Some(p(path)),
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "accepts --pass-start-args" {
            check_args(
                &["shawl", "run", "--pass-start-args", "--", "foo"],
                Cli {
                    sub: Subcommand::Run {
                        name: s("Shawl"),
                        cwd: None,
                        common: CommonOpts {
                            pass_start_args: true,
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "accepts --env" {
            check_args(
                &["shawl", "add", "--env", "FOO=bar", "--name", "foo", "--", "foo"],
                Cli {
                    sub: Subcommand::Add {
                        name: s("foo"),
                        cwd: None,
                        dependencies: vec![],
                        common: CommonOpts {
                            env: vec![(s("FOO"), s("bar"))],
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "accepts --env multiple times" {
            check_args(
                &["shawl", "add", "--env", "FOO=1", "--env", "BAR=2", "--name", "foo", "--", "foo"],
                Cli {
                    sub: Subcommand::Add {
                        name: s("foo"),
                        cwd: None,
                        dependencies: vec![],
                        common: CommonOpts {
                            env: vec![(s("FOO"), s("1")), (s("BAR"), s("2"))],
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "accepts --path" {
            let path = env!("CARGO_MANIFEST_DIR");
            check_args(
                &["shawl", "add", "--path", path, "--name", "foo", "--", "foo"],
                Cli {
                    sub: Subcommand::Add {
                        name: s("foo"),
                        cwd: None,
                        dependencies: vec![],
                        common: CommonOpts {
                            path: vec![p(path)],
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "accepts --path multiple times" {
            let path1 = format!("{}/target", env!("CARGO_MANIFEST_DIR"));
            let path2 = format!("{}/src", env!("CARGO_MANIFEST_DIR"));
            check_args(
                &["shawl", "add", "--path", &path1, "--path", &path2, "--name", "foo", "--", "foo"],
                Cli {
                    sub: Subcommand::Add {
                        name: s("foo"),
                        cwd: None,
                        dependencies: vec![],
                        common: CommonOpts {
                            path: vec![p(&path1), p(&path2)],
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "accepts --path-prepend" {
            let path = env!("CARGO_MANIFEST_DIR");
            check_args(
                &["shawl", "add", "--path-prepend", path, "--name", "foo", "--", "foo"],
                Cli {
                    sub: Subcommand::Add {
                        name: s("foo"),
                        cwd: None,
                        dependencies: vec![],
                        common: CommonOpts {
                            path_prepend: vec![p(path)],
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }

        it "accepts --path-prepend multiple times" {
            let path1 = format!("{}/target", env!("CARGO_MANIFEST_DIR"));
            let path2 = format!("{}/src", env!("CARGO_MANIFEST_DIR"));
            check_args(
                &["shawl", "add", "--path-prepend", &path1, "--path-prepend", &path2, "--name", "foo", "--", "foo"],
                Cli {
                    sub: Subcommand::Add {
                        name: s("foo"),
                        cwd: None,
                        dependencies: vec![],
                        common: CommonOpts {
                            path_prepend: vec![p(&path1), p(&path2)],
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }
    }
}
