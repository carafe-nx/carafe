//! Switch over MTP without the window — for checking on the console.
//!
//! ```text
//! cargo run -p carafe-mtp --example switch -- status
//! cargo run -p carafe-mtp --example switch -- logs <Title ID> <folder>
//! cargo run -p carafe-mtp --example switch -- install <file.nsp> sd|nand
//! ```

use std::process::ExitCode;

use carafe_core::TitleId;
use carafe_core::dbi::InstallTarget;
use carafe_core::ports::{Device, TransferProgress};
use carafe_mtp::{MtpDevice, storage_names};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["status"] => {
            println!("{:?}", MtpDevice.status());
            storage_names().map(|names| {
                for name in names {
                    println!("storage: {name}");
                }
            })
        }
        ["logs", title_id, dest] => match TitleId::parse(title_id) {
            Ok(id) => MtpDevice
                .fetch_logs(id, dest)
                .map(|count| println!("{count} files → {dest}")),
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::FAILURE;
            }
        },
        ["install", nsp, target] => {
            let target = match *target {
                "sd" => InstallTarget::Sd,
                "nand" => InstallTarget::Nand,
                _ => {
                    eprintln!("target must be sd or nand");
                    return ExitCode::FAILURE;
                }
            };
            let mut print = |progress: TransferProgress| {
                println!("{} / {}", progress.done_bytes, progress.total_bytes);
            };
            MtpDevice.install(nsp, target, &mut print)
        }
        _ => {
            eprintln!("usage: switch status | logs <title id> <folder> | install <nsp> sd|nand");
            return ExitCode::FAILURE;
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
