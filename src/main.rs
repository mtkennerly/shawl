use log::info;
use structopt::StructOpt;

#[derive(structopt::StructOpt, Debug)]
#[structopt(name = "shawl", about = "Wrap arbitrary commands as Windows services")]
struct Cli {
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

fn prepare_logging() -> Result<(), Box<std::error::Error>> {
    let mut log_file = std::env::current_exe()?;
    log_file.pop();
    log_file.push("shawl.log");

    simplelog::WriteLogger::init(
        simplelog::LevelFilter::Debug,
        simplelog::ConfigBuilder::new()
            .set_time_format_str("%Y-%m-%d %H:%M:%S")
            .build(),
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)?,
    )?;

    Ok(())
}

#[cfg(windows)]
fn main() -> windows_service::Result<()> {
    let _ = prepare_logging();
    info!("********** LAUNCH **********");
    info!("{:?}", Cli::from_args());
    service::run()
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
    use log::{error, info};
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

    const SERVICE_NAME: &str = "shawl-svc";
    const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

    define_windows_service!(ffi_service_main, service_main);

    pub fn run() -> windows_service::Result<()> {
        service_dispatcher::start(SERVICE_NAME, ffi_service_main)
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

        let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

        status_handle.set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::NO_ERROR,
            checkpoint: 0,
            wait_hint: std::time::Duration::default(),
        })?;

        info!("Entering main service loop");
        'outer: loop {
            info!("Launching command");
            let mut child = match std::process::Command::new(&cli.command[0])
                .args(&cli.command[1..])
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
                            wait_hint: std::time::Duration::from_millis(cli.stop_timeout + 1000),
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
                                    if start_time.elapsed().as_millis() < cli.stop_timeout.into() {
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
                        match cli.restart_ok {
                            true => break 'inner,
                            false => break 'outer,
                        }
                    }
                    Ok(crate::ProcessStatus::Failure(code)) => {
                        info!("Command failed with code {}", code);
                        service_exit_code = ServiceExitCode::ServiceSpecific(code as u32);
                        match !cli.no_restart_err {
                            true => break 'inner,
                            false => break 'outer,
                        }
                    }
                    Ok(crate::ProcessStatus::Terminated) => {
                        info!("Command was terminated by a signal");
                        service_exit_code =
                            ServiceExitCode::Win32(winapi::shared::winerror::ERROR_PROCESS_ABORTED);
                        match !cli.no_restart_err {
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
        info!("Exited main service loop");

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
