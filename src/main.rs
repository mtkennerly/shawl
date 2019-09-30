use log::{debug, error};
use std::io::Write;
use structopt::StructOpt;

#[derive(structopt::StructOpt, Debug, PartialEq)]
struct CommonOpts {
    /// Exit codes that should be considered successful (comma-separated)
    #[structopt(long, value_name = "codes", default_value = "0", use_delimiter(true))]
    pass: Vec<i32>,

    /// Always restart the command regardless of the exit code
    #[structopt(
        long,
        conflicts_with("no-restart"),
        conflicts_with("restart-if"),
        conflicts_with("restart-if-not")
    )]
    restart: bool,

    /// Never restart the command regardless of the exit code
    #[structopt(long)]
    no_restart: bool,

    /// Restart the command if the exit code is one of these (comma-separated)
    #[structopt(long, value_name = "codes", use_delimiter(true))]
    restart_if: Vec<i32>,

    /// Restart the command if the exit code is not one of these (comma-separated)
    #[structopt(long, value_name = "codes", use_delimiter(true))]
    restart_if_not: Vec<i32>,

    /// How long to wait in milliseconds between sending the wrapped process
    /// a ctrl-C event and forcibly killing it
    #[structopt(long, default_value = "3000", value_name = "ms")]
    stop_timeout: u64,

    /// Command to run as a service
    #[structopt(required(true), last(true))]
    command: Vec<String>,
}

#[derive(structopt::StructOpt, Debug, PartialEq)]
enum Subcommand {
    #[structopt(about = "Add a new service")]
    Add {
        #[structopt(flatten)]
        common: CommonOpts,

        /// Name of the service to create
        #[structopt(long)]
        name: String,
    },
    #[structopt(
        about = "Run a command as a service; only works when launched by the Windows service manager"
    )]
    Run {
        #[structopt(flatten)]
        common: CommonOpts,

        /// Name of the service; used in logging, but does not need to match real name
        #[structopt(long, default_value = "Shawl")]
        name: String,
    },
}

#[derive(structopt::StructOpt, Debug, PartialEq)]
#[structopt(
    name = "shawl",
    about = "Wrap arbitrary commands as Windows services",
    setting(structopt::clap::AppSettings::SubcommandsNegateReqs)
)]
struct Cli {
    #[structopt(subcommand)]
    sub: Subcommand,
}

fn prepare_logging(console: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut log_file = std::env::current_exe()?;
    log_file.pop();
    log_file.push("shawl.log");

    let mut loggers: Vec<Box<dyn simplelog::SharedLogger>> = vec![simplelog::WriteLogger::new(
        simplelog::LevelFilter::Debug,
        simplelog::ConfigBuilder::new()
            .set_time_format_str("%Y-%m-%d %H:%M:%S")
            .build(),
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)?,
    )];

    if console {
        loggers.push(
            simplelog::TermLogger::new(
                simplelog::LevelFilter::Info,
                simplelog::ConfigBuilder::new()
                    .set_time_format_str("")
                    .build(),
                simplelog::TerminalMode::default(),
            )
            .expect("Unable to create terminal logger"),
        );
    }

    simplelog::CombinedLogger::init(loggers)?;

    Ok(())
}

fn add_service(name: String, opts: CommonOpts) -> Result<(), ()> {
    let shawl_path = std::env::current_exe().expect("Unable to determine Shawl location");
    let shawl_args = construct_shawl_run_args(&name, &opts);

    let output = std::process::Command::new("sc")
        .arg("create")
        .arg(&name)
        .arg("binPath=")
        .arg(format!(
            "{} {} -- {}",
            shawl_path.display(),
            shawl_args.join(" "),
            opts.command.join(" ")
        ))
        .output()
        .expect("Failed to create the service");
    match output.status.code() {
        Some(0) => Ok(()),
        Some(x) => {
            error!("Failed to create the service. Error code: {}.", x);
            error!("SC stdout:\n{}", String::from_utf8_lossy(&output.stdout));
            error!("SC stderr:\n{}", String::from_utf8_lossy(&output.stderr));
            Err(())
        }
        None => {
            error!("Failed to create the service. Output:");
            std::io::stderr().write_all(&output.stdout).unwrap();
            std::io::stderr().write_all(&output.stderr).unwrap();
            Err(())
        }
    }
}

