use log::{debug, error};
use std::io::Write;
use structopt::StructOpt;

fn parse_canonical_path(path: &str) -> Result<String, std::io::Error> {
    Ok(std::fs::canonicalize(path)?.to_string_lossy().to_string())
}

fn parse_ensured_directory(path: &str) -> Result<String, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(&path)?;
    Ok(std::fs::canonicalize(path)?.to_string_lossy().to_string())
}

fn parse_env_var(value: &str) -> Result<(String, String), Box<dyn std::error::Error>> {
    let parts: Vec<&str> = value.splitn(2, '=').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid KEY=value formatting in '{}'", value).into());
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

#[derive(structopt::StructOpt, Clone, Debug, PartialEq)]
struct CommonOpts {
    /// Exit codes that should be considered successful (comma-separated) [default: 0]
    #[structopt(
        long,
        value_name = "codes",
        use_delimiter(true),
        number_of_values = 1,
        allow_hyphen_values(true)
    )]
    pass: Option<Vec<i32>>,

    /// Always restart the command regardless of the exit code
    #[structopt(
        long,
        conflicts_with("no-restart"),
        conflicts_with("restart-if"),
        conflicts_with("restart-if-not")
    )]
    restart: bool,

    /// Never restart the command regardless of the exit code
    #[structopt(
        long,
        conflicts_with("restart"),
        conflicts_with("restart-if"),
        conflicts_with("restart-if-not")
    )]
    no_restart: bool,

    /// Restart the command if the exit code is one of these (comma-separated)
    #[structopt(
        long,
        conflicts_with("restart"),
        conflicts_with("no-restart"),
        conflicts_with("restart-if-not"),
        value_name = "codes",
        use_delimiter(true),
        number_of_values = 1,
        allow_hyphen_values(true)
    )]
    restart_if: Vec<i32>,

    /// Restart the command if the exit code is not one of these (comma-separated)
    #[structopt(
        long,
        conflicts_with("restart"),
        conflicts_with("no-restart"),
        conflicts_with("restart-if"),
        value_name = "codes",
        use_delimiter(true),
        number_of_values = 1,
        allow_hyphen_values(true)
    )]
    restart_if_not: Vec<i32>,

    /// How long to wait in milliseconds between sending the wrapped process
    /// a ctrl-C event and forcibly killing it [default: 3000]
    #[structopt(long, value_name = "ms")]
    stop_timeout: Option<u64>,

    /// Disable all of Shawl's logging
    #[structopt(long)]
    no_log: bool,

    /// Disable logging of output from the command running as a service
    #[structopt(long)]
    no_log_cmd: bool,

    /// Write log file to a custom directory. This directory will be created if it doesn't exist.
    #[structopt(long, value_name = "path", parse(try_from_str = parse_ensured_directory))]
    log_dir: Option<String>,

    /// Append the service start arguments to the command
    #[structopt(long)]
    pass_start_args: bool,

    /// Additional environment variable in the format 'KEY=value' (repeatable)
    #[structopt(long, number_of_values = 1, parse(try_from_str = parse_env_var))]
    env: Vec<(String, String)>,

    /// Additional directory to add to the PATH environment variable (repeatable)
    #[structopt(long, number_of_values = 1, parse(try_from_str = parse_canonical_path))]
    path: Vec<String>,

    /// Command to run as a service
    #[structopt(required(true), last(true))]
    command: Vec<String>,
}

#[derive(structopt::StructOpt, Clone, Debug, PartialEq)]
enum Subcommand {
    #[structopt(about = "Add a new service")]
    Add {
        #[structopt(flatten)]
        common: CommonOpts,

        /// Working directory in which to run the command. You may provide a
        /// relative path, and it will be converted to an absolute one
        #[structopt(long, value_name = "path", parse(try_from_str = parse_canonical_path))]
        cwd: Option<String>,

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

        /// Working directory in which to run the command. Must be an absolute path
        #[structopt(long, value_name = "path")]
        cwd: Option<String>,

        /// Name of the service; used in logging, but does not need to match real name
        #[structopt(long, default_value = "Shawl")]
        name: String,
    },
}

