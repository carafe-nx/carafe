//! Extracts the icon from an `.exe` and saves it to a file — for checking on real games.
//!
//! ```text
//! cargo run -p carafe-images --example exe_icon -- <game.exe> <where to save>
//! ```

use std::process::ExitCode;

use carafe_core::ports::ImageReader;
use carafe_images::FileImageReader;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [executable, out] = args.as_slice() else {
        eprintln!("usage: exe_icon <game.exe> <output file>");
        return ExitCode::FAILURE;
    };
    match FileImageReader.executable_icon(executable) {
        Ok(Some(image)) => {
            if let Err(error) = std::fs::write(out, &image.bytes) {
                eprintln!("{out}: {error}");
                return ExitCode::FAILURE;
            }
            println!("{}: {} bytes → {out}", image.mime, image.bytes.len());
            ExitCode::SUCCESS
        }
        Ok(None) => {
            println!("no icon");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