fn should_restart_exited_command(
    code: &i32,
    restart: bool,
    no_restart: bool,
    restart_if: &Vec<i32>,
    restart_if_not: &Vec<i32>,
) -> bool {
    if !restart_if.is_empty() {
        restart_if.contains(code)
    } else if !restart_if_not.is_empty() {
        !restart_if_not.contains(code)
    } else {
        restart || !no_restart && *code != 0
    }
}

fn should_restart_terminated_command(restart: bool, _no_restart: bool) -> bool {
    restart
}

fn construct_shawl_run_args(name: &String, opts: &CommonOpts) -> Vec<String> {
    let mut shawl_args = vec![
        "run".to_string(),
        "--name".to_string(),
        name.clone(),
        "--stop-timeout".to_string(),
        opts.stop_timeout.to_string(),
    ];
    if opts.restart {
        shawl_args.push("--restart".to_string());
    }
    if opts.no_restart {
        shawl_args.push("--no-restart".to_string());
    }
    if !opts.restart_if.is_empty() {
        shawl_args.push("--restart-if".to_string());
        shawl_args.push(
            opts.restart_if
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<String>>()
                .join(","),
        );
    }
    if !opts.restart_if_not.is_empty() {
        shawl_args.push("--restart-if-not".to_string());
        shawl_args.push(
            opts.restart_if_not
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<String>>()
                .join(","),
        );
    };
    if !opts.pass.is_empty() {
        shawl_args.push("--pass".to_string());
        shawl_args.push(
            opts.pass
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<String>>()
                .join(","),
        );
    }
    shawl_args
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::from_args();
    let console = match cli.sub {
        Subcommand::Run { .. } => false,
        _ => true,
    };

    prepare_logging(console)?;
    debug!("********** LAUNCH **********");
    debug!("{:?}", cli);

    match cli.sub {
        Subcommand::Add { name, common: opts } => match add_service(name, opts) {
            Ok(_) => (),
            Err(_) => std::process::exit(1),
        },
        Subcommand::Run { name, .. } => match service::run(name) {
            Ok(_) => (),
            Err(e) => {
                error!("Failed to run the service:\n{:#?}", e);
                // We wouldn't have a console if the Windows service manager
                // ran this, but if we failed here, then it's likely the user
                // tried to run it directly, so try showing them the error:
                println!("Failed to run the service:\n{:#?}", e);
                std::process::exit(1)
            }
        },
    }
    debug!("Finished successfully");
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    panic!("This program is only intended to run on Windows.");
}

enum ProcessStatus {
    Running,
    Exited(i32),
    Terminated,
}

fn check_process(
    child: &mut std::process::Child,
) -> Result<ProcessStatus, Box<dyn std::error::Error>> {
    match child.try_wait() {
        Ok(None) => Ok(ProcessStatus::Running),
        Ok(Some(status)) => match status.code() {
            Some(code) => Ok(ProcessStatus::Exited(code)),
            None => Ok(ProcessStatus::Terminated),
        },
        Err(e) => Err(Box::new(e)),
    }
}

#[cfg(windows)]
mod service {
    use log::{debug, error, info};
    use structopt::StructOpt;
    use windows_service::{
        define_windows_service,
        service::{
            ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
            ServiceType,
        },
        service_control_handler::{self, ServiceControlHandlerResult},
        service_dispatcher,
    };

    const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

    define_windows_service!(ffi_service_main, service_main);

    pub fn run(name: String) -> windows_service::Result<()> {
        service_dispatcher::start(name, ffi_service_main)
    }

    pub fn service_main(_arguments: Vec<std::ffi::OsString>) {
        unsafe {
            // Windows services don't start with a console, so we have to
            // allocate one in order to send ctrl-C to children.
            if winapi::um::consoleapi::AllocConsole() == 0 {
                error!(
                    "winapi AllocConsole failed with code {:?}",
                    winapi::um::errhandlingapi::GetLastError()
                );
            };
        }
        let _ = run_service();
    }

    pub fn run_service() -> windows_service::Result<()> {
        let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();
        let cli = crate::Cli::from_args();
        let (name, opts) = match cli.sub {
            crate::Subcommand::Run { name, common: opts } => (name, opts),
            _ => {
                // Can't get here.
                return Ok(());
            }
        };
        let mut service_exit_code = ServiceExitCode::NO_ERROR;

        let ignore_ctrlc = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let ignore_ctrlc2 = ignore_ctrlc.clone();
        ctrlc::set_handler(move || {
            if !ignore_ctrlc2.load(std::sync::atomic::Ordering::SeqCst) {
                std::process::abort();
            }
        })
        .expect("Unable to create ctrl-C handler");

        let event_handler = move |control_event| -> ServiceControlHandlerResult {
            match control_event {
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                ServiceControl::Stop => {
                    info!("Received stop event");
                    shutdown_tx.send(()).unwrap();
                    ServiceControlHandlerResult::NoError
                }
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        };

        let status_handle = service_control_handler::register(name, event_handler)?;

        status_handle.set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::NO_ERROR,
            checkpoint: 0,
            wait_hint: std::time::Duration::default(),
        })?;