#[derive(structopt::StructOpt, Clone, Debug, PartialEq)]
#[structopt(
    name = "shawl",
    about = "Wrap arbitrary commands as Windows services",
    set_term_width = 80,
    setting(structopt::clap::AppSettings::SubcommandsNegateReqs)
)]
struct Cli {
    #[structopt(subcommand)]
    sub: Subcommand,
}

fn prepare_logging(
    name: &str,
    log_dir: Option<String>,
    console: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut exe_dir = std::env::current_exe()?;
    exe_dir.pop();

    let mut logger = flexi_logger::Logger::with_env_or_str("debug")
        .log_to_file()
        .directory(exe_dir)
        .discriminant(format!("for_{}", name))
        .append()
        .rotate(
            flexi_logger::Criterion::Size(1024 * 1024 * 2),
            flexi_logger::Naming::Timestamps,
            flexi_logger::Cleanup::KeepLogFiles(2),
        )
        .format_for_files(|w, now, record| {
            write!(
                w,
                "{} [{}] {}",
                now.now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                &record.args()
            )
        })
        .format_for_stderr(|w, _now, record| write!(w, "[{}] {}", record.level(), &record.args()));

    // Set custom log directory
    if let Some(dir) = log_dir {
        logger = logger.o_directory(Some(dir));
    }

    if console {
        logger = logger.duplicate_to_stderr(flexi_logger::Duplicate::Info);
    }

    logger.start()?;
    Ok(())
}

