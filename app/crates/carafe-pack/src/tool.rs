//! Running hacBrewPack and reading its output.

use std::ffi::OsString;
use std::io::{BufRead, BufReader, ErrorKind, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;

use carafe_core::ports::AdapterError;

/// Runs hacBrewPack with the arguments `args` and waits for it to finish.
///
/// `on_line` receives every non-empty output line: first stdout as it appears, then stderr.
/// Lines in which hacBrewPack prints a key value arrive with a mask instead of the value.
///
/// # Errors
///
/// [`AdapterError::NotFound`] if the program does not exist; [`AdapterError::Tool`] if it failed to start
/// or exited with an error — the text then holds the last stderr line.
pub fn run(
    tool: &Path,
    args: &[OsString],
    on_line: &mut dyn FnMut(&str),
) -> Result<(), AdapterError> {
    let mut command = Command::new(tool);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console(&mut command);
    let mut child = command.spawn().map_err(|error| {
        let message = format!("{}: {error}", tool.display());
        if error.kind() == ErrorKind::NotFound {
            AdapterError::NotFound(message)
        } else {
            AdapterError::Tool(message)
        }
    })?;
    let stderr = child.stderr.take();
    let errors = thread::spawn(move || {
        let mut text = Vec::new();
        if let Some(mut stderr) = stderr {
            let _ = stderr.read_to_end(&mut text);
        }
        String::from_utf8_lossy(&text).into_owned()
    });
    if let Some(stdout) = child.stdout.take() {
        let mut reader = BufReader::new(stdout);
        let mut line = Vec::new();
        while reader.read_until(b'\n', &mut line).unwrap_or(0) > 0 {
            emit(&String::from_utf8_lossy(&line), on_line);
            line.clear();
        }
    }
    let status = child
        .wait()
        .map_err(|error| AdapterError::Tool(format!("hacBrewPack: {error}")))?;
    let errors = errors.join().unwrap_or_default();
    for line in errors.lines() {
        emit(line, on_line);
    }
    if status.success() {
        return Ok(());
    }
    let last = errors
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(redact)
        .unwrap_or_default();
    Err(AdapterError::Tool(format!("hacBrewPack {status}: {last}")))
}

fn emit(line: &str, on_line: &mut dyn FnMut(&str)) {
    let line = line.trim_end();
    if !line.trim().is_empty() {
        on_line(&redact(line));
    }
}

fn redact(line: &str) -> String {
    match line
        .strip_prefix("Key (")
        .and_then(|rest| rest.split_once(')'))
    {
        Some((_, rest)) => format!("Key (…){rest}"),
        None => line.to_owned(),
    }
}

#[cfg(target_os = "windows")]
fn hide_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
fn hide_console(_command: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_values_are_masked() {
        assert_eq!(
            redact("Key (0123abcd) must be 32 hex digits!"),
            "Key (…) must be 32 hex digits!"
        );
        assert_eq!(redact("Writing a to b"), "Writing a to b");
    }

    #[test]
    fn a_missing_tool_is_not_found() {
        let result = run(Path::new("Z:/carafe/missing/hacbrewpack"), &[], &mut |_| {});
        assert!(matches!(result, Err(AdapterError::NotFound(_))));
    }
}
