use log::info;

use clap::Parser;

#[derive(clap::Parser, Debug)]
#[clap(name = "shawl-child", about = "Dummy program to test wrapping with Shawl")]
struct Cli {
    /// Run forever unless forcibly killed
    #[clap(long)]
    infinite: bool,

    /// Exit immediately with this code
    #[clap(long)]
    exit: Option<i32>,

    /// Test option, prints an extra line to stdout if received
    #[clap(long)]
    test: bool,
}

fn prepare_logging() -> Result<(), Box<dyn std::error::Error>> {
    let mut exe_dir = std::env::current_exe()?;
    exe_dir.pop();

    flexi_logger::Logger::try_with_env_or_str("debug")?
        .log_to_file(
            flexi_logger::FileSpec::default()
                .directory(exe_dir)
                .suppress_timestamp(),
        )
        .append()
        .duplicate_to_stderr(flexi_logger::Duplicate::Info)
        .format_for_files(|w, now, record| {
            write!(
                w,
                "{} [{}] {}",
                now.now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                &record.args()
            )
        })
        .format_for_stderr(|w, _now, record| write!(w, "[{}] {}", record.level(), &record.args()))
        .start()?;

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    prepare_logging()?;
    info!("********** LAUNCH **********");
    let cli = Cli::parse();
    info!("{:?}", cli);
    info!("PATH: {}", std::env::var("PATH").unwrap());
    info!("env.SHAWL_FROM_CLI: {:?}", std::env::var("SHAWL_FROM_CLI"));

    println!("shawl-child message on stdout");
    eprintln!("shawl-child message on stderr");

    if cli.test {
        println!("shawl-child test option received");
    }

    if let Some(code) = cli.exit {
        std::process::exit(code);
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