fn add_service(name: String, cwd: Option<String>, opts: CommonOpts) -> Result<(), ()> {
    let shawl_path = std::env::current_exe().expect("Unable to determine Shawl location");
    let shawl_args = construct_shawl_run_args(&name, &cwd, &opts);
    let prepared_command = prepare_command(&opts.command);

    let output = std::process::Command::new("sc")
        .arg("create")
        .arg(&name)
        .arg("binPath=")
        .arg(format!(
            "{} {} -- {}",
            shawl_path.display(),
            shawl_args.join(" "),
            prepared_command.join(" ")
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
    code: i32,
    restart: bool,
    no_restart: bool,
    restart_if: &[i32],
    restart_if_not: &[i32],
) -> bool {
    if !restart_if.is_empty() {
        restart_if.contains(&code)
    } else if !restart_if_not.is_empty() {
        !restart_if_not.contains(&code)
    } else {
        restart || !no_restart && code != 0
    }
}

fn should_restart_terminated_command(restart: bool, _no_restart: bool) -> bool {
    restart
}

fn construct_shawl_run_args(name: &str, cwd: &Option<String>, opts: &CommonOpts) -> Vec<String> {
    let mut shawl_args = vec!["run".to_string(), "--name".to_string(), quote(name)];
    if let Some(st) = opts.stop_timeout {
        shawl_args.push("--stop-timeout".to_string());
        shawl_args.push(st.to_string());
    }
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
    if let Some(pass) = &opts.pass {
        shawl_args.push("--pass".to_string());
        shawl_args.push(
            pass.iter()
                .map(|x| x.to_string())
                .collect::<Vec<String>>()
                .join(","),
        );
    }
    if let Some(cwd) = &cwd {
        shawl_args.push("--cwd".to_string());
        shawl_args.push(quote(cwd));
    };
    if opts.no_log {
        shawl_args.push("--no-log".to_string());
    }
    if opts.no_log_cmd {
        shawl_args.push("--no-log-cmd".to_string());
    }
    if let Some(log_dir) = &opts.log_dir {
        shawl_args.push("--log-dir".to_string());
        shawl_args.push(quote(log_dir));
    }
    if opts.pass_start_args {
        shawl_args.push("--pass-start-args".to_string());
    }
    if !opts.env.is_empty() {
        for (x, y) in &opts.env {
            shawl_args.push("--env".to_string());
            shawl_args.push(quote(&format!("{}={}", x, y)));
        }
    }
    if !opts.path.is_empty() {
        for path in &opts.path {
            shawl_args.push("--path".to_string());
            shawl_args.push(quote(path));
        }
    }
    shawl_args
}

fn prepare_command(command: &[String]) -> Vec<String> {
    command.iter().map(|x| quote(x)).collect::<Vec<String>>()
}

fn quote(text: &str) -> String {
    if text.contains(' ') {
        format!("\"{}\"", text)
    } else {
        text.to_owned()
    }
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::from_args();
    let console = !matches!(cli.sub, Subcommand::Run { .. });

    let should_log = match cli.clone().sub {
        Subcommand::Add { common: opts, .. } => !opts.no_log,
        Subcommand::Run { common: opts, .. } => !opts.no_log,
    };
    if should_log {
        let name = match cli.clone().sub {
            Subcommand::Add { name, .. } => name,
            Subcommand::Run { name, .. } => name,
        };
        let log_dir = match cli.clone().sub {
            Subcommand::Add { common, .. } => common.log_dir,
            Subcommand::Run { common, .. } => common.log_dir,
        };
        prepare_logging(&name, log_dir, console)?;
    }

    debug!("********** LAUNCH **********");
    debug!("{:?}", cli);

    match cli.sub {
        Subcommand::Add {
            name,
            cwd,
            common: opts,
        } => match add_service(name, cwd, opts) {
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
    use std::io::BufRead;
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

    pub fn service_main(mut arguments: Vec<std::ffi::OsString>) {
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
        if !arguments.is_empty() {
            // first argument is the service name
            arguments.remove(0);
        }
        let _ = run_service(arguments);
    }

    #[allow(clippy::cognitive_complexity)]
    pub fn run_service(start_arguments: Vec<std::ffi::OsString>) -> windows_service::Result<()> {
        let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();
        let cli = crate::Cli::from_args();
        let (name, cwd, opts) = match cli.sub {
            crate::Subcommand::Run {
                name,
                cwd,
                common: opts,
            } => (name, cwd, opts),
            _ => {
                // Can't get here.
                return Ok(());
            }
        };
        let pass = &opts.pass.unwrap_or_else(|| vec![0]);
        let stop_timeout = &opts.stop_timeout.unwrap_or(3000_u64);
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
                ServiceControl::Shutdown => {
                    info!("Received shutdown event");
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
            controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
            exit_code: ServiceExitCode::NO_ERROR,
            checkpoint: 0,
            wait_hint: std::time::Duration::default(),
            process_id: None,
        })?;

        let mut command = opts.command.into_iter();
        let program = command.next().unwrap();
        let mut args: Vec<_> = command.map(std::ffi::OsString::from).collect();
        if opts.pass_start_args {
            args.extend(start_arguments);
        }

        debug!("Entering main service loop");
        'outer: loop {
            info!("Launching command");
            let should_log_cmd = !&opts.no_log_cmd;
            let mut child_cmd = std::process::Command::new(&program);

            child_cmd
                .args(&args)
                .stdout(if should_log_cmd {
                    std::process::Stdio::piped()
                } else {
                    std::process::Stdio::null()
                })
                .stderr(if should_log_cmd {
                    std::process::Stdio::piped()
                } else {
                    std::process::Stdio::null()
                });
            for (key, value) in &opts.env {
                child_cmd.env(key, value);
            }
            if !opts.path.is_empty() {
                child_cmd.env(
                    "PATH",
                    match std::env::var("PATH") {
                        Ok(path) => format!("{};{}", path, &opts.path.join(";")),
                        Err(_) => opts.path.join(";").to_string(),
                    },
                );
            }
            if let Some(active_cwd) = &cwd {
                child_cmd.current_dir(active_cwd);
                child_cmd.env(
                    "PATH",
                    match std::env::var("PATH") {
                        Ok(path) => format!("{};{}", path, active_cwd),
                        Err(_) => active_cwd.to_string(),
                    },
                );
            }
            let mut child = match child_cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    error!("Unable to launch command: {}", e);
                    service_exit_code = match e.raw_os_error() {
                        Some(win_code) => ServiceExitCode::Win32(win_code as u32),
                        None => {
                            ServiceExitCode::Win32(winapi::shared::winerror::ERROR_PROCESS_ABORTED)
                        }
                    };
                    break;
                }
            };

            // Log stdout.
            let stdout_option = child.stdout.take();
            let stdout_logger = std::thread::spawn(move || {
                if !should_log_cmd {
                    return;
                }
                if let Some(stdout) = stdout_option {
                    std::io::BufReader::new(stdout)
                        .lines()
                        .for_each(|line| match line {
                            Ok(ref x) if !x.is_empty() => debug!("stdout: {:?}", x),
                            _ => (),
                        });
                }
            });

            // Log stderr.
            let stderr_option = child.stderr.take();
            let stderr_logger = std::thread::spawn(move || {
                if !should_log_cmd {
                    return;
                }
                if let Some(stderr) = stderr_option {
                    std::io::BufReader::new(stderr)
                        .lines()
                        .for_each(|line| match line {
                            Ok(ref x) if !x.is_empty() => debug!("stderr: {:?}", x),
                            _ => (),
                        });
                }
            });

            'inner: loop {
                match shutdown_rx.recv_timeout(std::time::Duration::from_secs(1)) {
                    Ok(_) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        status_handle.set_service_status(ServiceStatus {
                            service_type: SERVICE_TYPE,
                            current_state: ServiceState::StopPending,
                            controls_accepted: ServiceControlAccept::empty(),
                            exit_code: ServiceExitCode::NO_ERROR,
                            checkpoint: 0,
                            wait_hint: std::time::Duration::from_millis(
                                opts.stop_timeout.unwrap_or(3000) + 1000,
                            ),
                            process_id: None,
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
                                    if start_time.elapsed().as_millis() < (*stop_timeout).into() {
                                        std::thread::sleep(std::time::Duration::from_millis(50))
                                    } else {
                                        info!("Killing command because stop timeout expired",);
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
                                    service_exit_code = if pass.contains(&code) {
                                        ServiceExitCode::NO_ERROR
                                    } else {
                                        ServiceExitCode::ServiceSpecific(code as u32)
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
                        service_exit_code = if pass.contains(&code) {
                            ServiceExitCode::NO_ERROR
                        } else {
                            ServiceExitCode::ServiceSpecific(code as u32)
                        };
                        if crate::should_restart_exited_command(
                            code,
                            opts.restart,
                            opts.no_restart,
                            &opts.restart_if,
                            &opts.restart_if_not,
                        ) {
                            break 'inner;
                        } else {
                            break 'outer;
                        }
                    }
                    Ok(crate::ProcessStatus::Terminated) => {
                        info!("Command was terminated by a signal");
                        service_exit_code =
                            ServiceExitCode::Win32(winapi::shared::winerror::ERROR_PROCESS_ABORTED);
                        if crate::should_restart_terminated_command(opts.restart, opts.no_restart) {
                            break 'inner;
                        } else {
                            break 'outer;
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

            if let Err(e) = stdout_logger.join() {
                error!("Unable to join stdout logger thread: {:?}", e);
            }
            if let Err(e) = stderr_logger.join() {
                error!("Unable to join stderr logger thread: {:?}", e);
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
            process_id: None,
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
        text.to_string()
    }

    fn p(path: &str) -> String {
        std::fs::canonicalize(&path).unwrap().to_string_lossy().to_string()
    }

    describe "CLI" {
        describe "run subcommand" {
            it "works with minimal arguments" {
                check_args(
                    &["shawl", "run", "--", "foo"],
                    Cli {
                        sub: Subcommand::Run {
                            name: s("Shawl"),
                            cwd: None,
                            common: CommonOpts {
                                pass: None,
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: None,
                                no_log: false,
                                no_log_cmd: false,
                                log_dir: None,
                                pass_start_args: false,
                                env: vec![],
                                path: vec![],
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
                            cwd: None,
                            common: CommonOpts {
                                pass: Some(vec![1, 2]),
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: None,
                                no_log: false,
                                no_log_cmd: false,
                                log_dir: None,
                                pass_start_args: false,
                                env: vec![],
                                path: vec![],
                                command: vec![s("foo")],
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
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: None,
                                no_log: false,
                                no_log_cmd: false,
                                log_dir: None,
                                pass_start_args: false,
                                env: vec![],
                                path: vec![],
                                command: vec![s("foo")],
                            }
                        }
                    },
                );
            }

            it "rejects --pass without value" {
                check_args_err(
                    &["shawl", "run", "--pass", "--", "foo"],
                    structopt::clap::ErrorKind::UnknownArgument,
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
                                pass: None,
                                restart: true,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: None,
                                no_log: false,
                                no_log_cmd: false,
                                log_dir: None,
                                pass_start_args: false,
                                env: vec![],
                                path: vec![],
                                command: vec![s("foo")],
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
                        structopt::clap::ErrorKind::ArgumentConflict,
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
                                pass: None,
                                restart: false,
                                no_restart: true,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: None,
                                no_log: false,
                                no_log_cmd: false,
                                log_dir: None,
                                pass_start_args: false,
                                env: vec![],
                                path: vec![],
                                command: vec![s("foo")],
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
                        structopt::clap::ErrorKind::ArgumentConflict,
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
                                pass: None,
                                restart: false,
                                no_restart: false,
                                restart_if: vec![1, 2],
                                restart_if_not: vec![],
                                stop_timeout: None,
                                no_log: false,
                                no_log_cmd: false,
                                log_dir: None,
                                pass_start_args: false,
                                env: vec![],
                                path: vec![],
                                command: vec![s("foo")],
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
                                pass: None,
                                restart: false,
                                no_restart: false,
                                restart_if: vec![-1],
                                restart_if_not: vec![],
                                stop_timeout: None,
                                no_log: false,
                                no_log_cmd: false,
                                log_dir: None,
                                pass_start_args: false,
                                env: vec![],
                                path: vec![],
                                command: vec![s("foo")],
                            }
                        }
                    },
                );
            }

            it "rejects --restart-if without value" {
                check_args_err(
                    &["shawl", "run", "--restart-if", "--", "foo"],
                    structopt::clap::ErrorKind::UnknownArgument,
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
                        structopt::clap::ErrorKind::ArgumentConflict,
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
                                pass: None,
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![1, 2],
                                stop_timeout: None,
                                no_log: false,
                                no_log_cmd: false,
                                log_dir: None,
                                pass_start_args: false,
                                env: vec![],
                                path: vec![],
                                command: vec![s("foo")],
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
                                pass: None,
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![-1],
                                stop_timeout: None,
                                no_log: false,
                                no_log_cmd: false,
                                log_dir: None,
                                pass_start_args: false,
                                env: vec![],
                                path: vec![],
                                command: vec![s("foo")],
                            }
                        }
                    },
                );
            }

            it "rejects --restart-if-not without value" {
                check_args_err(
                    &["shawl", "run", "--restart-if-not", "--", "foo"],
                    structopt::clap::ErrorKind::UnknownArgument,
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
                        structopt::clap::ErrorKind::ArgumentConflict,
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
                                pass: None,
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: Some(500),
                                no_log: false,
                                no_log_cmd: false,
                                log_dir: None,
                                pass_start_args: false,
                                env: vec![],
                                path: vec![],
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
                            cwd: None,
                            common: CommonOpts {
                                pass: None,
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: None,
                                no_log: false,
                                no_log_cmd: false,
                                log_dir: None,
                                pass_start_args: false,
                                env: vec![],
                                path: vec![],
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
                            cwd: None,
                            common: CommonOpts {
                                pass: None,
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: None,
                                no_log: false,
                                no_log_cmd: false,
                                log_dir: None,
                                pass_start_args: false,
                                env: vec![],
                                path: vec![],
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
                            cwd: None,
                            common: CommonOpts {
                                pass: Some(vec![1, 2]),
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: None,
                                no_log: false,
                                no_log_cmd: false,
                                log_dir: None,
                                pass_start_args: false,
                                env: vec![],
                                path: vec![],
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
                            cwd: None,
                            common: CommonOpts {
                                pass: None,
                                restart: true,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: None,
                                no_log: false,
                                no_log_cmd: false,
                                log_dir: None,
                                pass_start_args: false,
                                env: vec![],
                                path: vec![],
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
                            cwd: None,
                            common: CommonOpts {
                                pass: None,
                                restart: false,
                                no_restart: true,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: None,
                                no_log: false,
                                no_log_cmd: false,
                                log_dir: None,
                                pass_start_args: false,
                                env: vec![],
                                path: vec![],
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
                            cwd: None,
                            common: CommonOpts {
                                pass: None,
                                restart: false,
                                no_restart: false,
                                restart_if: vec![1, 2],
                                restart_if_not: vec![],
                                stop_timeout: None,
                                no_log: false,
                                no_log_cmd: false,
                                log_dir: None,
                                pass_start_args: false,
                                env: vec![],
                                path: vec![],
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
                            cwd: None,
                            common: CommonOpts {
                                pass: None,
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![1, 2],
                                stop_timeout: None,
                                no_log: false,
                                no_log_cmd: false,
                                log_dir: None,
                                pass_start_args: false,
                                env: vec![],
                                path: vec![],
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
                            cwd: None,
                            common: CommonOpts {
                                pass: None,
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: Some(500),
                                no_log: false,
                                no_log_cmd: false,
                                log_dir: None,
                                pass_start_args: false,
                                env: vec![],
                                path: vec![],
                                command: vec![s("foo")],
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
                                pass: None,
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: None,
                                no_log: true,
                                no_log_cmd: false,
                                log_dir: None,
                                pass_start_args: false,
                                env: vec![],
                                path: vec![],
                                command: vec![s("foo")],
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
                                pass: None,
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: None,
                                no_log: false,
                                no_log_cmd: true,
                                log_dir: None,
                                pass_start_args: false,
                                env: vec![],
                                path: vec![],
                                command: vec![s("foo")],
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
                                pass: None,
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: None,
                                no_log: false,
                                no_log_cmd: false,
                                log_dir: Some(p(path)),
                                pass_start_args: false,
                                env: vec![],
                                path: vec![],
                                command: vec![s("foo")],
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
                                pass: None,
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: None,
                                no_log: false,
                                no_log_cmd: false,
                                log_dir: None,
                                pass_start_args: true,
                                env: vec![],
                                path: vec![],
                                command: vec![s("foo")],
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
                                pass: None,
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: None,
                                no_log: false,
                                no_log_cmd: false,
                                log_dir: None,
                                pass_start_args: false,
                                env: vec![(s("FOO"), s("bar"))],
                                path: vec![],
                                command: vec![s("foo")],
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
                                pass: None,
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: None,
                                no_log: false,
                                no_log_cmd: false,
                                log_dir: None,
                                pass_start_args: false,
                                env: vec![(s("FOO"), s("1")), (s("BAR"), s("2"))],
                                path: vec![],
                                command: vec![s("foo")],
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
                                pass: None,
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: None,
                                no_log: false,
                                no_log_cmd: false,
                                log_dir: None,
                                pass_start_args: false,
                                env: vec![],
                                path: vec![p(path)],
                                command: vec![s("foo")],
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
                                pass: None,
                                restart: false,
                                no_restart: false,
                                restart_if: vec![],
                                restart_if_not: vec![],
                                stop_timeout: None,
                                no_log: false,
                                no_log_cmd: false,
                                log_dir: None,
                                pass_start_args: false,
                                env: vec![],
                                path: vec![p(&path1), p(&path2)],
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
            assert!(should_restart_exited_command(5, true, false, &[], &[]));
        }

        it "handles --no-restart" {
            assert!(!should_restart_exited_command(0, false, true, &[], &[]));
        }

        it "handles --restart-if" {
            assert!(should_restart_exited_command(0, false, false, &[0], &[]));
            assert!(!should_restart_exited_command(1, false, false, &[0], &[]));
        }

        it "handles --restart-if-not" {
            assert!(!should_restart_exited_command(0, false, false, &[], &[0]));
            assert!(should_restart_exited_command(1, false, false, &[], &[0]));
        }

        it "restarts nonzero by default" {
            assert!(!should_restart_exited_command(0, false, false, &[], &[]));
            assert!(should_restart_exited_command(1, false, false, &[], &[]));
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
                    &None,
                    &CommonOpts {
                        pass: None,
                        restart: false,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![],
                        stop_timeout: None,
                        no_log: false,
                        no_log_cmd: false,
                        log_dir: None,
                        pass_start_args: false,
                        env: vec![],
                        path: vec![],
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl"],
            );
        }

        it "handles --name with spaces" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("C:/Program Files/shawl"),
                    &None,
                    &CommonOpts {
                        pass: None,
                        restart: false,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![],
                        stop_timeout: None,
                        no_log: false,
                        no_log_cmd: false,
                        log_dir: None,
                        pass_start_args: false,
                        env: vec![],
                        path: vec![],
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "\"C:/Program Files/shawl\""],
            );
        }

        it "handles --restart" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        pass: None,
                        restart: true,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![],
                        stop_timeout: None,
                        no_log: false,
                        no_log_cmd: false,
                        log_dir: None,
                        pass_start_args: false,
                        env: vec![],
                        path: vec![],
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--restart"],
            );
        }

        it "handles --no-restart" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        pass: None,
                        restart: false,
                        no_restart: true,
                        restart_if: vec![],
                        restart_if_not: vec![],
                        stop_timeout: None,
                        no_log: false,
                        no_log_cmd: false,
                        log_dir: None,
                        pass_start_args: false,
                        env: vec![],
                        path: vec![],
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--no-restart"],
            );
        }

        it "handles --restart-if with one code" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        pass: None,
                        restart: false,
                        no_restart: false,
                        restart_if: vec![0],
                        restart_if_not: vec![],
                        stop_timeout: None,
                        no_log: false,
                        no_log_cmd: false,
                        log_dir: None,
                        pass_start_args: false,
                        env: vec![],
                        path: vec![],
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--restart-if", "0"],
            );
        }

        it "handles --restart-if with multiple codes" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        pass: None,
                        restart: false,
                        no_restart: false,
                        restart_if: vec![1, 10],
                        restart_if_not: vec![],
                        stop_timeout: None,
                        no_log: false,
                        no_log_cmd: false,
                        log_dir: None,
                        pass_start_args: false,
                        env: vec![],
                        path: vec![],
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--restart-if", "1,10"],
            );
        }

        it "handles --restart-if-not with one code" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        pass: None,
                        restart: false,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![0],
                        stop_timeout: None,
                        no_log: false,
                        no_log_cmd: false,
                        log_dir: None,
                        pass_start_args: false,
                        env: vec![],
                        path: vec![],
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--restart-if-not", "0"],
            );
        }

        it "handles --restart-if-not with multiple codes" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        pass: None,
                        restart: false,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![1, 10],
                        stop_timeout: None,
                        no_log: false,
                        no_log_cmd: false,
                        log_dir: None,
                        pass_start_args: false,
                        env: vec![],
                        path: vec![],
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--restart-if-not", "1,10"],
            );
        }

        it "handles --pass with one code" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        pass: Some(vec![0]),
                        restart: false,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![],
                        stop_timeout: None,
                        no_log: false,
                        no_log_cmd: false,
                        log_dir: None,
                        pass_start_args: false,
                        env: vec![],
                        path: vec![],
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--pass", "0"],
            );
        }

        it "handles --pass with multiple codes" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        pass: Some(vec![1, 10]),
                        restart: false,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![],
                        stop_timeout: None,
                        no_log: false,
                        no_log_cmd: false,
                        log_dir: None,
                        pass_start_args: false,
                        env: vec![],
                        path: vec![],
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--pass", "1,10"],
            );
        }

        it "handles --stop-timeout" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        pass: None,
                        restart: false,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![],
                        stop_timeout: Some(3000),
                        no_log: false,
                        no_log_cmd: false,
                        log_dir: None,
                        pass_start_args: false,
                        env: vec![],
                        path: vec![],
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--stop-timeout", "3000"],
            );
        }

        it "handles --cwd without spaces" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &Some(s("C:/foo")),
                    &CommonOpts {
                        pass: None,
                        restart: false,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![],
                        stop_timeout: None,
                        no_log: false,
                        no_log_cmd: false,
                        log_dir: None,
                        pass_start_args: false,
                        env: vec![],
                        path: vec![],
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--cwd", "C:/foo"],
            );
        }

        it "handles --cwd with spaces" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &Some(s("C:/Program Files/foo")),
                    &CommonOpts {
                        pass: None,
                        restart: false,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![],
                        stop_timeout: None,
                        no_log: false,
                        no_log_cmd: false,
                        log_dir: None,
                        pass_start_args: false,
                        env: vec![],
                        path: vec![],
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--cwd", "\"C:/Program Files/foo\""],
            );
        }

        it "handles --no-log" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        pass: None,
                        restart: false,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![],
                        stop_timeout: None,
                        no_log: true,
                        no_log_cmd: false,
                        log_dir: None,
                        pass_start_args: false,
                        env: vec![],
                        path: vec![],
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--no-log"],
            );
        }
        it "handles --no-log-cmd" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        pass: None,
                        restart: false,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![],
                        stop_timeout: None,
                        no_log: false,
                        no_log_cmd: true,
                        log_dir: None,
                        pass_start_args: false,
                        env: vec![],
                        path: vec![],
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--no-log-cmd"],
            );
        }

        it "handles --log-dir without spaces" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        pass: None,
                        restart: false,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![],
                        stop_timeout: None,
                        no_log: false,
                        no_log_cmd: false,
                        log_dir: Some("C:/foo".to_string()),
                        pass_start_args: false,
                        env: vec![],
                        path: vec![],
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--log-dir", "C:/foo"],
            );
        }

        it "handles --log-dir with spaces" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        pass: None,
                        restart: false,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![],
                        stop_timeout: None,
                        no_log: false,
                        no_log_cmd: false,
                        log_dir: Some("C:/foo bar/hello".to_string()),
                        pass_start_args: false,
                        env: vec![],
                        path: vec![],
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--log-dir", "\"C:/foo bar/hello\""],
            );
        }

        it "handles --pass-start-args" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        pass: None,
                        restart: false,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![],
                        stop_timeout: None,
                        no_log: false,
                        no_log_cmd: false,
                        log_dir: None,
                        pass_start_args: true,
                        env: vec![],
                        path: vec![],
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--pass-start-args"],
            );
        }

        it "handles --env without spaces" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        pass: None,
                        restart: false,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![],
                        stop_timeout: None,
                        no_log: false,
                        no_log_cmd: false,
                        log_dir: None,
                        pass_start_args: false,
                        env: vec![(s("FOO"), s("bar"))],
                        path: vec![],
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--env", "FOO=bar"],
            );
        }

        it "handles --env with spaces" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        pass: None,
                        restart: false,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![],
                        stop_timeout: None,
                        no_log: false,
                        no_log_cmd: false,
                        log_dir: None,
                        pass_start_args: false,
                        env: vec![(s("FOO"), s("bar baz"))],
                        path: vec![],
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--env", "\"FOO=bar baz\""],
            );
        }

        it "handles --env multiple times" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        pass: None,
                        restart: false,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![],
                        stop_timeout: None,
                        no_log: false,
                        no_log_cmd: false,
                        log_dir: None,
                        pass_start_args: false,
                        env: vec![(s("FOO"), s("1")), (s("BAR"), s("2"))],
                        path: vec![],
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--env", "FOO=1", "--env", "BAR=2"],
            );
        }

        it "handles --path without spaces" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        pass: None,
                        restart: false,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![],
                        stop_timeout: None,
                        no_log: false,
                        no_log_cmd: false,
                        log_dir: None,
                        pass_start_args: false,
                        env: vec![],
                        path: vec![s("C:/foo")],
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--path", "C:/foo"],
            );
        }

        it "handles --path with spaces" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        pass: None,
                        restart: false,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![],
                        stop_timeout: None,
                        no_log: false,
                        no_log_cmd: false,
                        log_dir: None,
                        pass_start_args: false,
                        env: vec![],
                        path: vec![s("C:/foo bar")],
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--path", "\"C:/foo bar\""],
            );
        }

        it "handles --path multiple times" {
            assert_eq!(
                construct_shawl_run_args(
                    &s("shawl"),
                    &None,
                    &CommonOpts {
                        pass: None,
                        restart: false,
                        no_restart: false,
                        restart_if: vec![],
                        restart_if_not: vec![],
                        stop_timeout: None,
                        no_log: false,
                        no_log_cmd: false,
                        log_dir: None,
                        pass_start_args: false,
                        env: vec![],
                        path: vec![s("C:/foo"), s("C:/bar")],
                        command: vec![s("foo")],
                    }
                ),
                vec!["run", "--name", "shawl", "--path", "C:/foo", "--path", "C:/bar"],
            );
        }
    }

    describe "prepare_command" {
        it "handles commands without inner spaces" {
            assert_eq!(
                prepare_command(&[s("cat"), s("file")]),
                vec![s("cat"), s("file")],
            );
        }

        it "handles commands with inner spaces" {
            assert_eq!(
                prepare_command(&[s("cat"), s("some file")]),
                vec![s("cat"), s("\"some file\"")],
            );
        }
    }
}
