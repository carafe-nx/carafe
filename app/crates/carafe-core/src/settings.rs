//! Autorun runtime settings: input, graphics, system, debugging.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

/// All Autorun runtime settings that Carafe puts into the NSP.
///
/// The defaults match Autorun's behavior when there is no settings file.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AutorunSettings {
    /// Input and key mapping.
    pub input: InputSettings,
    /// Image output.
    pub graphics: GraphicsSettings,
    /// CPU, synchronization, windows.
    pub system: SystemSettings,
    /// Debugging.
    pub debug: DebugSettings,
}

impl AutorunSettings {
    /// Checks the consistency of all groups.
    ///
    /// # Errors
    ///
    /// The first [`SettingsError`] found.
    pub fn validate(&self) -> Result<(), SettingsError> {
        self.input.validate()?;
        self.graphics.validate()
    }
}

/// Error in the Autorun settings.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SettingsError {
    /// Stick dead zone outside 0…25.
    #[error("dead zone {0} is outside the range 0…25")]
    Deadzone(u8),
    /// A button has more than [`CHORD_MAX`] codes, a repeated code or an invalid code.
    #[error("invalid binding for button {0:?}")]
    Binding(SwitchButton),
    /// The mapping contains a button twice.
    #[error("button {0:?} is bound twice")]
    DuplicateButton(SwitchButton),
    /// Frame limit not from the Autorun list.
    #[error("frame limit {0} is not supported")]
    FrameLimit(u16),
    /// Sharpness is not a multiple of 20 or is greater than 100.
    #[error("sharpness {0} is not one of 0, 20, …, 100")]
    Sharpness(u8),
    /// DXVK Sarek is incompatible with VKD3D.
    #[error("the DXVK Sarek build is incompatible with VKD3D")]
    SarekWithVkd3d,
}

/// Maximum number of codes in one chord.
pub const CHORD_MAX: usize = 4;
/// Number of targets in the controller mapping (Xbox buttons and axes).
pub const PAD_TARGET_COUNT: u16 = 24;
/// First mouse action code; codes below it are Windows virtual keys.
pub const MOUSE_CODE_FIRST: u16 = 0x100;
/// Code past the last mouse action.
pub const MOUSE_CODE_END: u16 = 0x10d;
/// Frame limits that Autorun understands; `0` means no limit.
pub const FRAME_LIMITS: [u16; 8] = [0, 30, 40, 45, 60, 75, 90, 120];

/// Switch button or stick direction: a source in the Autorun mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "UPPERCASE")]
#[ts(export)]
#[allow(missing_docs)]
pub enum SwitchButton {
    A,
    B,
    X,
    Y,
    L,
    R,
    Zl,
    Zr,
    Plus,
    Minus,
    StickL,
    StickR,
    Up,
    Down,
    Left,
    Right,
    LUp,
    LDown,
    LLeft,
    LRight,
    RUp,
    RDown,
    RLeft,
    RRight,
}

impl SwitchButton {
    /// All sources in Autorun order.
    pub const ALL: [Self; 24] = [
        Self::A,
        Self::B,
        Self::X,
        Self::Y,
        Self::L,
        Self::R,
        Self::Zl,
        Self::Zr,
        Self::Plus,
        Self::Minus,
        Self::StickL,
        Self::StickR,
        Self::Up,
        Self::Down,
        Self::Left,
        Self::Right,
        Self::LUp,
        Self::LDown,
        Self::LLeft,
        Self::LRight,
        Self::RUp,
        Self::RDown,
        Self::RLeft,
        Self::RRight,
    ];

    /// Returns the source name in Autorun files (`A`, `ZL`, `STICKL`, `LUP`…).
    #[must_use]
    pub fn autorun_name(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::X => "X",
            Self::Y => "Y",
            Self::L => "L",
            Self::R => "R",
            Self::Zl => "ZL",
            Self::Zr => "ZR",
            Self::Plus => "PLUS",
            Self::Minus => "MINUS",
            Self::StickL => "STICKL",
            Self::StickR => "STICKR",
            Self::Up => "UP",
            Self::Down => "DOWN",
            Self::Left => "LEFT",
            Self::Right => "RIGHT",
            Self::LUp => "LUP",
            Self::LDown => "LDOWN",
            Self::LLeft => "LLEFT",
            Self::LRight => "LRIGHT",
            Self::RUp => "RUP",
            Self::RDown => "RDOWN",
            Self::RLeft => "RLEFT",
            Self::RRight => "RRIGHT",
        }
    }
}

/// Input mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum InputMode {
    /// The game sees an Xbox 360 controller.
    #[default]
    Controller,
    /// Switch buttons are turned into keys and mouse actions.
    KeyboardMouse,
}

/// Binding of one Switch button: a chord of up to [`CHORD_MAX`] codes; empty means nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Binding {
    /// Switch button.
    pub button: SwitchButton,
    /// Codes: for the keyboard, a VK code or a mouse action from [`MOUSE_CODE_FIRST`]; for the controller,
    /// a target number 1…[`PAD_TARGET_COUNT`].
    pub codes: Vec<u16>,
}

