mod cli;
mod control;
#[cfg(windows)]
mod service;

use crate::cli::{evaluate_cli, Subcommand};
use log::{debug, error};

fn prepare_logging(
    name: &str,
    log_dir: Option<&String>,
    console: bool,
    rotation: cli::LogRotation,
    retention: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut exe_dir = std::env::current_exe()?;
    exe_dir.pop();

    let mut logger = flexi_logger::Logger::with_env_or_str("debug")
        .log_to_file()
        .directory(exe_dir)
        .discriminant(format!("for_{}", name))
        .append()
        .rotate(
            match rotation {
                cli::LogRotation::Bytes(bytes) => flexi_logger::Criterion::Size(bytes),
                cli::LogRotation::Daily => flexi_logger::Criterion::Age(flexi_logger::Age::Day),
                cli::LogRotation::Hourly => flexi_logger::Criterion::Age(flexi_logger::Age::Hour),
            },
            flexi_logger::Naming::Timestamps,
            flexi_logger::Cleanup::KeepLogFiles(retention),
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
        logger = logger.o_directory(Some(dir.replace("\\\\?\\", "")));
    }

    if console {
        logger = logger.duplicate_to_stderr(flexi_logger::Duplicate::Info);
    }

    logger.start()?;
    Ok(())
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = evaluate_cli();
    let console = !matches!(cli.sub, Subcommand::Run { .. });

    let should_log = match cli.clone().sub {
        Subcommand::Add { common: opts, .. } => !opts.no_log,
        Subcommand::Run { common: opts, .. } => !opts.no_log,
    };
    if should_log {
        let (name, common) = match &cli.sub {
            Subcommand::Add { name, common, .. } | Subcommand::Run { name, common, .. } => {
                (name, common)
            }
        };
        prepare_logging(
            name,
            common.log_dir.as_ref(),
            console,
            common.log_rotate.unwrap_or_default(),
            common.log_retain.unwrap_or(2),
        )?;
    }

    debug!("********** LAUNCH **********");
    debug!("{:?}", cli);

    match cli.sub {
        Subcommand::Add {
            name,
            cwd,
            dependencies,
            common: opts,
        } => match control::add_service(name, cwd, &dependencies, opts) {
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
