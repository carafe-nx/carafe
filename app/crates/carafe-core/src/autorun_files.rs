//! Converting settings into the `<exe>.keys.txt` and `<exe>.wine-nx.txt` files for RomFS.

use std::fmt::Write as _;

use crate::settings::{
    AutorunSettings, Binding, CpuEmulator, Direct3d, DxvkHud, DxvkSource, InputMode, LsfgFlow,
    SyncMode, Upscaling, WindowOutput,
};

/// Autorun settings files placed in RomFS next to the game's `.exe`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutorunFiles {
    /// Name of the key mapping file: `<.exe name without extension>.keys.txt`.
    pub keys_name: String,
    /// Contents of the key mapping file.
    pub keys: String,
    /// Name of the program settings file: `<.exe name without extension>.wine-nx.txt`.
    pub program_name: String,
    /// Contents of the program settings file.
    pub program: String,
}

impl AutorunFiles {
    /// Creates both files for a game whose executable is `executable`.
    ///
    /// Every value is written explicitly, so the result does not depend on Autorun defaults.
    #[must_use]
    pub fn render(executable: &str, settings: &AutorunSettings) -> Self {
        let stem = executable_stem(executable);
        Self {
            keys_name: format!("{stem}.keys.txt"),
            keys: render_keys(settings),
            program_name: format!("{stem}.wine-nx.txt"),
            program: render_program(settings),
        }
    }
}

pub(crate) fn executable_stem(executable: &str) -> &str {
    let name = executable.rsplit(['/', '\\']).next().unwrap_or(executable);
    let has_exe = name.len() > 4 && name[name.len() - 4..].eq_ignore_ascii_case(".exe");
    if has_exe {
        &name[..name.len() - 4]
    } else {
        name
    }
}

fn render_keys(settings: &AutorunSettings) -> String {
    let input = &settings.input;
    let mut out = String::new();
    let mode = match input.mode {
        InputMode::Controller => 1,
        InputMode::KeyboardMouse => 2,
    };
    line(&mut out, "input-mode", mode);
    line(&mut out, "left-deadzone", input.left_deadzone);
    line(&mut out, "right-deadzone", input.right_deadzone);
    line(&mut out, "keyboard-auto", flag(input.keyboard_auto));
    for binding in &input.keys {
        line(
            &mut out,
            &format!("key.{}", binding.button.autorun_name()),
            chord(binding),
        );
    }
    for binding in &input.pad {
        line(
            &mut out,
            &format!("pad.{}", binding.button.autorun_name()),
            chord(binding),
        );
    }
    out
}

fn render_program(settings: &AutorunSettings) -> String {
    let graphics = &settings.graphics;
    let system = &settings.system;
    let mut out = String::new();
    let sync = match system.sync {
        SyncMode::Standard => "standard",
        SyncMode::Horizon => "horizon",
    };
    line(&mut out, "sync", sync);
    let cpu = match system.cpu {
        CpuEmulator::Fex => "fex",
        CpuEmulator::Box64 => "box64",
    };
    line(&mut out, "cpu", cpu);
    line(&mut out, "four-cores", flag(system.four_cores));
    match system.windows {
        WindowOutput::Auto => {}
        WindowOutput::Framebuffer => line(&mut out, "windows", "framebuffer"),
        WindowOutput::Compositor => line(&mut out, "windows", "compositor"),
    }
    match graphics.direct3d {
        Direct3d::Dxvk => {}
        Direct3d::DxvkVkd3d => line(&mut out, "d3d", "dxvk+vkd3d"),
        Direct3d::Wine => line(&mut out, "d3d", "wine"),
    }
    match graphics.dxvk_source {
        DxvkSource::Official => {}
        DxvkSource::Sarek => line(&mut out, "dxvk-source", "sarek"),
        DxvkSource::Gplasync => line(&mut out, "dxvk-source", "gplasync"),
    }
    let hud = match graphics.hud {
        DxvkHud::Off => "0",
        DxvkHud::Fps => "fps",
        DxvkHud::Compact => "api,fps,frametimes",
        DxvkHud::Full => "version,api,devinfo,fps,memory,frametimes,compiler",
    };
    line(&mut out, "dxvk-hud", hud);
    if graphics.frame_limit != 0 {
        line(&mut out, "frame-limit", graphics.frame_limit);
    }
    line(&mut out, "vsync", flag(graphics.vsync));
    line(&mut out, "lsfg", flag(graphics.lsfg));
    line(
        &mut out,
        "lsfg-performance",
        flag(graphics.lsfg_performance),
    );
    let flow = match graphics.lsfg_flow {
        LsfgFlow::Eighth => "0.125",
        LsfgFlow::Quarter => "0.25",
        LsfgFlow::Half => "0.5",
    };
    line(&mut out, "lsfg-flow", flow);
    let upscaling = match graphics.upscaling {
        Upscaling::Off => "off",
        Upscaling::Fsr => "fsr",
        Upscaling::Integer => "integer",
    };
    line(&mut out, "upscaling", upscaling);
    line(
        &mut out,
        "upscaling-sharpness",
        format!("{}%", graphics.sharpness),
    );
    line(&mut out, "verbose", flag(settings.debug.verbose));
    line(&mut out, "profile", flag(settings.debug.profile));
    out
}