/// Input: mode, dead zones and two mappings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InputSettings {
    /// Input mode.
    pub mode: InputMode,
    /// Left stick dead zone, 0…25.
    pub left_deadzone: u8,
    /// Right stick dead zone, 0…25.
    pub right_deadzone: u8,
    /// Show the on-screen keyboard automatically.
    pub keyboard_auto: bool,
    /// Mapping for the "keyboard and mouse" mode: all 24 buttons.
    pub keys: Vec<Binding>,
    /// Mapping for the controller mode: all 24 buttons.
    pub pad: Vec<Binding>,
}

impl Default for InputSettings {
    fn default() -> Self {
        const KEYS: [u16; 24] = [
            0x100, 0x101, 0x20, 0x46, 0x09, 0x10, 0x28, 0x26, 0x1b, 0x09, 0x11, 0x12, 0x31, 0x32,
            0x33, 0x34, 0x57, 0x53, 0x41, 0x44, 0x109, 0x10a, 0x10b, 0x10c,
        ];
        let keys = SwitchButton::ALL
            .iter()
            .zip(KEYS)
            .map(|(&button, code)| Binding {
                button,
                codes: vec![code],
            })
            .collect();
        let pad = SwitchButton::ALL
            .iter()
            .zip(1..=PAD_TARGET_COUNT)
            .map(|(&button, target)| Binding {
                button,
                codes: vec![target],
            })
            .collect();
        Self {
            mode: InputMode::Controller,
            left_deadzone: 5,
            right_deadzone: 5,
            keyboard_auto: true,
            keys,
            pad,
        }
    }
}

impl InputSettings {
    /// Checks the dead zones and both mappings.
    ///
    /// # Errors
    ///
    /// [`SettingsError::Deadzone`], [`SettingsError::Binding`] or [`SettingsError::DuplicateButton`].
    pub fn validate(&self) -> Result<(), SettingsError> {
        for deadzone in [self.left_deadzone, self.right_deadzone] {
            if deadzone > 25 {
                return Err(SettingsError::Deadzone(deadzone));
            }
        }
        validate_layout(&self.keys, |code| code != 0 && code < MOUSE_CODE_END)?;
        validate_layout(&self.pad, |code| (1..=PAD_TARGET_COUNT).contains(&code))
    }
}

fn validate_layout(layout: &[Binding], code_ok: impl Fn(u16) -> bool) -> Result<(), SettingsError> {
    let mut seen = Vec::with_capacity(layout.len());
    for binding in layout {
        if seen.contains(&binding.button) {
            return Err(SettingsError::DuplicateButton(binding.button));
        }
        seen.push(binding.button);
        let too_long = binding.codes.len() > CHORD_MAX;
        let bad_code = !binding.codes.iter().all(|&code| code_ok(code));
        let repeated = binding
            .codes
            .iter()
            .enumerate()
            .any(|(i, code)| binding.codes[..i].contains(code));
        if too_long || bad_code || repeated {
            return Err(SettingsError::Binding(binding.button));
        }
    }
    Ok(())
}

/// What renders Direct3D.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Direct3d {
    /// DXVK, the Autorun default.
    #[default]
    Dxvk,
    /// DXVK and VKD3D-Proton for Direct3D 12.
    DxvkVkd3d,
    /// WineD3D.
    Wine,
}

/// DXVK build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
#[allow(missing_docs)]
pub enum DxvkSource {
    #[default]
    Official,
    Sarek,
    Gplasync,
}

/// Scaling of the frame to the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Upscaling {
    /// Bilinear.
    Off,
    /// AMD FSR 1.0.
    #[default]
    Fsr,
    /// Integer.
    Integer,
}

/// DXVK HUD overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
#[allow(missing_docs)]
pub enum DxvkHud {
    #[default]
    Off,
    Fps,
    Compact,
    Full,
}

/// LSFG-VK optical flow scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
#[allow(missing_docs)]
pub enum LsfgFlow {
    Eighth,
    #[default]
    Quarter,
    Half,
}

/// Image output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GraphicsSettings {
    /// What renders Direct3D.
    pub direct3d: Direct3d,
    /// DXVK build.
    pub dxvk_source: DxvkSource,
    /// Frame limit from [`FRAME_LIMITS`]; `0` means no limit.
    pub frame_limit: u16,
    /// Vertical sync.
    pub vsync: bool,
    /// Scaling.
    pub upscaling: Upscaling,
    /// FSR sharpness in percent: 0, 20, …, 100.
    pub sharpness: u8,
    /// DXVK HUD.
    pub hud: DxvkHud,
    /// LSFG-VK frame generation.
    pub lsfg: bool,
    /// LSFG-VK in performance mode.
    pub lsfg_performance: bool,
    /// LSFG-VK optical flow scale.
    pub lsfg_flow: LsfgFlow,
}

