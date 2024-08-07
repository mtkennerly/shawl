use std::collections::HashMap;

use log::info;
use windows::core::s;

use clap::Parser;

#[derive(clap::Parser, Debug)]
#[clap(
    name = "shawl-child",
    about = "Dummy program to test wrapping with Shawl"
)]
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

#[allow(unused)]
type DeviceAudio = HashMap<String, Audio>;

#[allow(unused)]
#[derive(Debug)]
struct Audio {
    volume: f32,
    peak: f32,
}

#[allow(unused)]
fn get_audio() -> windows::core::Result<DeviceAudio> {
    use windows::Win32::{
        Media::Audio::{
            eCapture, eCommunications, eConsole, eMultimedia, eRender,
            Endpoints::{IAudioEndpointVolume, IAudioMeterInformation},
            IMMDeviceEnumerator, MMDeviceEnumerator,
        },
        System::Com::{
            CoCreateInstance, CoInitializeEx, CLSCTX_ALL, CLSCTX_INPROC_SERVER,
            COINIT_APARTMENTTHREADED,
        },
    };

    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let device_enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER)?;

        let mut out = HashMap::<String, Audio>::new();

        for (label, dataflow, role) in [
            ("output-console", eRender, eConsole),
            ("output-multimedia", eRender, eMultimedia),
            ("output-communications", eRender, eCommunications),
            ("input-console", eCapture, eConsole),
            ("input-multimedia", eCapture, eMultimedia),
            ("input-communications", eCapture, eCommunications),
        ] {
            let device = device_enumerator.GetDefaultAudioEndpoint(dataflow, role)?;

            let endpoint_volume: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;
            let volume = endpoint_volume.GetMasterVolumeLevel()?;

            let meter: IAudioMeterInformation = device.Activate(CLSCTX_ALL, None)?;
            let peak = meter.GetPeakValue()?;

            out.insert(label.to_string(), Audio { volume, peak });
        }

        Ok(out)
    }
}

unsafe fn mci_send_string(label: &str, input: windows::core::PCSTR) -> Result<(), String> {
    use windows::Win32::Media::Multimedia::mciSendStringA;

    let mut buffer = [0; 12];

    let code = mciSendStringA(
        input,
        Some(&mut buffer),
        windows::Win32::Foundation::HWND::default(),
    );

    let message = String::from_utf8(buffer.to_vec());

    log::info!("[mci-{label}] code: {code}, message: {message:?}");
    if code == 0 {
        Ok(())
    } else {
        Err(format!("[mci-{label}] code: {code}"))
    }
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

    unsafe {
        mci_send_string("open", s!("open new type waveaudio alias arbitrary"))?;
        mci_send_string("set", s!("set arbitrary bitspersample 16 channels 1 alignment 2 samplespersec 22050 format tag pcm wait"))?;
    }

    while running.load(std::sync::atomic::Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_millis(250));
        unsafe {
            mci_send_string("status", s!("status arbitrary level"))?;
        }
    }

    unsafe {
        mci_send_string("close", s!("close arbitrary"))?;
    }

    info!("End");
    Ok(())
}
