//! Autorun runtime settings: input, graphics, system, debugging.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use crate::registry::{RegistryError, RegistrySettings};
use crate::settings_catalog::{self, Setting, Storage};

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
    /// Values of the catalog's settings without a typed field: FEX and DXVK options.
    #[serde(default)]
    pub options: OptionValues,
    /// Entries laid over the game's Windows registry and the console menu rows that change it.
    #[serde(default)]
    pub registry: RegistrySettings,
}

impl AutorunSettings {
    /// Checks the consistency of all groups.
    ///
    /// # Errors
    ///
    /// The first [`SettingsError`] found.
    pub fn validate(&self) -> Result<(), SettingsError> {
        self.input.validate()?;
        self.graphics.validate()?;
        self.options.validate()?;
        self.registry.validate().map_err(SettingsError::Registry)
    }
}

/// Chosen values of the catalog's settings kept by id ([`Storage::Option`]): a setting that is not here has its
/// default value.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, TS)]
#[ts(export, type = "Record<string, string>")]
pub struct OptionValues(BTreeMap<String, String>);

impl<'de> Deserialize<'de> for OptionValues {
    /// Keeps only the values the catalog has: a setting it no longer has, or a text that is not one of its values,
    /// falls back to the default.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut values = BTreeMap::<String, String>::deserialize(deserializer)?;
        values.retain(|id, text| option_fits(id, text));
        Ok(Self(values))
    }
}

fn option_fits(id: &str, text: &str) -> bool {
    settings_catalog::catalog()
        .setting(id)
        .is_some_and(|setting| {
            setting.storage == Storage::Option
                && setting
                    .values
                    .iter()
                    .any(|value| value.text.as_deref() == Some(text))
        })
}

impl OptionValues {
    /// Returns the file text chosen for a setting, or `None` when it has its default value.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&str> {
        self.0.get(id).map(String::as_str)
    }

    /// Sets the file text of a setting; `None` gives it back its default value.
    pub fn set(&mut self, id: &str, text: Option<&str>) {
        match text {
            Some(text) => {
                self.0.insert(id.to_owned(), text.to_owned());
            }
            None => {
                self.0.remove(id);
            }
        }
    }

    /// Returns the index of the value a catalog setting has.
    ///
    /// # Returns
    ///
    /// The default index when nothing is chosen or the chosen text is not one of the values.
    #[must_use]
    pub fn index(&self, setting: &Setting) -> usize {
        self.get(&setting.id)
            .and_then(|text| {
                setting
                    .values
                    .iter()
                    .position(|value| value.text.as_deref() == Some(text))
            })
            .unwrap_or(setting.default)
    }

    /// Returns the file text a catalog setting has: the chosen one or its default.
    #[must_use]
    pub fn text<'a>(&'a self, setting: &'a Setting) -> Option<&'a str> {
        setting.values[self.index(setting)].text.as_deref()
    }

    /// Checks that every id is a catalog setting kept here and every text one of its values.
    ///
    /// # Errors
    ///
    /// [`SettingsError::Option`] with the first id that does not fit.
    pub fn validate(&self) -> Result<(), SettingsError> {
        for (id, text) in &self.0 {
            if !option_fits(id, text) {
                return Err(SettingsError::Option(id.clone()));
            }
        }
        Ok(())
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
    /// An option that is not in the catalog, or a value it does not have.
    #[error("option {0} has an unknown value")]
    Option(String),
    /// A registry entry or a console menu row that cannot go into the package.
    #[error(transparent)]
    Registry(RegistryError),
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
        const PAD: [u16; 24] = [
            2, 1, 4, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
        ];
        let pad = SwitchButton::ALL
            .iter()
            .zip(PAD)
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

/// A component choice that Carafe no longer offers, found in settings written by an older version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RetiredChoice {
    /// Direct3D through Wine's wined3d; the game now gets DXVK.
    WineDirect3d,
    /// VKD3D chosen by hand; it now comes with games that use Direct3D 12.
    Vkd3dByHand,
    /// The GPL Async or Sarek build of DXVK; the game now gets Carafe's DXVK.
    OtherDxvk,
    /// The Box64 emulator and its options; the game now runs on FEX.
    Box64,
    /// Wine's standard thread synchronisation; the game now waits through Horizon.
    StandardSync,
    /// Graphics workers kept off core 3; they may now use it.
    ThreeCores,
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
    /// What rendered Direct3D in settings of an older version: `dxvk`, `dxvkVkd3d` or `wine`; never written.
    #[serde(default, rename = "direct3d", skip_serializing)]
    #[ts(skip)]
    pub retired_direct3d: Option<String>,
    /// The DXVK build of settings of an older version: `official`, `sarek` or `gplasync`; never written.
    #[serde(default, rename = "dxvkSource", skip_serializing)]
    #[ts(skip)]
    pub retired_dxvk_source: Option<String>,
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
            retired_direct3d: None,
            retired_dxvk_source: None,
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
    /// Checks the frame limit and sharpness.
    ///
    /// # Errors
    ///
    /// [`SettingsError::FrameLimit`] or [`SettingsError::Sharpness`].
    pub fn validate(&self) -> Result<(), SettingsError> {
        if !FRAME_LIMITS.contains(&self.frame_limit) {
            return Err(SettingsError::FrameLimit(self.frame_limit));
        }
        if self.sharpness > 100 || !self.sharpness.is_multiple_of(20) {
            return Err(SettingsError::Sharpness(self.sharpness));
        }
        Ok(())
    }
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

/// Windows, and the component choices of settings of an older version.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SystemSettings {
    /// Thread synchronisation of settings of an older version: `standard` or `horizon`; never written.
    #[serde(default, rename = "sync", skip_serializing)]
    #[ts(skip)]
    pub retired_sync: Option<String>,
    /// The CPU emulator of settings of an older version: `fex` or `box64`; never written.
    #[serde(default, rename = "cpu", skip_serializing)]
    #[ts(skip)]
    pub retired_cpu: Option<String>,
    /// Whether graphics workers could use core 3 in settings of an older version; never written.
    #[serde(default, rename = "fourCores", skip_serializing)]
    #[ts(skip)]
    pub retired_four_cores: Option<bool>,
    /// Where windows are output.
    pub windows: WindowOutput,
}

