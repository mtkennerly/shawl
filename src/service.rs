use crate::cli;
use crate::process_job::ProcessJob;
use log::{debug, error, info};
use std::{io::BufRead, os::windows::process::CommandExt};
use windows_service::{
    define_windows_service,
    service::{ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType},
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};

const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

define_windows_service!(ffi_service_main, service_main);

enum ProcessStatus {
    Running,
    Exited(i32),
    Terminated,
}

fn check_process(child: &mut std::process::Child) -> Result<ProcessStatus, Box<dyn std::error::Error>> {
    match child.try_wait() {
        Ok(None) => Ok(ProcessStatus::Running),
        Ok(Some(status)) => match status.code() {
            Some(code) => Ok(ProcessStatus::Exited(code)),
            None => Ok(ProcessStatus::Terminated),
        },
        Err(e) => Err(Box::new(e)),
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

pub fn run(name: String) -> windows_service::Result<()> {
    service_dispatcher::start(name, ffi_service_main)
}

fn service_main(mut arguments: Vec<std::ffi::OsString>) {
    unsafe {
        // Windows services don't start with a console, so we have to
        // allocate one in order to send ctrl-C to children.
        if windows::Win32::System::Console::AllocConsole().is_err() {
            error!(
                "Windows AllocConsole failed with code {:?}",
                windows::Win32::Foundation::GetLastError()
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
    let cli = cli::evaluate_cli();
    let (name, cwd, opts) = match cli.sub {
        cli::Subcommand::Run {
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

    let priority = match opts.priority {
        Some(x) => x.to_windows().0,
        None => windows::Win32::System::Threading::INHERIT_CALLER_PRIORITY.0,
    };

    let mut restart_after: Option<std::time::Instant> = None;

    // Create a process job that kills all child processes when closed (if kill_process_tree is enabled)
    let mut process_job: Option<ProcessJob> = if opts.kill_process_tree {
        match ProcessJob::create_kill_on_close() {
            Ok(pj) => {
                info!("Created process job for process group management");
                Some(pj)
            }
            Err(e) => {
                error!("Failed to create process job: {:?}", e);
                None
            }
        }
    } else {
        None
    };

    debug!("Entering main service loop");
    'outer: loop {
        if let Some(delay) = restart_after {
            match shutdown_rx.recv_timeout(std::time::Duration::from_millis(1)) {
                Ok(_) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    info!("Cancelling before launch");
                    break 'outer;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => (),
            };

            let now = std::time::Instant::now();
            if now < delay {
                let step = (delay - now).min(std::time::Duration::from_millis(50));
                debug!("Sleeping another {} ms", step.as_millis());
                std::thread::sleep(step);
                continue;
            } else {
                info!("Restart delay is complete");
                restart_after = None;
            }
        }

        info!("Launching command");
        let should_log_cmd = !&opts.no_log_cmd;
        let mut child_cmd = std::process::Command::new(&program);
        let mut path_env = std::env::var("PATH").ok();

        child_cmd
            .args(&args)
            .creation_flags(priority)
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
            let simplified: Vec<_> = opts.path.iter().map(|x| crate::simplify_path(x)).collect();
            path_env = match path_env {
                Some(path) => Some(format!("{};{}", path, simplified.join(";"))),
                None => Some(simplified.join(";").to_string()),
            };
        }
        if !opts.path_prepend.is_empty() {
            let simplified: Vec<_> = opts.path_prepend.iter().map(|x| crate::simplify_path(x)).collect();
            path_env = match path_env {
                Some(path) => Some(format!("{};{}", simplified.join(";"), path)),
                None => Some(simplified.join(";").to_string()),
            };
        }
        if let Some(active_cwd) = &cwd {
            let active_cwd = crate::simplify_path(active_cwd);
            child_cmd.current_dir(&active_cwd);
            path_env = match path_env {
                Some(path) => Some(format!("{};{}", path, active_cwd)),
                None => Some(active_cwd),
            };
        }
        if let Some(path_env) = path_env {
            child_cmd.env("PATH", path_env);
        }

        let mut child = match child_cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                error!("Unable to launch command: {}", e);
                service_exit_code = match e.raw_os_error() {
                    Some(win_code) => ServiceExitCode::Win32(win_code as u32),
                    None => ServiceExitCode::Win32(windows::Win32::Foundation::ERROR_PROCESS_ABORTED.0),
                };
                break;
            }
        };

        // Assign process to job (if kill_process_tree is enabled)
        if let Some(ref pj) = process_job {
            if let Err(e) = pj.assign(&child) {
                error!("Failed to assign process to job: {:?}", e);
            } else {
                debug!("Assigned process (PID: {}) to job", child.id());
            }
        }

        // Log stdout.
        let output_logs_need_target = opts.log_cmd_as.is_some();
        let stdout_option = child.stdout.take();
        let stdout_logger = std::thread::spawn(move || {
            if !should_log_cmd {
                return;
            }
            if let Some(stdout) = stdout_option {
                std::io::BufReader::new(stdout).lines().for_each(|line| match line {
                    Ok(ref x) if !x.is_empty() => {
                        if output_logs_need_target {
                            debug!(target: "{shawl-cmd}", "{}", x);
                        } else {
                            debug!("stdout: {:?}", x);
                        }
                    }
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
                std::io::BufReader::new(stderr).lines().for_each(|line| match line {
                    Ok(ref x) if !x.is_empty() => {
                        if output_logs_need_target {
                            debug!(target: "{shawl-cmd}", "{}", x);
                        } else {
                            debug!("stderr: {:?}", x);
                        }
                    }
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
                        wait_hint: std::time::Duration::from_millis(opts.stop_timeout.unwrap_or(3000) + 1000),
                        process_id: None,
                    })?;

                    ignore_ctrlc.store(true, std::sync::atomic::Ordering::SeqCst);
                    info!("Sending ctrl-C to command");
                    unsafe {
                        if windows::Win32::System::Console::GenerateConsoleCtrlEvent(
                            windows::Win32::System::Console::CTRL_C_EVENT,
                            0,
                        )
                        .is_err()
                        {
                            error!(
                                "Windows GenerateConsoleCtrlEvent failed with code {:?}",
                                windows::Win32::Foundation::GetLastError()
                            );
                        };
                    }

                    let start_time = std::time::Instant::now();
                    loop {
                        match check_process(&mut child) {
                            Ok(ProcessStatus::Running) => {
                                if start_time.elapsed().as_millis() < (*stop_timeout).into() {
                                    std::thread::sleep(std::time::Duration::from_millis(50))
                                } else {
                                    info!("Killing command because stop timeout expired");
                                    if let Some(pj) = process_job.take() {
                                        // Drop the job, which will terminate all child processes
                                        info!("Dropping process job to terminate all child processes");
                                        drop(pj);
                                    } else {
                                        // Fallback to standard kill
                                        let _ = child.kill();
                                    }
                                    service_exit_code = ServiceExitCode::NO_ERROR;
                                    break;
                                }
                            }
                            Ok(ProcessStatus::Exited(code)) => {
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

            match check_process(&mut child) {
                Ok(ProcessStatus::Running) => (),
                Ok(ProcessStatus::Exited(code)) => {
                    info!("Command exited with code {:?}", code);
                    service_exit_code = if pass.contains(&code) {
                        ServiceExitCode::NO_ERROR
                    } else {
                        ServiceExitCode::ServiceSpecific(code as u32)
                    };
                    if should_restart_exited_command(
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
                Ok(ProcessStatus::Terminated) => {
                    info!("Command was terminated by a signal");
                    service_exit_code = ServiceExitCode::Win32(windows::Win32::Foundation::ERROR_PROCESS_ABORTED.0);
                    if should_restart_terminated_command(opts.restart, opts.no_restart) {
                        break 'inner;
                    } else {
                        break 'outer;
                    }
                }
                Err(e) => {
                    info!("Error trying to determine command status: {:?}", e);
                    service_exit_code = ServiceExitCode::Win32(windows::Win32::Foundation::ERROR_PROCESS_ABORTED.0);
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

        if let Some(delay) = opts.restart_delay {
            info!("Delaying {delay} ms before restart");
            restart_after = Some(std::time::Instant::now() + std::time::Duration::from_millis(delay));
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

#[cfg(test)]
speculate::speculate! {
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

    describe "process_job" {
        it "can create a process job" {
            assert!(ProcessJob::create_kill_on_close().is_ok());
        }

        it "kills the assigned process when the job is dropped" {
            use std::{thread, time::Duration};

            // Create job
            let job = ProcessJob::create_kill_on_close().unwrap();

            // Spawn long-running dummy command
            let mut child = std::process::Command::new("cmd")
                .args(&["/C", "timeout", "/t", "60", "/nobreak"])
                .spawn()
                .unwrap();

            // Assign to job
            assert!(job.assign(&child).is_ok());

            // Drop the job → should terminate the child
            drop(job);

            // Give Windows a small time window to process the kill
            thread::sleep(Duration::from_millis(150));

            // Child must be dead
            let status = child.try_wait()
                .expect("Failed to poll child process status");

            assert!(
                status.is_some(),
                "Child process should have been terminated when ProcessJob was dropped"
            );
        }

        it "kills child and grandchild processes when job is dropped" {
            use std::{thread, time::Duration};
            use sysinfo::{System, Pid};

            let job = ProcessJob::create_kill_on_close().unwrap();

            // Parent process spawns a grandchild
            let child = std::process::Command::new("powershell")
                .arg("-NoProfile")
                .arg("-Command")
                .arg("Start-Process powershell -ArgumentList '-NoProfile','-Command','Start-Sleep 60'; Start-Sleep 60")
                .spawn()
                .expect("Failed to spawn parent process");

            job.assign(&child).expect("Failed to assign job");

            let parent_pid = child.id();

            // Let grandchildren spawn
            thread::sleep(Duration::from_millis(300));

            let mut system = System::new_all();
            system.refresh_all();

            // Find grandchildren (children of the parent PID)
            let grandchildren: Vec<u32> = system
                .processes()
                .iter()
                .filter(|(_, p)| p.parent() == Some(Pid::from_u32(parent_pid)))
                .map(|(pid, _)| pid.as_u32())
                .collect();

            assert!(
                !grandchildren.is_empty(),
                "Expected parent to spawn at least one grandchild"
            );

            // Drop job -> kill whole tree
            drop(job);

            thread::sleep(Duration::from_millis(300));
            system.refresh_all();

            // Parent should be dead
            let parent_alive = system.process(Pid::from_u32(parent_pid)).is_some();
            assert!(!parent_alive, "Parent should be terminated");

            // Grandchildren should be dead too
            for gc_pid in grandchildren {
                let alive = system.process(Pid::from_u32(gc_pid)).is_some();
                assert!(
                    !alive,
                    "Grandchild process {} should also be terminated",
                    gc_pid
                );
            }
        }
    }
}
