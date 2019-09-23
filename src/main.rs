use log::{debug, error};
use std::io::Write;
use structopt::StructOpt;

#[derive(structopt::StructOpt, Debug)]
struct CommonOpts {
    /// Restart the wrapped program if it exits with 0
    #[structopt(long)]
    restart_ok: bool,

    /// Do not restart the wrapped program if it exits with a nonzero code
    #[structopt(long)]
    no_restart_err: bool,

    /// How long to wait in milliseconds between sending the wrapped process
    /// a ctrl-C event and forcibly killing it
    #[structopt(long, default_value = "3000")]
    stop_timeout: u64,

    /// Command to run as a service
    #[structopt(required(true), last(true))]
    command: Vec<String>,
}

#[derive(structopt::StructOpt, Debug)]
enum Subcommand {
    #[structopt(about = "Add a new service")]
    Add {
        #[structopt(flatten)]
        common: CommonOpts,

        /// Name of the service to create
        #[structopt(long)]
        name: String,
    },
    #[structopt(about = "Run a command as a service; only works when launched by the Windows service manager")]
    Run {
        #[structopt(flatten)]
        common: CommonOpts,

        /// Name of the service; used in logging, but does not need to match real name
        #[structopt(long, default_value = "Shawl")]
        name: String,
    },
}

#[derive(structopt::StructOpt, Debug)]
#[structopt(
    name = "shawl",
    about = "Wrap arbitrary commands as Windows services",
    setting(structopt::clap::AppSettings::SubcommandsNegateReqs)
)]
struct Cli {
    #[structopt(subcommand)]
    sub: Subcommand,
}

fn prepare_logging(console: bool) -> Result<(), Box<std::error::Error>> {
    let mut log_file = std::env::current_exe()?;
    log_file.pop();
    log_file.push("shawl.log");

    let mut loggers: Vec<Box<simplelog::SharedLogger>> = vec![
        simplelog::WriteLogger::new(
            simplelog::LevelFilter::Debug,
            simplelog::ConfigBuilder::new()
                .set_time_format_str("%Y-%m-%d %H:%M:%S")
                .build(),
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_file)?,
        )
    ];

    if console {
        loggers.push(
            simplelog::TermLogger::new(
                simplelog::LevelFilter::Info,
                simplelog::ConfigBuilder::new()
                    .set_time_format_str("")
                    .build(),
                simplelog::TerminalMode::default(),
            ).expect("Unable to create terminal logger")
        );
    }

    simplelog::CombinedLogger::init(loggers)?;

    Ok(())
}

fn add_service(name: String, opts: CommonOpts) -> Result<(), ()> {
    let shawl_path = std::env::current_exe().expect("Unable to determine Shawl location");
    let mut shawl_args = vec![
        "run".to_string(),
        "--name".to_string(),
        name.clone(),
        "--stop-timeout".to_string(),
        opts.stop_timeout.to_string(),
    ];
    if opts.restart_ok {
        shawl_args.push("--restart-ok".to_string());
    }
    if opts.no_restart_err {
        shawl_args.push("--no-restart-err".to_string());
    }

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

#[cfg(windows)]
fn main() -> Result<(), Box<std::error::Error>> {
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
            },
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
    Success(i32),
    Failure(i32),
    Terminated,
}

fn check_process(child: &mut std::process::Child) -> Result<ProcessStatus, Box<std::error::Error>> {
    match child.try_wait() {
        Ok(None) => Ok(ProcessStatus::Running),
        Ok(Some(status)) => match status.code() {
            Some(0) => Ok(ProcessStatus::Success(0)),
            Some(code) => Ok(ProcessStatus::Failure(code)),
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
                                Ok(crate::ProcessStatus::Success(code))
                                | Ok(crate::ProcessStatus::Failure(code)) => {
                                    info!(
                                        "Command exited after {:?} ms with code {:?}",
                                        start_time.elapsed().as_millis(),
                                        code
                                    );
                                    service_exit_code = match code {
                                        0 => ServiceExitCode::NO_ERROR,
                                        x => ServiceExitCode::ServiceSpecific(x as u32),
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
                    Ok(crate::ProcessStatus::Success(_)) => {
                        info!("Command finished successfully");
                        service_exit_code = ServiceExitCode::NO_ERROR;
                        match opts.restart_ok {
                            true => break 'inner,
                            false => break 'outer,
                        }
                    }
                    Ok(crate::ProcessStatus::Failure(code)) => {
                        info!("Command failed with code {}", code);
                        service_exit_code = ServiceExitCode::ServiceSpecific(code as u32);
                        match !opts.no_restart_err {
                            true => break 'inner,
                            false => break 'outer,
                        }
                    }
                    Ok(crate::ProcessStatus::Terminated) => {
                        info!("Command was terminated by a signal");
                        service_exit_code =
                            ServiceExitCode::Win32(winapi::shared::winerror::ERROR_PROCESS_ABORTED);
                        match !opts.no_restart_err {
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