impl AutorunSettings {
    /// Returns the component choices of an older version that these settings had and that Carafe no longer
    /// offers.
    ///
    /// # Returns
    ///
    /// The choices in a fixed order; empty for settings written by this version.
    #[must_use]
    pub fn retired_choices(&self) -> Vec<RetiredChoice> {
        let graphics = &self.graphics;
        let system = &self.system;
        let mut choices = Vec::new();
        match graphics.retired_direct3d.as_deref() {
            Some("wine") => choices.push(RetiredChoice::WineDirect3d),
            Some("dxvkVkd3d") => choices.push(RetiredChoice::Vkd3dByHand),
            _ => {}
        }
        if matches!(
            graphics.retired_dxvk_source.as_deref(),
            Some("sarek" | "gplasync")
        ) {
            choices.push(RetiredChoice::OtherDxvk);
        }
        if system.retired_cpu.as_deref() == Some("box64") {
            choices.push(RetiredChoice::Box64);
        }
        if system.retired_sync.as_deref() == Some("standard") {
            choices.push(RetiredChoice::StandardSync);
        }
        if system.retired_four_cores == Some(false) {
            choices.push(RetiredChoice::ThreeCores);
        }
        choices
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
    fn options_the_catalog_no_longer_has_fall_back_to_default() {
        let options: OptionValues = serde_json::from_str(
            r#"{"fex-3dnow":"enable3dnow","fex-flags":"0","fex-call-cache":"1"}"#,
        )
        .unwrap();
        assert_eq!(options.get("fex-3dnow"), None);
        assert_eq!(options.get("fex-flags"), None);
        assert_eq!(options.get("fex-call-cache"), Some("1"));
        assert_eq!(options.validate(), Ok(()));
    }

    #[test]
    fn defaults_are_valid() {
        assert_eq!(AutorunSettings::default().validate(), Ok(()));
    }

    #[test]
    fn defaults_suit_the_switch() {
        let settings = AutorunSettings::default();
        assert_eq!(settings.graphics.upscaling, Upscaling::Fsr);
        assert_eq!(settings.graphics.sharpness, 40);
        assert!(settings.input.keyboard_auto);
        assert_eq!(settings.input.mode, InputMode::Controller);
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
    fn default_pad_keeps_button_letters() {
        let input = InputSettings::default();
        let target = |button: SwitchButton| {
            input
                .pad
                .iter()
                .find(|binding| binding.button == button)
                .unwrap()
                .codes
                .clone()
        };
        assert_eq!(target(SwitchButton::A), vec![2]);
        assert_eq!(target(SwitchButton::B), vec![1]);
        assert_eq!(target(SwitchButton::X), vec![4]);
        assert_eq!(target(SwitchButton::Y), vec![3]);
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

    const WITH_EVERY_RETIRED_CHOICE: &str = r#"{
        "input": {"mode": "controller", "leftDeadzone": 8, "rightDeadzone": 5, "keyboardAuto": true,
                  "keys": [{"button": "A", "codes": [32]}], "pad": [{"button": "A", "codes": [1]}]},
        "graphics": {"direct3d": "wine", "dxvkSource": "sarek", "frameLimit": 0, "vsync": true,
                     "upscaling": "fsr", "sharpness": 40, "hud": "off", "lsfg": false,
                     "lsfgPerformance": true, "lsfgFlow": "quarter"},
        "system": {"sync": "standard", "cpu": "box64", "fourCores": false, "windows": "compositor"},
        "debug": {"verbose": false, "profile": false},
        "options": {"box64-strongmem": "3", "fex-maxinst": "5000"}
    }"#;

    #[test]
    fn retired_choices_are_read_and_named() {
        let settings: AutorunSettings = serde_json::from_str(WITH_EVERY_RETIRED_CHOICE).unwrap();
        assert_eq!(
            settings.retired_choices(),
            vec![
                RetiredChoice::WineDirect3d,
                RetiredChoice::OtherDxvk,
                RetiredChoice::Box64,
                RetiredChoice::StandardSync,
                RetiredChoice::ThreeCores,
            ]
        );
        assert_eq!(settings.system.windows, WindowOutput::Compositor);
        assert_eq!(settings.options.get("box64-strongmem"), None);
        assert_eq!(settings.options.get("fex-maxinst"), Some("5000"));
        assert_eq!(settings.validate(), Ok(()));
    }

    #[test]
    fn retired_choices_are_not_written_again() {
        let settings: AutorunSettings = serde_json::from_str(WITH_EVERY_RETIRED_CHOICE).unwrap();
        let json = serde_json::to_string(&settings).unwrap();
        for key in [
            "direct3d",
            "dxvkSource",
            "\"sync\"",
            "\"cpu\"",
            "fourCores",
            "box64",
        ] {
            assert!(!json.contains(key), "{key} in {json}");
        }
        let again: AutorunSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(again.retired_choices(), Vec::new());
        assert_eq!(again.system.windows, WindowOutput::Compositor);
    }

    #[test]
    fn the_one_path_values_of_an_older_version_are_not_retired_choices() {
        let settings: AutorunSettings = serde_json::from_str(WRITTEN_BEFORE_OPTIONS).unwrap();
        assert_eq!(settings.retired_choices(), Vec::new());
        let vkd3d =
            WRITTEN_BEFORE_OPTIONS.replace(r#""direct3d": "dxvk""#, r#""direct3d": "dxvkVkd3d""#);
        let settings: AutorunSettings = serde_json::from_str(&vkd3d).unwrap();
        assert_eq!(settings.retired_choices(), vec![RetiredChoice::Vkd3dByHand]);
    }

    const WRITTEN_BEFORE_OPTIONS: &str = r#"{
        "input": {"mode": "keyboardMouse", "leftDeadzone": 8, "rightDeadzone": 5, "keyboardAuto": true,
                  "keys": [{"button": "A", "codes": [32]}], "pad": [{"button": "A", "codes": [1]}]},
        "graphics": {"direct3d": "dxvk", "dxvkSource": "official", "frameLimit": 30, "vsync": true,
                     "upscaling": "fsr", "sharpness": 40, "hud": "off", "lsfg": false,
                     "lsfgPerformance": true, "lsfgFlow": "quarter"},
        "system": {"sync": "horizon", "cpu": "fex", "fourCores": true, "windows": "auto"},
        "debug": {"verbose": false, "profile": false}
    }"#;

    #[test]
    fn settings_written_before_options_still_read() {
        let settings: AutorunSettings = serde_json::from_str(WRITTEN_BEFORE_OPTIONS).unwrap();
        assert_eq!(settings.input.left_deadzone, 8);
        assert_eq!(settings.graphics.frame_limit, 30);
        assert_eq!(settings.options, OptionValues::default());
    }

    #[test]
    fn options_round_trip_through_json() {
        let mut settings = AutorunSettings::default();
        settings.options.set("fex-tso", Some("1"));
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains(r#""options":{"fex-tso":"1"}"#));
        let back: AutorunSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(back, settings);
    }

    #[test]
    fn options_are_checked_against_the_catalog() {
        let mut options = OptionValues::default();
        options.set("fex-smc", Some("mtrack"));
        options.set("d3d9-maxAvailableMemory", Some("1024"));
        assert_eq!(options.validate(), Ok(()));
        options.set("fex-smc", Some("sometimes"));
        assert_eq!(
            options.validate(),
            Err(SettingsError::Option("fex-smc".to_owned()))
        );
        let mut unknown = OptionValues::default();
        unknown.set("no-such-option", Some("1"));
        assert_eq!(
            unknown.validate(),
            Err(SettingsError::Option("no-such-option".to_owned()))
        );
        let mut typed = OptionValues::default();
        typed.set("vsync", Some("1"));
        assert_eq!(
            typed.validate(),
            Err(SettingsError::Option("vsync".to_owned()))
        );
    }

    #[test]
    fn option_index_falls_back_to_the_default() {
        let catalog = settings_catalog::catalog();
        let maxinst = catalog.setting("fex-maxinst").unwrap();
        let mut options = OptionValues::default();
        assert_eq!(options.text(maxinst), Some("1000"));
        options.set("fex-maxinst", Some("5000"));
        assert_eq!(options.index(maxinst), 4);
        options.set("fex-maxinst", None);
        assert_eq!(options.index(maxinst), maxinst.default);
        let budget = catalog.setting("dxvk-maxMemoryBudget").unwrap();
        assert_eq!(options.text(budget), None);
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
