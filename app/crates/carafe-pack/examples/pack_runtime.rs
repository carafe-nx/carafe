//! Builds the runtime archive from a folder in the `out/runtime-test` layout; the DLLs are first checked
//! against Autorun's `horizon-dlls/manifest.json`.
//!
//! ```text
//! cargo run --release -p carafe-pack --example pack_runtime -- <runtime folder> <archive file>
//! ```

use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use carafe_pack::archive::write_archive;
use carafe_pack::verify::verify_runtime;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [runtime, out] = args.as_slice() else {
        eprintln!("usage: pack_runtime <runtime folder> <archive file>");
        return ExitCode::FAILURE;
    };
    let started = Instant::now();
    let runtime = Path::new(runtime);
    match verify_runtime(&runtime.join("romfs"), &mut |_| {}) {
        Ok(count) => println!("{count} DLL match horizon-dlls/manifest.json"),
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    }
    let mut added = 0usize;
    let result = write_archive(runtime, Path::new(out), &mut |_| {
        added += 1;
        if added.is_multiple_of(200) {
            println!("{added} files");
        }
    });
    match result {
        Ok(count) => {
            let size = std::fs::metadata(out).map(|meta| meta.len()).unwrap_or(0);
            println!(
                "{count} files → {out} ({size} bytes, {:.0} s)",
                started.elapsed().as_secs_f64()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