        debug!("Entering main service loop");
        'outer: loop {
            info!("Launching command");
            let mut child = match std::process::Command::new(&opts.command[0])
                .args(&opts.command[1..])
                .spawn()
            {
                Ok(c) => c,
                Err(_) => break,
            };

            'inner: loop {
                match shutdown_rx.recv_timeout(std::time::Duration::from_secs(1)) {
                    Ok(_) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        status_handle.set_service_status(ServiceStatus {
                            service_type: SERVICE_TYPE,
                            current_state: ServiceState::StopPending,
                            controls_accepted: ServiceControlAccept::empty(),
                            exit_code: ServiceExitCode::NO_ERROR,
                            checkpoint: 0,
                            wait_hint: std::time::Duration::from_millis(opts.stop_timeout + 1000),
                        })?;

                        ignore_ctrlc.store(true, std::sync::atomic::Ordering::SeqCst);
                        info!("Sending ctrl-C to command");
                        unsafe {
                            if winapi::um::wincon::GenerateConsoleCtrlEvent(
                                winapi::um::wincon::CTRL_C_EVENT,
                                0,
                            ) == 0
                            {
                                error!(
                                    "winapi GenerateConsoleCtrlEvent failed with code {:?}",
                                    winapi::um::errhandlingapi::GetLastError()
                                );
                            };
                        }

                        let start_time = std::time::Instant::now();
                        loop {
                            match crate::check_process(&mut child) {
                                Ok(crate::ProcessStatus::Running) => {
                                    if start_time.elapsed().as_millis() < opts.stop_timeout.into() {
                                        std::thread::sleep(std::time::Duration::from_millis(50))
                                    } else {
                                        info!("Killing command because stop timeout expired");
                                        let _ = child.kill();
                                        service_exit_code = ServiceExitCode::NO_ERROR;
                                        break;
                                    }
                                }
                                Ok(crate::ProcessStatus::Exited(code)) => {
                                    info!(
                                        "Command exited after {:?} ms with code {:?}",
                                        start_time.elapsed().as_millis(),
                                        code
                                    );
                                    service_exit_code = match opts.pass.contains(&code) {
                                        true => ServiceExitCode::NO_ERROR,
                                        false => ServiceExitCode::ServiceSpecific(code as u32),
                                    };
                                    break;
                                }
                                _ => {
                                    info!("Command exited within stop timeout");
                                    break;
                                }
                            }
                        }

                        ignore_ctrlc.store(false, std::sync::atomic::Ordering::SeqCst);
                        break 'outer;
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => (),
                };

                match crate::check_process(&mut child) {
                    Ok(crate::ProcessStatus::Running) => (),
                    Ok(crate::ProcessStatus::Exited(code)) => {
                        info!("Command exited with code {:?}", code);
                        service_exit_code = match opts.pass.contains(&code) {
                            true => ServiceExitCode::NO_ERROR,
                            false => ServiceExitCode::ServiceSpecific(code as u32),
                        };
                        match crate::should_restart_exited_command(
                            &code,
                            opts.restart,
                            opts.no_restart,
                            &opts.restart_if,
                            &opts.restart_if_not,
                        ) {
                            true => break 'inner,
                            false => break 'outer,
                        }
                    }
                    Ok(crate::ProcessStatus::Terminated) => {
                        info!("Command was terminated by a signal");
                        service_exit_code =
                            ServiceExitCode::Win32(winapi::shared::winerror::ERROR_PROCESS_ABORTED);
                        match crate::should_restart_terminated_command(
                            opts.restart,
                            opts.no_restart,
                        ) {
                            true => break 'inner,
                            false => break 'outer,
                        }
                    }
                    Err(e) => {
                        info!("Error trying to determine command status: {:?}", e);
                        service_exit_code =
                            ServiceExitCode::Win32(winapi::shared::winerror::ERROR_PROCESS_ABORTED);
                        break 'inner;
                    }
                }
            }
        }
        debug!("Exited main service loop");

        status_handle.set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: service_exit_code,
            checkpoint: 0,
            wait_hint: std::time::Duration::default(),
        })?;

        Ok(())
    }
}

