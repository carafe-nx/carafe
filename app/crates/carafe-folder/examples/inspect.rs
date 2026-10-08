//! Analyzes a game folder and prints what it finds — for checking on real games.
//!
//! ```text
//! cargo run -p carafe-folder --example inspect -- <game folder>
//! ```

use std::process::ExitCode;

use carafe_core::ports::FolderInspector;
use carafe_folder::DiskFolderInspector;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [folder] = args.as_slice() else {
        eprintln!("usage: inspect <game folder>");
        return ExitCode::FAILURE;
    };
    match DiskFolderInspector.inspect(folder) {
        Ok(report) => {
            for exe in &report.executables {
                println!(
                    "{:?} {:>12} {} | product {:?} | company {:?}",
                    exe.arch, exe.size_bytes, exe.path, exe.product_name, exe.company_name
                );
            }
            println!("steam_api: {}", report.has_steam_api);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
