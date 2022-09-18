use clap::Parser;

pub fn evaluate_cli() -> Cli {
    Cli::from_args()
}

fn parse_canonical_path(path: &str) -> Result<String, std::io::Error> {
    Ok(std::fs::canonicalize(path)?.to_string_lossy().to_string())
}

fn parse_ensured_directory(path: &str) -> Result<String, std::io::Error> {
    std::fs::create_dir_all(&path)?;
    Ok(std::fs::canonicalize(path)?.to_string_lossy().to_string())
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

fn parse_env_var(value: &str) -> Result<(String, String), CliError> {
    let parts: Vec<&str> = value.splitn(2, '=').collect();
    if parts.len() != 2 {
        return Err(CliError::InvalidEnvVar {
            specification: value.to_string(),
        });
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

#[derive(clap::Parser, Clone, Debug, Default, PartialEq, Eq)]
pub struct CommonOpts {
    /// Exit codes that should be considered successful (comma-separated) [default: 0]
    #[clap(
        long,
        value_name = "codes",
        use_delimiter(true),
        number_of_values = 1,
        allow_hyphen_values(true)
    )]
    pub pass: Option<Vec<i32>>,

    /// Always restart the command regardless of the exit code
    #[clap(
        long,
        conflicts_with("no-restart"),
        conflicts_with("restart-if"),
        conflicts_with("restart-if-not")
    )]
    pub restart: bool,

    /// Never restart the command regardless of the exit code
    #[clap(
        long,
        conflicts_with("restart"),
        conflicts_with("restart-if"),
        conflicts_with("restart-if-not")
    )]
    pub no_restart: bool,

    /// Restart the command if the exit code is one of these (comma-separated)
    #[clap(
        long,
        conflicts_with("restart"),
        conflicts_with("no-restart"),
        conflicts_with("restart-if-not"),
        value_name = "codes",
        use_delimiter(true),
        number_of_values = 1,
        allow_hyphen_values(true)
    )]
    pub restart_if: Vec<i32>,

    /// Restart the command if the exit code is not one of these (comma-separated)
    #[clap(
        long,
        conflicts_with("restart"),
        conflicts_with("no-restart"),
        conflicts_with("restart-if"),
        value_name = "codes",
        use_delimiter(true),
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
    #[clap(long, value_name = "path", parse(try_from_str = parse_ensured_directory))]
    pub log_dir: Option<String>,

    /// Append the service start arguments to the command
    #[clap(long)]
    pub pass_start_args: bool,

    /// Additional environment variable in the format 'KEY=value' (repeatable)
    #[clap(long, number_of_values = 1, parse(try_from_str = parse_env_var))]
    pub env: Vec<(String, String)>,

    /// Additional directory to add to the PATH environment variable (repeatable)
    #[clap(long, number_of_values = 1, parse(try_from_str = parse_canonical_path))]
    pub path: Vec<String>,

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
        #[clap(long, value_name = "path", parse(try_from_str = parse_canonical_path))]
        cwd: Option<String>,

        /// Name of the service to create
        #[clap(long)]
        name: String,
    },
    #[clap(
        about = "Run a command as a service; only works when launched by the Windows service manager"
    )]
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
    about = "Wrap arbitrary commands as Windows services",
    set_term_width = 80,
    setting(clap::AppSettings::SubcommandsNegateReqs)
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
            Cli::from_clap(&Cli::clap().get_matches_from(args))
        );
    }

    fn check_args_err(args: &[&str], error: clap::ErrorKind) {
        let result = Cli::clap().get_matches_from_safe(args);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind, error);
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
                clap::ErrorKind::MissingRequiredArgument,
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
                clap::ErrorKind::UnknownArgument,
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
                    clap::ErrorKind::ArgumentConflict,
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
                    clap::ErrorKind::ArgumentConflict,
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
                clap::ErrorKind::UnknownArgument,
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
                    clap::ErrorKind::ArgumentConflict,
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
                clap::ErrorKind::UnknownArgument,
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
                    clap::ErrorKind::ArgumentConflict,
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
                clap::ErrorKind::MissingRequiredArgument,
            );
        }

        it "requires a name" {
            check_args_err(
                &["shawl", "add", "--", "foo"],
                clap::ErrorKind::MissingRequiredArgument,
            );
        }

        it "accepts --pass" {
            check_args(
                &["shawl", "add", "--pass", "1,2", "--name", "foo", "--", "foo"],
                Cli {
                    sub: Subcommand::Add {
                        name: s("foo"),
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

        it "accepts --restart" {
            check_args(
                &["shawl", "add", "--restart", "--name", "foo", "--", "foo"],
                Cli {
                    sub: Subcommand::Add {
                        name: s("foo"),
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

        it "accepts --no-restart" {
            check_args(
                &["shawl", "add", "--no-restart", "--name", "foo", "--", "foo"],
                Cli {
                    sub: Subcommand::Add {
                        name: s("foo"),
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

        it "accepts --restart-if" {
            check_args(
                &["shawl", "add", "--restart-if", "1,2", "--name", "foo", "--", "foo"],
                Cli {
                    sub: Subcommand::Add {
                        name: s("foo"),
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

        it "accepts --restart-if-not" {
            check_args(
                &["shawl", "add", "--restart-if-not", "1,2", "--name", "foo", "--", "foo"],
                Cli {
                    sub: Subcommand::Add {
                        name: s("foo"),
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

        it "accepts --stop-timeout" {
            check_args(
                &["shawl", "add", "--stop-timeout", "500", "--name", "foo", "--", "foo"],
                Cli {
                    sub: Subcommand::Add {
                        name: s("foo"),
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
                        common: CommonOpts {
                            path: vec![p(&path1), p(&path2)],
                            command: vec![s("foo")],
                            ..Default::default()
                        }
                    }
                },
            );
        }
    }
}
