use log::info;

use structopt::StructOpt;

#[derive(structopt::StructOpt, Debug)]
#[structopt(
    name = "shawl-child",
    about = "Dummy program to test wrapping with Shawl"
)]
struct Cli {
    /// Run forever unless forcibly killed
    #[structopt(long)]
    infinite: bool,

    /// Exit immediately with this code
    #[structopt(long)]
    exit: Option<i32>,
}

fn prepare_logging() -> Result<(), Box<std::error::Error>> {
    let mut log_file = std::env::current_exe()?;
    log_file.pop();
    log_file.push("shawl-child.log");

    simplelog::CombinedLogger::init(vec![
        simplelog::TermLogger::new(
            simplelog::LevelFilter::Debug,
            simplelog::ConfigBuilder::new()
                .set_time_format_str("%Y-%m-%d %H:%M:%S")
                .build(),
            simplelog::TerminalMode::default(),
        )
        .expect("Unable to create terminal logger"),
        simplelog::WriteLogger::new(
            simplelog::LevelFilter::Debug,
            simplelog::ConfigBuilder::new()
                .set_time_format_str("%Y-%m-%d %H:%M:%S")
                .build(),
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_file)?,
        ),
    ])?;

    Ok(())
}

fn main() -> Result<(), Box<std::error::Error>> {
    prepare_logging()?;
    info!("********** LAUNCH **********");
    let cli = Cli::from_args();
    info!("{:?}", cli);

    match cli.exit {
        Some(code) => std::process::exit(code),
        None => (),
    }

    let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let running2 = running.clone();

    ctrlc::set_handler(move || {
        if cli.infinite {
            info!("Ignoring ctrl-C");
        } else {
            running2.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    })?;

    while running.load(std::sync::atomic::Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_millis(500));
        info!("Looping!");
    }

    info!("End");
    Ok(())
}
