use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use log::{debug, error};

/// Parses an RGB consignment (Transfer / Contract / Kit) and writes a JSON
/// summary to stdout. With no path or `-`, reads bytes from stdin.
#[derive(Parser)]
#[command(name = "rgb-consignment", version, about, long_about = None)]
struct Cli {
    /// Path to a `.rgb` consignment file. Use `-` (or omit) to read stdin.
    file: Option<PathBuf>,

    /// Emit compact JSON instead of pretty-printed.
    #[arg(short, long)]
    compact: bool,
}

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    let bytes = match cli.file.as_deref() {
        None => {
            debug!("reading consignment from stdin");
            match read_stdin() {
                Ok(b) => b,
                Err(e) => {
                    error!("read stdin: {e}");
                    return ExitCode::FAILURE;
                }
            }
        }
        Some(p) if p.as_os_str() == "-" => {
            debug!("reading consignment from stdin");
            match read_stdin() {
                Ok(b) => b,
                Err(e) => {
                    error!("read stdin: {e}");
                    return ExitCode::FAILURE;
                }
            }
        }
        Some(p) => {
            debug!("reading consignment from {}", p.display());
            match std::fs::read(p) {
                Ok(b) => b,
                Err(e) => {
                    error!("read {}: {e}", p.display());
                    return ExitCode::FAILURE;
                }
            }
        }
    };

    debug!("parsing {} bytes", bytes.len());
    let info = match rgb_consignment::parse(&bytes) {
        Ok(i) => i,
        Err(e) => {
            error!("{e}");
            return ExitCode::FAILURE;
        }
    };

    let json = if cli.compact {
        serde_json::to_string(&info)
    } else {
        serde_json::to_string_pretty(&info)
    };
    match json {
        Ok(s) => {
            println!("{s}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            error!("serialize: {e}");
            ExitCode::FAILURE
        }
    }
}

fn read_stdin() -> std::io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    std::io::stdin().lock().read_to_end(&mut buf)?;
    Ok(buf)
}