fn line(out: &mut String, key: &str, value: impl std::fmt::Display) {
    let _ = writeln!(out, "{key} = {value}");
}

fn flag(on: bool) -> u8 {
    u8::from(on)
}

fn chord(binding: &Binding) -> String {
    if binding.codes.is_empty() {
        return "none".to_owned();
    }
    binding
        .codes
        .iter()
        .map(|code| format!("0x{code:x}"))
        .collect::<Vec<_>>()
        .join("+")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::SwitchButton;

    fn value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
        text.lines()
            .find_map(|l| l.strip_prefix(&format!("{key} = ")))
    }

    #[test]
    fn names_files_after_executable() {
        let files = AutorunFiles::render("bin\\OpenTTD.EXE", &AutorunSettings::default());
        assert_eq!(files.keys_name, "OpenTTD.keys.txt");
        assert_eq!(files.program_name, "OpenTTD.wine-nx.txt");
    }

    #[test]
    fn writes_keyboard_mode_and_chords() {
        let mut settings = AutorunSettings::default();
        settings.input.mode = InputMode::KeyboardMouse;
        settings.input.keys[0] = Binding {
            button: SwitchButton::A,
            codes: vec![0x11, 0x43],
        };
        settings.input.keys[1].codes.clear();
        let keys = AutorunFiles::render("game.exe", &settings).keys;
        assert_eq!(value(&keys, "input-mode"), Some("2"));
        assert_eq!(value(&keys, "key.A"), Some("0x11+0x43"));
        assert_eq!(value(&keys, "key.B"), Some("none"));
        assert_eq!(value(&keys, "pad.ZL"), Some("0x7"));
    }

    #[test]
    fn plain_dxvk_leaves_d3d_unset() {
        let program = AutorunFiles::render("game.exe", &AutorunSettings::default()).program;
        assert_eq!(value(&program, "d3d"), None);
        let mut settings = AutorunSettings::default();
        settings.graphics.direct3d = Direct3d::DxvkVkd3d;
        let program = AutorunFiles::render("game.exe", &settings).program;
        assert_eq!(value(&program, "d3d"), Some("dxvk+vkd3d"));
    }

    #[test]
    fn sharpness_carries_percent_sign() {
        let program = AutorunFiles::render("game.exe", &AutorunSettings::default()).program;
        assert_eq!(value(&program, "upscaling-sharpness"), Some("40%"));
    }

    #[test]
    fn horizon_sync_is_written() {
        let mut settings = AutorunSettings::default();
        settings.system.sync = SyncMode::Horizon;
        let program = AutorunFiles::render("game.exe", &settings).program;
        assert_eq!(value(&program, "sync"), Some("horizon"));
    }
}