impl Default for GraphicsSettings {
    fn default() -> Self {
        Self {
            direct3d: Direct3d::Dxvk,
            dxvk_source: DxvkSource::Official,
            frame_limit: 0,
            vsync: true,
            upscaling: Upscaling::Fsr,
            sharpness: 40,
            hud: DxvkHud::Off,
            lsfg: false,
            lsfg_performance: true,
            lsfg_flow: LsfgFlow::Quarter,
        }
    }
}

impl GraphicsSettings {
    /// Checks the frame limit, sharpness and DXVK compatibility with VKD3D.
    ///
    /// # Errors
    ///
    /// [`SettingsError::FrameLimit`], [`SettingsError::Sharpness`] or [`SettingsError::SarekWithVkd3d`].
    pub fn validate(&self) -> Result<(), SettingsError> {
        if !FRAME_LIMITS.contains(&self.frame_limit) {
            return Err(SettingsError::FrameLimit(self.frame_limit));
        }
        if self.sharpness > 100 || !self.sharpness.is_multiple_of(20) {
            return Err(SettingsError::Sharpness(self.sharpness));
        }
        if self.direct3d == Direct3d::DxvkVkd3d && self.dxvk_source == DxvkSource::Sarek {
            return Err(SettingsError::SarekWithVkd3d);
        }
        Ok(())
    }
}

/// Thread synchronization mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SyncMode {
    /// Waits through the emulated wineserver.
    Standard,
    /// Waits through the Horizon kernel (`sync = horizon`).
    #[default]
    Horizon,
}

/// x86 CPU emulator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
#[allow(missing_docs)]
pub enum CpuEmulator {
    #[default]
    Fex,
    Box64,
}

/// Where the game windows are output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum WindowOutput {
    /// The runtime decides.
    #[default]
    Auto,
    /// Directly to the framebuffer.
    Framebuffer,
    /// Through the compositor.
    Compositor,
}

/// CPU, synchronization, windows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SystemSettings {
    /// Thread synchronization mode.
    pub sync: SyncMode,
    /// CPU emulator.
    pub cpu: CpuEmulator,
    /// Give the game four cores.
    pub four_cores: bool,
    /// Where windows are output.
    pub windows: WindowOutput,
}

impl Default for SystemSettings {
    fn default() -> Self {
        Self {
            sync: SyncMode::Horizon,
            cpu: CpuEmulator::Fex,
            four_cores: true,
            windows: WindowOutput::Auto,
        }
    }
}

/// Debugging.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DebugSettings {
    /// Verbose traces.
    pub verbose: bool,
    /// Autorun profiler.
    pub profile: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        assert_eq!(AutorunSettings::default().validate(), Ok(()));
    }

    #[test]
    fn defaults_suit_the_switch() {
        let settings = AutorunSettings::default();
        assert_eq!(settings.system.sync, SyncMode::Horizon);
        assert_eq!(settings.graphics.upscaling, Upscaling::Fsr);
        assert_eq!(settings.graphics.sharpness, 40);
        assert!(settings.input.keyboard_auto);
        assert_eq!(settings.input.mode, InputMode::Controller);
        assert!(settings.system.four_cores);
        assert!(settings.graphics.vsync);
        assert!(!settings.graphics.lsfg);
    }

    #[test]
    fn default_keys_follow_autorun() {
        let input = InputSettings::default();
        assert_eq!(input.keys.len(), 24);
        assert_eq!(
            input.keys[0],
            Binding {
                button: SwitchButton::A,
                codes: vec![0x100]
            }
        );
        assert_eq!(
            input.keys[8],
            Binding {
                button: SwitchButton::Plus,
                codes: vec![0x1b]
            }
        );
        assert_eq!(
            input.pad[23],
            Binding {
                button: SwitchButton::RRight,
                codes: vec![24]
            }
        );
    }

    #[test]
    fn rejects_long_or_repeated_chords() {
        let mut input = InputSettings::default();
        input.keys[0].codes = vec![1, 2, 3, 4, 5];
        assert_eq!(
            input.validate(),
            Err(SettingsError::Binding(SwitchButton::A))
        );
        input.keys[0].codes = vec![0x41, 0x41];
        assert_eq!(
            input.validate(),
            Err(SettingsError::Binding(SwitchButton::A))
        );
    }

    #[test]
    fn rejects_pad_target_out_of_range() {
        let mut input = InputSettings::default();
        input.pad[2].codes = vec![25];
        assert_eq!(
            input.validate(),
            Err(SettingsError::Binding(SwitchButton::X))
        );
    }

    #[test]
    fn rejects_sarek_with_vkd3d() {
        let graphics = GraphicsSettings {
            direct3d: Direct3d::DxvkVkd3d,
            dxvk_source: DxvkSource::Sarek,
            ..GraphicsSettings::default()
        };
        assert_eq!(graphics.validate(), Err(SettingsError::SarekWithVkd3d));
    }

    #[test]
    fn rejects_frame_limit_outside_list() {
        let graphics = GraphicsSettings {
            frame_limit: 50,
            ..GraphicsSettings::default()
        };
        assert_eq!(graphics.validate(), Err(SettingsError::FrameLimit(50)));
    }
}