#[cfg(test)]
speculate::speculate! {
    fn check_args(args: &[&str], expected: Cli) {
        assert_eq!(
            expected,
            Cli::from_clap(&Cli::clap().get_matches_from(args))
        );
    }

    fn check_args_err(args: &[&str], error: structopt::clap::ErrorKind) {
        let result = Cli::clap().get_matches_from_safe(args);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind, error);
    }

    fn s(text: &str) -> String {
        return text.to_string();
    }

    describe "CLI" {
        describe "run subcommand" {
            it "works with minimal arguments" {
                check_args(
                    &["shawl", "run", "--", "foo"],
                    Cli {
                        sub: Subcommand::Run {
                            name: s("Shawl"),
                            common: CommonOpts {
                                pass: vec![0],
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: 3000,
                                command: vec![s("foo")],
                            }
                        }
                    },
                );
            }

            it "requires a command" {
                check_args_err(
                    &["shawl", "run"],
                    structopt::clap::ErrorKind::MissingRequiredArgument,
                );
            }

            it "accepts --pass" {
                check_args(
                    &["shawl", "run", "--pass", "1,2", "--", "foo"],
                    Cli {
                        sub: Subcommand::Run {
                            name: s("Shawl"),
                            common: CommonOpts {
                                pass: vec![1, 2],
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: 3000,
                                command: vec![s("foo")],
                            }
                        }
                    },
                );
            }

            it "accepts --restart" {
                check_args(
                    &["shawl", "run", "--restart", "--", "foo"],
                    Cli {
                        sub: Subcommand::Run {
                            name: s("Shawl"),
                            common: CommonOpts {
                                pass: vec![0],
                                restart: true,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: 3000,
                                command: vec![s("foo")],
                            }
                        }
                    },
                );
            }

            it "accepts --no-restart" {
                check_args(
                    &["shawl", "run", "--no-restart", "--", "foo"],
                    Cli {
                        sub: Subcommand::Run {
                            name: s("Shawl"),
                            common: CommonOpts {
                                pass: vec![0],
                                restart: false,
                                no_restart: true,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: 3000,
                                command: vec![s("foo")],
                            }
                        }
                    },
                );
            }

            it "accepts --restart-if" {
                check_args(
                    &["shawl", "run", "--restart-if", "1,2", "--", "foo"],
                    Cli {
                        sub: Subcommand::Run {
                            name: s("Shawl"),
                            common: CommonOpts {
                                pass: vec![0],
                                restart: false,
                                no_restart: false,
                                restart_if: vec![1, 2],
                                restart_if_not: vec![],
                                stop_timeout: 3000,
                                command: vec![s("foo")],
                            }
                        }
                    },
                );
            }

            it "accepts --restart-if-not" {
                check_args(
                    &["shawl", "run", "--restart-if-not", "1,2", "--", "foo"],
                    Cli {
                        sub: Subcommand::Run {
                            name: s("Shawl"),
                            common: CommonOpts {
                                pass: vec![0],
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![1, 2],
                                stop_timeout: 3000,
                                command: vec![s("foo")],
                            }
                        }
                    },
                );
            }

            it "accepts --stop-timeout" {
                check_args(
                    &["shawl", "run", "--stop-timeout", "500", "--", "foo"],
                    Cli {
                        sub: Subcommand::Run {
                            name: s("Shawl"),
                            common: CommonOpts {
                                pass: vec![0],
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: 500,
                                command: vec![s("foo")],
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
                            common: CommonOpts {
                                pass: vec![0],
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: 3000,
                                command: vec![s("foo")],
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
                            common: CommonOpts {
                                pass: vec![0],
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: 3000,
                                command: vec![s("foo")],
                            }
                        }
                    },
                );
            }

            it "requires a command" {
                check_args_err(
                    &["shawl", "add", "--name", "foo"],
                    structopt::clap::ErrorKind::MissingRequiredArgument,
                );
            }

            it "requires a name" {
                check_args_err(
                    &["shawl", "add", "--", "foo"],
                    structopt::clap::ErrorKind::MissingRequiredArgument,
                );
            }

            it "accepts --pass" {
                check_args(
                    &["shawl", "add", "--pass", "1,2", "--name", "foo", "--", "foo"],
                    Cli {
                        sub: Subcommand::Add {
                            name: s("foo"),
                            common: CommonOpts {
                                pass: vec![1, 2],
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: 3000,
                                command: vec![s("foo")],
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
                            common: CommonOpts {
                                pass: vec![0],
                                restart: true,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: 3000,
                                command: vec![s("foo")],
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
                            common: CommonOpts {
                                pass: vec![0],
                                restart: false,
                                no_restart: true,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: 3000,
                                command: vec![s("foo")],
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
                            common: CommonOpts {
                                pass: vec![0],
                                restart: false,
                                no_restart: false,
                                restart_if: vec![1, 2],
                                restart_if_not: vec![],
                                stop_timeout: 3000,
                                command: vec![s("foo")],
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
                            common: CommonOpts {
                                pass: vec![0],
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![1, 2],
                                stop_timeout: 3000,
                                command: vec![s("foo")],
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
                            common: CommonOpts {
                                pass: vec![0],
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: 500,
                                command: vec![s("foo")],
                            }
                        }
                    },
                );
            }
        }
    }

    describe "should_restart_exited_command" {
        it "handles --restart" {
            assert!(should_restart_exited_command(&5, true, false, &vec![], &vec![]));
        }

        it "handles --no-restart" {
            assert!(!should_restart_exited_command(&0, false, true, &vec![], &vec![]));
        }

        it "handles --restart-if" {
            assert!(should_restart_exited_command(&0, false, false, &vec![0], &vec![]));
            assert!(!should_restart_exited_command(&1, false, false, &vec![0], &vec![]));
        }

        it "handles --restart-if-not" {
            assert!(!should_restart_exited_command(&0, false, false, &vec![], &vec![0]));
            assert!(should_restart_exited_command(&1, false, false, &vec![], &vec![0]));
        }

        it "restarts nonzero by default" {
            assert!(!should_restart_exited_command(&0, false, false, &vec![], &vec![]));
            assert!(should_restart_exited_command(&1, false, false, &vec![], &vec![]));
        }
    }

    describe "should_restart_terminated_command" {
        it "only restarts with --restart" {
            assert!(!should_restart_terminated_command(false, false));
            assert!(should_restart_terminated_command(true, false));
            assert!(!should_restart_terminated_command(false, true));
        }
    }

    describe "construct_shawl_run_args" {
        it "works with minimal input" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &CommonOpts {
                        pass: vec![],
                        restart: false,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![],
                        stop_timeout: 3000,
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--stop-timeout", "3000"],
            );
        }

        it "handles --restart" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &CommonOpts {
                        pass: vec![],
                        restart: true,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![],
                        stop_timeout: 3000,
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--stop-timeout", "3000", "--restart"],
            );
        }

        it "handles --no-restart" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &CommonOpts {
                        pass: vec![],
                        restart: false,
                        no_restart: true,
                        restart_if: vec![],
                        restart_if_not: vec![],
                        stop_timeout: 3000,
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--stop-timeout", "3000", "--no-restart"],
            );
        }

        it "handles --restart-if with one code" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &CommonOpts {
                        pass: vec![],
                        restart: false,
                        no_restart: false,
                        restart_if: vec![0],
                        restart_if_not: vec![],
                        stop_timeout: 3000,
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--stop-timeout", "3000", "--restart-if", "0"],
            );
        }

        it "handles --restart-if with multiple codes" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &CommonOpts {
                        pass: vec![],
                        restart: false,
                        no_restart: false,
                        restart_if: vec![1, 10],
                        restart_if_not: vec![],
                        stop_timeout: 3000,
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--stop-timeout", "3000", "--restart-if", "1,10"],
            );
        }

        it "handles --restart-if-not with one code" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &CommonOpts {
                        pass: vec![],
                        restart: false,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![0],
                        stop_timeout: 3000,
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--stop-timeout", "3000", "--restart-if-not", "0"],
            );
        }

        it "handles --restart-if-not with multiple codes" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &CommonOpts {
                        pass: vec![],
                        restart: false,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![1, 10],
                        stop_timeout: 3000,
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--stop-timeout", "3000", "--restart-if-not", "1,10"],
            );
        }

        it "handles --pass with one code" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &CommonOpts {
                        pass: vec![0],
                        restart: false,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![],
                        stop_timeout: 3000,
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--stop-timeout", "3000", "--pass", "0"],
            );
        }

        it "handles --pass with multiple codes" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &CommonOpts {
                        pass: vec![1, 10],
                        restart: false,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![],
                        stop_timeout: 3000,
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--stop-timeout", "3000", "--pass", "1,10"],
            );
        }
    }
}
