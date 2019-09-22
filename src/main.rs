use log::{info};
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

#[cfg(windows)]
mod service {
    use log::{info};
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
        let _ = run_service();
    }

    pub fn run_service() -> windows_service::Result<()> {
        let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();
        let cli = crate::Cli::from_args();

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
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: std::time::Duration::default(),
        })?;

        info!("Entering main service loop");
        'outer: loop {
            info!("Launching command");
            let child = std::process::Command::new(&cli.command[0])
                .args(&cli.command[1..])
                .spawn();
            let mut child_handle = match child {
                Ok(c) => c,
                Err(_) => break,
            };

            'inner: loop {
                match shutdown_rx.recv_timeout(std::time::Duration::from_secs(1)) {
                    Ok(_) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        info!("Killing command in response to stop event");
                        let _ = child_handle.kill();
                        break 'outer;
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => (),
                };

                match child_handle.try_wait() {
                    Ok(Some(status)) => match status.code() {
                        Some(0) => {
                            info!("Command finished successfully");
                            match cli.restart_ok {
                                true => break 'inner,
                                false => break 'outer,
                            }
                        },
                        Some(code) => {
                            info!("Command failed with code {}", code);
                            match !cli.no_restart_err {
                                true => break 'inner,
                                false => break 'outer,
                            }
                        },
                        None => {
                            info!("Command was terminated by a signal");
                            match !cli.no_restart_err {
                                true => break 'inner,
                                false => break 'outer,
                            }
                        },
                    },
                    Ok(None) => (),
                    Err(e) => {
                        info!("Error trying to determine command status: {:?}", e);
                        break 'inner;
                    },
                }
            }
        }
        info!("Exited main service loop");

        status_handle.set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: std::time::Duration::default(),
        })?;

        Ok(())
    }
}
