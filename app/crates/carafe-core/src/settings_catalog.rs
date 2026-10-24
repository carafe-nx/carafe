//! Every per-game setting Autorun reads: the file and key it lives under, its values, when it takes effect and
//! where the app and the console menu show it.

use std::sync::OnceLock;

use serde::Serialize;
use ts_rs::TS;

/// The settings file next to the game's `.exe` that holds a setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SettingFile {
    /// `<exe>.wine-nx.txt`.
    Program,
    /// `<exe>.keys.txt`.
    Keys,
    /// `<exe>.box64.txt`.
    Box64,
}

impl SettingFile {
    /// Returns the name used in `carafe/menu.txt`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Program => "program",
            Self::Keys => "keys",
            Self::Box64 => "box64",
        }
    }
}

/// When a change on the console takes effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Apply {
    /// While the game runs.
    Now,
    /// When the game starts again.
    Restart,
}

/// A Direct3D version a game can import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
#[allow(missing_docs)]
pub enum Api {
    D3d8,
    D3d9,
    D3d10,
    D3d11,
    D3d12,
}

impl Api {
    /// All versions, oldest first.
    pub const ALL: [Self; 5] = [
        Self::D3d8,
        Self::D3d9,
        Self::D3d10,
        Self::D3d11,
        Self::D3d12,
    ];

    /// Returns the name used in `carafe/menu.txt`: `d3d8`…`d3d12`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::D3d8 => "d3d8",
            Self::D3d9 => "d3d9",
            Self::D3d10 => "d3d10",
            Self::D3d11 => "d3d11",
            Self::D3d12 => "d3d12",
        }
    }
}

/// How a value, a section or a setting is named.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase", tag = "kind")]
#[ts(export)]
pub enum Label {
    /// An i18n key.
    Key {
        /// The key.
        key: String,
    },
    /// The same text in every language.
    Text {
        /// The text.
        text: String,
    },
    /// An i18n key whose text has `{{value}}` in it.
    Unit {
        /// The key.
        key: String,
        /// What `{{value}}` becomes.
        value: String,
    },
}

/// A value a setting can take.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SettingValue {
    /// The text written after the key; `None` means the key is not written.
    pub text: Option<String>,
    /// How the value is named.
    pub label: Label,
}

/// When a row is shown or can be changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase", tag = "kind")]
#[ts(export)]
pub enum Condition {
    /// Always.
    Always,
    /// Hidden unless the setting has one of the value indices.
    Show {
        /// The setting it depends on.
        setting: String,
        /// The value indices that show the row.
        values: Vec<usize>,
    },
    /// Dimmed and fixed unless the setting has one of the value indices.
    On {
        /// The setting it depends on.
        setting: String,
        /// The value indices that let the row change.
        values: Vec<usize>,
    },
}

/// Where the app keeps the value of a setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Storage {
    /// In a typed field of [`crate::settings::AutorunSettings`].
    Typed,
    /// In [`crate::settings::OptionValues`] under the setting's id.
    Option,
}

/// One setting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Setting {
    /// Id, unique in the catalog.
    pub id: String,
    /// File that holds it.
    pub file: SettingFile,
    /// Key in that file.
    pub key: String,
    /// Values, in the order they are offered.
    pub values: Vec<SettingValue>,
    /// Index of the value the setting has when its key is missing from the file.
    pub default: usize,
    /// When a change on the console takes effect.
    pub apply: Apply,
    /// When the row is shown or can be changed.
    pub condition: Condition,
    /// Direct3D versions the setting is for; empty means every game.
    pub apis: Vec<Api>,
    /// i18n key of the name.
    pub name: String,
    /// i18n key of the hint.
    pub hint: String,
    /// Where the app keeps the value.
    pub storage: Storage,
}

/// A list of rows a group shows on one page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Page {
    /// Id; `None` for the only page of a group without pages.
    pub id: Option<String>,
    /// i18n key of the name; `None` with `id`.
    pub name: Option<String>,
    /// Rows.
    pub settings: Vec<Setting>,
}

/// A row that opens a list of its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Group {
    /// Id, unique in the catalog.
    pub id: String,
    /// When the row is shown.
    pub condition: Condition,
    /// i18n key of the name.
    pub name: String,
    /// Pages: one without an id, or several with ids.
    pub pages: Vec<Page>,
}

/// A row of a section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase", tag = "kind")]
#[ts(export)]
pub enum Entry {
    /// A setting.
    Setting(Setting),
    /// A group.
    Group(Group),
}

/// A value of a preset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PresetValue {
    /// Setting id.
    pub setting: String,
    /// Value index.
    pub value: usize,
}

/// A set of values chosen together.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Preset {
    /// Id.
    pub id: String,
    /// i18n key of the name.
    pub name: String,
    /// i18n key of the hint.
    pub hint: String,
    /// Values it sets.
    pub values: Vec<PresetValue>,
}

/// Id of a section, in the order the sections are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
#[allow(missing_docs)]
pub enum SectionId {
    Controls,
    Graphics,
    Directx,
    Processor,
    System,
}

impl SectionId {
    /// Returns the name used in `carafe/menu.txt`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Controls => "controls",
            Self::Graphics => "graphics",
            Self::Directx => "directx",
            Self::Processor => "processor",
            Self::System => "system",
        }
    }
}

/// A tab of the menu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Section {
    /// Id.
    pub id: SectionId,
    /// i18n key of the name.
    pub name: String,
    /// Rows.
    pub entries: Vec<Entry>,
    /// Presets of the section's settings.
    pub presets: Vec<Preset>,
}

/// Kind of failure the error screen suggests settings for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
#[allow(missing_docs)]
pub enum FailureKind {
    Crash,
    Memory,
    Threads,
}

impl FailureKind {
    /// Returns the name used in `carafe/menu.txt`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Crash => "crash",
            Self::Memory => "memory",
            Self::Threads => "threads",
        }
    }
}

/// A setting the error screen offers for a kind of failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Suggestion {
    /// Kind of failure.
    pub kind: FailureKind,
    /// Setting id.
    pub setting: String,
    /// Value index offered.
    pub value: usize,
    /// i18n key of why it may help.
    pub why: String,
}

/// Something a button can press.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Choice {
    /// Code in the keys file.
    pub code: u16,
    /// How it is named.
    pub label: Label,
}

/// The whole catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Catalog {
    /// Sections in order.
    pub sections: Vec<Section>,
    /// Settings the error screen offers.
    pub suggestions: Vec<Suggestion>,
    /// Keys and mouse actions for the keyboard and mouse mode.
    pub keys: Vec<Choice>,
    /// Xbox controller targets for the controller mode.
    pub pads: Vec<Choice>,
}

impl Catalog {
    /// Returns every setting, groups included, in the order they are shown.
    #[must_use]
    pub fn settings(&self) -> Vec<&Setting> {
        let mut all = Vec::new();
        for section in &self.sections {
            for entry in &section.entries {
                match entry {
                    Entry::Setting(setting) => all.push(setting),
                    Entry::Group(group) => {
                        all.extend(group.pages.iter().flat_map(|page| &page.settings))
                    }
                }
            }
        }
        all
    }

    /// Returns the setting with the id.
    #[must_use]
    pub fn setting(&self, id: &str) -> Option<&Setting> {
        self.settings().into_iter().find(|setting| setting.id == id)
    }

    /// Returns every group.
    pub fn groups(&self) -> impl Iterator<Item = &Group> {
        self.sections.iter().flat_map(|section| {
            section.entries.iter().filter_map(|entry| match entry {
                Entry::Group(group) => Some(group),
                Entry::Setting(_) => None,
            })
        })
    }
}

/// Returns the catalog.
pub fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(build)
}

fn key(key: &str) -> Label {
    Label::Key {
        key: key.to_owned(),
    }
}

fn text(text: &str) -> Label {
    Label::Text {
        text: text.to_owned(),
    }
}

fn unit(key: &str, value: &str) -> Label {
    Label::Unit {
        key: key.to_owned(),
        value: value.to_owned(),
    }
}

fn word(name: &str) -> Label {
    key(&format!("catalog.values.{name}"))
}

fn value(text: Option<&str>, label: Label) -> SettingValue {
    SettingValue {
        text: text.map(str::to_owned),
        label,
    }
}

fn absent(label: Label) -> SettingValue {
    value(None, label)
}

fn is(text: &str, label: Label) -> SettingValue {
    value(Some(text), label)
}

fn plain(texts: &[&str]) -> Vec<SettingValue> {
    texts.iter().map(|&t| is(t, text(t))).collect()
}

fn units(unit_key: &str, texts: &[&str]) -> Vec<SettingValue> {
    texts
        .iter()
        .map(|&t| is(t, unit(&format!("catalog.values.{unit_key}"), t)))
        .collect()
}

fn off_on() -> Vec<SettingValue> {
    vec![is("0", word("off")), is("1", word("on"))]
}

fn on_off() -> Vec<SettingValue> {
    vec![is("1", word("on")), is("0", word("off"))]
}

impl Setting {
    fn new(
        id: &str,
        file: SettingFile,
        key: &str,
        apply: Apply,
        values: Vec<SettingValue>,
        default: usize,
    ) -> Self {
        Self {
            id: id.to_owned(),
            file,
            key: key.to_owned(),
            values,
            default,
            apply,
            condition: Condition::Always,
            apis: Vec::new(),
            name: format!("catalog.settings.{id}.name"),
            hint: format!("catalog.settings.{id}.hint"),
            storage: Storage::Option,
        }
    }

    fn typed(mut self) -> Self {
        self.storage = Storage::Typed;
        self
    }

    fn show(mut self, setting: &str, values: &[usize]) -> Self {
        self.condition = Condition::Show {
            setting: setting.to_owned(),
            values: values.to_vec(),
        };
        self
    }

    fn on(mut self, setting: &str, values: &[usize]) -> Self {
        self.condition = Condition::On {
            setting: setting.to_owned(),
            values: values.to_vec(),
        };
        self
    }

    fn apis(mut self, apis: &[Api]) -> Self {
        self.apis = apis.to_vec();
        self
    }
}

fn group(id: &str, condition: Condition, pages: Vec<Page>) -> Entry {
    Entry::Group(Group {
        id: id.to_owned(),
        condition,
        name: format!("catalog.groups.{id}"),
        pages,
    })
}

fn page(id: &str, settings: Vec<Setting>) -> Page {
    Page {
        id: Some(id.to_owned()),
        name: Some(format!("catalog.pages.{id}")),
        settings,
    }
}

fn single(settings: Vec<Setting>) -> Vec<Page> {
    vec![Page {
        id: None,
        name: None,
        settings,
    }]
}

fn show(setting: &str, values: &[usize]) -> Condition {
    Condition::Show {
        setting: setting.to_owned(),
        values: values.to_vec(),
    }
}

fn section(id: SectionId, entries: Vec<Entry>, presets: Vec<Preset>) -> Section {
    Section {
        id,
        name: format!("catalog.sections.{}", id.name()),
        entries,
        presets,
    }
}

fn rows(settings: Vec<Setting>) -> Vec<Entry> {
    settings.into_iter().map(Entry::Setting).collect()
}

fn build() -> Catalog {
    Catalog {
        sections: vec![
            section(SectionId::Controls, rows(controls()), Vec::new()),
            section(SectionId::Graphics, rows(graphics()), Vec::new()),
            section(SectionId::Directx, directx(), Vec::new()),
            section(SectionId::Processor, processor(), fex_presets()),
            section(SectionId::System, rows(system()), Vec::new()),
        ],
        suggestions: suggestions(),
        keys: keys(),
        pads: (1..=24)
            .map(|code| Choice {
                code,
                label: key(&format!("padShort.t{code}")),
            })
            .collect(),
    }
}

const DEADZONES: [&str; 8] = ["0", "2", "5", "8", "10", "15", "20", "25"];

fn controls() -> Vec<Setting> {
    use Apply::{Now, Restart};
    use SettingFile::Keys;
    vec![
        Setting::new(
            "input-mode",
            Keys,
            "input-mode",
            Restart,
            vec![
                is("1", key("input.controller")),
                is("2", key("input.keyboardMouse")),
            ],
            0,
        )
        .typed(),
        Setting::new(
            "left-deadzone",
            Keys,
            "left-deadzone",
            Now,
            units("percent", &DEADZONES),
            2,
        )
        .typed(),
        Setting::new(
            "right-deadzone",
            Keys,
            "right-deadzone",
            Now,
            units("percent", &DEADZONES),
            2,
        )
        .typed(),
        Setting::new("keyboard-auto", Keys, "keyboard-auto", Now, off_on(), 1).typed(),
    ]
}

fn graphics() -> Vec<Setting> {
    use Apply::{Now, Restart};
    use SettingFile::Program;
    let mut frame_limits = vec![absent(key("graphics.noLimit"))];
    frame_limits.extend(plain(&["30", "40", "45", "60", "75", "90", "120"]));
    vec![
        Setting::new("frame-limit", Program, "frame-limit", Now, frame_limits, 0).typed(),
        Setting::new("vsync", Program, "vsync", Restart, on_off(), 0).typed(),
        Setting::new(
            "upscaling",
            Program,
            "upscaling",
            Restart,
            vec![
                is("off", key("graphics.upscalingOff")),
                is("fsr", text("FSR 1.0")),
                is("integer", key("graphics.upscalingInteger")),
            ],
            0,
        )
        .typed(),
        Setting::new(
            "upscaling-sharpness",
            Program,
            "upscaling-sharpness",
            Restart,
            ["0", "20", "40", "60", "80", "100"]
                .iter()
                .map(|&n| is(&format!("{n}%"), unit("catalog.values.percent", n)))
                .collect(),
            2,
        )
        .typed()
        .on("upscaling", &[1]),
        Setting::new(
            "dxvk-hud",
            Program,
            "dxvk-hud",
            Restart,
            vec![
                is("0", key("graphics.hudOff")),
                is("fps", key("graphics.hudFps")),
                is("api,fps,frametimes", key("graphics.hudCompact")),
                is(
                    "version,api,devinfo,fps,memory,frametimes,compiler",
                    key("graphics.hudFull"),
                ),
            ],
            0,
        )
        .typed(),
        Setting::new("lsfg", Program, "lsfg", Restart, off_on(), 0).typed(),
        Setting::new(
            "lsfg-performance",
            Program,
            "lsfg-performance",
            Restart,
            on_off(),
            0,
        )
        .typed()
        .on("lsfg", &[1]),
        Setting::new(
            "lsfg-flow",
            Program,
            "lsfg-flow",
            Restart,
            vec![
                is("0.125", word("flowEighth")),
                is("0.25", word("flowQuarter")),
                is("0.5", word("flowHalf")),
            ],
            1,
        )
        .typed()
        .on("lsfg", &[1]),
    ]
}

fn directx() -> Vec<Entry> {
    use Apply::Restart;
    use SettingFile::Program;
    let dxvk = show("d3d", &[0, 1]);
    vec![
        Entry::Setting(
            Setting::new(
                "d3d",
                Program,
                "d3d",
                Restart,
                vec![
                    absent(text("DXVK")),
                    is("dxvk+vkd3d", text("DXVK + VKD3D")),
                    is("wine", key("graphics.wined3d")),
                ],
                0,
            )
            .typed(),
        ),
        Entry::Setting(
            Setting::new(
                "dxvk-source",
                Program,
                "dxvk-source",
                Restart,
                vec![
                    absent(key("graphics.dxvkOfficial")),
                    is("gplasync", text("GPL Async")),
                    is("sarek", text("Sarek")),
                ],
                0,
            )
            .typed()
            .show("d3d", &[0, 1]),
        ),
        Entry::Setting(
            Setting::new(
                "dxvk-version",
                Program,
                "dxvk-version",
                Restart,
                vec![absent(unit("catalog.values.bundled", "3.1.1"))],
                0,
            )
            .show("d3d", &[0, 1]),
        ),
        Entry::Setting(
            Setting::new(
                "vkd3d-version",
                Program,
                "vkd3d-version",
                Restart,
                vec![absent(unit("catalog.values.bundled", "3.0.1"))],
                0,
            )
            .show("d3d", &[1]),
        ),
        group("dxvk", dxvk, dxvk_pages()),
    ]
}

const D3D9: &[Api] = &[Api::D3d8, Api::D3d9];
const D3D11: &[Api] = &[Api::D3d10, Api::D3d11];
const DXGI: &[Api] = &[Api::D3d10, Api::D3d11, Api::D3d12];

fn dxvk_id(name: &str) -> String {
    name.replace('.', "-")
}

fn dxvk(name: &str, values: Vec<SettingValue>) -> Setting {
    let apis: &[Api] = match name.split_once('.').map(|(prefix, _)| prefix) {
        Some("d3d9") => D3D9,
        Some("d3d11") => D3D11,
        Some("dxgi") => DXGI,
        _ => &[],
    };
    let mut all = vec![absent(word("default"))];
    all.extend(values);
    Setting::new(
        &dxvk_id(name),
        SettingFile::Program,
        name,
        Apply::Restart,
        all,
        0,
    )
    .apis(apis)
}

fn dxvk_bool() -> Vec<SettingValue> {
    vec![is("True", word("on")), is("False", word("off"))]
}

fn dxvk_tri() -> Vec<SettingValue> {
    vec![
        is("Auto", word("auto")),
        is("True", word("on")),
        is("False", word("off")),
    ]
}

fn dxvk_threads() -> Vec<SettingValue> {
    let mut values = vec![is("0", word("auto"))];
    values.extend(plain(&["1", "2", "3", "4", "6", "8", "12", "16"]));
    values
}

const MEMORY: [&str; 9] = [
    "128", "256", "512", "768", "1024", "1536", "2048", "3072", "4096",
];

fn dxvk_memory() -> Vec<SettingValue> {
    let mut values = vec![is("0", word("auto"))];
    values.extend(units("mib", &MEMORY));
    values
}

fn dxvk_latency() -> Vec<SettingValue> {
    let mut values = vec![is("0", word("game"))];
    values.extend(plain(&["1", "2", "3", "4", "8", "16"]));
    values
}

fn dxvk_anisotropy() -> Vec<SettingValue> {
    let mut values = vec![is("-1", word("game")), is("0", word("off"))];
    values.extend(units("times", &["2", "4", "8", "16"]));
    values
}

fn dxvk_lod() -> Vec<SettingValue> {
    vec![
        is("-2", text("−2")),
        is("-1", text("−1")),
        is("-0.5", text("−0.5")),
        is("0", text("0")),
        is("0.5", text("+0.5")),
        is("1", text("+1")),
    ]
}

fn dxvk_refresh() -> Vec<SettingValue> {
    let mut values = vec![is("0", word("game"))];
    values.extend(units("hz", &["30", "60", "120"]));
    values
}

fn dxvk_pages() -> Vec<Page> {
    vec![
        page(
            "dxvk-memory",
            vec![
                dxvk("dxvk.maxMemoryBudget", dxvk_memory()),
                dxvk("dxgi.maxDeviceMemory", dxvk_memory()),
                dxvk("dxgi.maxSharedMemory", dxvk_memory()),
                dxvk("d3d9.maxAvailableMemory", units("mib", &MEMORY)),
                dxvk(
                    "d3d9.textureMemory",
                    units("mib", &["16", "32", "64", "100", "128", "256"]),
                ),
                dxvk("dxvk.enableMemoryDefrag", dxvk_tri()),
                dxvk("dxvk.trackPipelineLifetime", dxvk_tri()),
            ],
        ),
        page(
            "dxvk-frames",
            vec![
                dxvk("dxgi.maxFrameLatency", dxvk_latency()),
                dxvk("d3d9.maxFrameLatency", dxvk_latency()),
                dxvk("dxvk.latencySleep", dxvk_tri()),
                dxvk("dxvk.tearFree", dxvk_tri()),
            ],
        ),
        page(
            "dxvk-shaders",
            vec![
                dxvk("dxvk.numCompilerThreads", dxvk_threads()),
                dxvk(
                    "dxvk.enableGraphicsPipelineLibrary",
                    vec![
                        is("Auto", word("auto")),
                        is("True", word("noRefine")),
                        is("False", word("off")),
                    ],
                ),
                dxvk(
                    "d3d9.floatEmulation",
                    vec![
                        is("Auto", word("auto")),
                        is("True", word("faster")),
                        is("False", word("off")),
                        is("Strict", word("strict")),
                    ],
                ),
                dxvk("dxvk.lowerSinCos", dxvk_tri()),
                dxvk("d3d9.useFP16", dxvk_tri()),
            ],
        ),
        page(
            "dxvk-quality",
            vec![
                dxvk("d3d11.maxTessFactor", {
                    let mut values = vec![is("0", word("game"))];
                    values.extend(plain(&["8", "16", "32", "64"]));
                    values
                }),
                dxvk("d3d11.disableMsaa", dxvk_bool()),
                dxvk("d3d11.samplerAnisotropy", dxvk_anisotropy()),
                dxvk("d3d9.samplerAnisotropy", dxvk_anisotropy()),
                dxvk("d3d11.samplerLodBias", dxvk_lod()),
                dxvk("d3d9.samplerLodBias", dxvk_lod()),
                dxvk("d3d11.clampNegativeLodBias", dxvk_bool()),
                dxvk("d3d9.clampNegativeLodBias", dxvk_bool()),
            ],
        ),
        page(
            "dxvk-display",
            vec![
                dxvk("dxgi.deferSurfaceCreation", dxvk_bool()),
                dxvk("d3d9.deferSurfaceCreation", dxvk_bool()),
                dxvk(
                    "d3d9.forceAspectRatio",
                    vec![
                        is("\"\"", word("all")),
                        is("\"16:9\"", text("16:9")),
                        is("\"4:3\"", text("4:3")),
                        is("\"16:10\"", text("16:10")),
                    ],
                ),
                dxvk("dxgi.forceRefreshRate", dxvk_refresh()),
                dxvk("d3d9.forceRefreshRate", dxvk_refresh()),
                dxvk("d3d9.modeCountCompatibility", dxvk_bool()),
            ],
        ),
        page(
            "dxvk-fixes",
            vec![
                dxvk(
                    "d3d11.cachedDynamicResources",
                    vec![
                        is("\"\"", word("none")),
                        is("\"v\"", word("vertexBuffers")),
                        is("\"i\"", word("indexBuffers")),
                        is("\"c\"", word("constantBuffers")),
                        is("\"r\"", word("shaderResources")),
                        is("\"vi\"", word("vertexIndex")),
                        is("\"a\"", word("all")),
                    ],
                ),
                dxvk("d3d9.deviceLocalConstantBuffers", dxvk_tri()),
                dxvk("d3d9.cachedWriteOnlyBuffers", dxvk_bool()),
                dxvk("d3d9.allowDirectBufferMapping", dxvk_bool()),
                dxvk("d3d11.disableDirectImageMapping", dxvk_bool()),
                dxvk("d3d11.enableContextLock", dxvk_bool()),
                dxvk("d3d11.exposeDriverCommandLists", dxvk_bool()),
                dxvk("d3d11.forceComputeLdsBarriers", dxvk_bool()),
                dxvk("d3d11.forceComputeUavBarriers", dxvk_bool()),
                dxvk("d3d9.lenientClear", dxvk_bool()),
                dxvk("d3d9.forceSamplerTypeSpecConstants", dxvk_bool()),
            ],
        ),
        page(
            "dxvk-render",
            vec![
                dxvk("dxvk.enableDescriptorBuffer", dxvk_tri()),
                dxvk("dxvk.enableDescriptorHeap", dxvk_tri()),
                dxvk("dxvk.enableUnifiedImageLayouts", dxvk_bool()),
                dxvk("dxvk.enableImplicitResolves", dxvk_bool()),
                dxvk("dxvk.useRawSsbo", dxvk_tri()),
            ],
        ),
        page(
            "dxvk-identity",
            vec![
                dxvk(
                    "d3d11.maxFeatureLevel",
                    ["9_1", "9_2", "9_3", "10_0", "10_1", "11_0", "11_1"]
                        .iter()
                        .map(|&level| is(level, text(&level.replace('_', "."))))
                        .collect(),
                ),
                dxvk("dxgi.hideNvidiaGpu", dxvk_tri()),
                dxvk("dxgi.hideNvkGpu", dxvk_tri()),
                dxvk("d3d9.hideNvidiaGpu", dxvk_tri()),
                dxvk("d3d9.hideNvkGpu", dxvk_tri()),
            ],
        ),
    ]
}

fn fex(id: &str, key: &str, values: Vec<SettingValue>, default: usize) -> Setting {
    Setting::new(
        id,
        SettingFile::Program,
        key,
        Apply::Restart,
        values,
        default,
    )
}

fn box64(id: &str, name: &str, values: Vec<SettingValue>, default: usize) -> Setting {
    Setting::new(
        id,
        SettingFile::Box64,
        &format!("BOX64_DYNAREC_{name}"),
        Apply::Restart,
        values,
        default,
    )
}

fn numbered(labels: &[&str]) -> Vec<SettingValue> {
    labels
        .iter()
        .enumerate()
        .map(|(i, name)| is(&i.to_string(), word(name)))
        .collect()
}

fn processor() -> Vec<Entry> {
    let on_fex = || show("cpu", &[0]);
    let on_box64 = || show("cpu", &[1]);
    let mut entries = vec![Entry::Setting(
        Setting::new(
            "cpu",
            SettingFile::Program,
            "cpu",
            Apply::Restart,
            vec![is("fex", text("FEX")), is("box64", text("Box64"))],
            0,
        )
        .typed(),
    )];
    entries.extend(
        [
            fex("fex-tso", "FEX_TSOENABLED", off_on(), 0),
            fex("fex-x87", "FEX_X87REDUCEDPRECISION", off_on(), 1),
            fex(
                "fex-smc",
                "FEX_SMCCHECKS",
                vec![
                    is("none", word("none")),
                    is("mtrack", text("MTrack")),
                    is("full", text("Full")),
                ],
                0,
            ),
            fex(
                "fex-maxinst",
                "FEX_MAXINST",
                plain(&["250", "500", "1000", "2500", "5000", "10000", "20000"]),
                2,
            ),
            fex("fex-multiblock", "FEX_MULTIBLOCK", off_on(), 1),
            fex(
                "fex-l2",
                "FEX_DISABLEL2CACHE",
                vec![is("1", word("off")), is("0", word("on"))],
                0,
            ),
            fex("fex-l1", "FEX_DYNAMICL1CACHE", off_on(), 1),
        ]
        .into_iter()
        .map(|setting| Entry::Setting(setting.show("cpu", &[0]))),
    );
    entries.push(group(
        "fex-memory",
        on_fex(),
        single(vec![
            fex("fex-vector-tso", "FEX_VECTORTSOENABLED", off_on(), 0).on("fex-tso", &[1]),
            fex("fex-memcpy-tso", "FEX_MEMCPYSETTSOENABLED", off_on(), 0).on("fex-tso", &[1]),
            fex("fex-half-barrier", "FEX_HALFBARRIERTSOENABLED", off_on(), 1).on("fex-tso", &[1]),
            fex(
                "fex-split-locks",
                "FEX_STRICTINPROCESSSPLITLOCKS",
                off_on(),
                0,
            ),
            fex(
                "fex-unaligned",
                "FEX_KERNELUNALIGNEDATOMICBACKPATCHING",
                off_on(),
                1,
            ),
            fex("fex-volatile", "FEX_VOLATILEMETADATA", off_on(), 1),
        ]),
    ));
    entries.push(group(
        "fex-compat",
        on_fex(),
        single(vec![
            fex("fex-mono", "FEX_MONOHACKS", off_on(), 1),
            fex("fex-hypervisor", "FEX_HIDEHYPERVISORBIT", off_on(), 0),
            fex("fex-small-tsc", "FEX_SMALLTSCSCALE", off_on(), 1),
            fex("fex-hybrid", "FEX_HIDEHYBRID", off_on(), 1),
            fex(
                "fex-l1-increase",
                "FEX_DYNAMICL1CACHEINCREASECOUNTHEURISTIC",
                plain(&["50", "100", "250", "500", "1000"]),
                2,
            )
            .on("fex-l1", &[1]),
            fex(
                "fex-l1-decrease",
                "FEX_DYNAMICL1CACHEDECREASECOUNTHEURISTIC",
                plain(&["10", "25", "50", "100", "200"]),
                2,
            )
            .on("fex-l1", &[1]),
        ]),
    ));
    entries.extend(
        [
            box64(
                "box64-atomics",
                "ALIGNED_ATOMICS",
                numbered(&["safe", "faster"]),
                0,
            ),
            box64(
                "box64-bigblock",
                "BIGBLOCK",
                numbered(&["small", "large", "balanced", "wine"]),
                2,
            ),
            box64(
                "box64-callret",
                "CALLRET",
                numbered(&["off", "fast", "guarded"]),
                2,
            ),
            box64(
                "box64-safeflags",
                "SAFEFLAGS",
                numbered(&["fast", "balanced", "safe"]),
                1,
            ),
            box64(
                "box64-strongmem",
                "STRONGMEM",
                vec![
                    is("0", word("off")),
                    is("1", word("basic")),
                    is("2", text("SIMD")),
                    is("3", word("strong")),
                    is("4", text("TSO")),
                ],
                0,
            ),
            box64(
                "box64-forward",
                "FORWARD",
                plain(&["0", "128", "256", "512", "1024"]),
                1,
            ),
        ]
        .into_iter()
        .map(|setting| Entry::Setting(setting.show("cpu", &[1]))),
    );
    entries.push(group(
        "box64-precision",
        on_box64(),
        single(vec![
            box64("box64-df", "DF", off_on(), 1),
            box64("box64-div0", "DIV0", off_on(), 1),
            box64(
                "box64-fastnan",
                "FASTNAN",
                numbered(&["precise", "fast"]),
                1,
            ),
            box64(
                "box64-fastround",
                "FASTROUND",
                numbered(&["precise", "fast", "hybrid"]),
                1,
            ),
            box64("box64-nativeflags", "NATIVEFLAGS", off_on(), 1),
            box64(
                "box64-x87double",
                "X87DOUBLE",
                numbered(&["auto", "double", "honor"]),
                0,
            ),
        ]),
    ));
    entries.push(group(
        "box64-memory",
        on_box64(),
        single(vec![
            box64(
                "box64-noarch",
                "NOARCH",
                numbered(&["full", "reduced", "minimal"]),
                0,
            ),
            box64(
                "box64-pause",
                "PAUSE",
                vec![
                    is("0", word("ignore")),
                    is("1", word("yield")),
                    is("2", text("WFI")),
                    is("3", text("WFE")),
                ],
                0,
            ),
            box64("box64-sep", "SEP", numbered(&["off", "files", "all"]), 2),
            box64(
                "box64-weakbarrier",
                "WEAKBARRIER",
                numbered(&["safe", "weak", "weakest"]),
                1,
            ),
            box64("box64-purge", "PURGE", off_on(), 0),
            box64(
                "box64-purge-age",
                "PURGE_AGE",
                [("500", "5"), ("1000", "10"), ("3000", "30"), ("6000", "60")]
                    .iter()
                    .map(|&(ticks, seconds)| is(ticks, unit("catalog.values.seconds", seconds)))
                    .collect(),
                1,
            )
            .on("box64-purge", &[1]),
        ]),
    ));
    entries
}

fn preset(id: &str, values: &[(&str, usize)]) -> Preset {
    Preset {
        id: id.to_owned(),
        name: format!("catalog.presets.{id}.name"),
        hint: format!("catalog.presets.{id}.hint"),
        values: values
            .iter()
            .map(|&(setting, value)| PresetValue {
                setting: setting.to_owned(),
                value,
            })
            .collect(),
    }
}

fn fex_presets() -> Vec<Preset> {
    vec![
        preset(
            "balanced",
            &[
                ("fex-tso", 0),
                ("fex-x87", 1),
                ("fex-smc", 0),
                ("fex-maxinst", 2),
                ("fex-l1", 1),
            ],
        ),
        preset(
            "reliable",
            &[
                ("fex-tso", 1),
                ("fex-x87", 0),
                ("fex-smc", 1),
                ("fex-maxinst", 2),
                ("fex-l1", 1),
            ],
        ),
        preset(
            "fast",
            &[
                ("fex-tso", 0),
                ("fex-x87", 1),
                ("fex-smc", 0),
                ("fex-maxinst", 4),
                ("fex-l1", 0),
            ],
        ),
    ]
}

fn system() -> Vec<Setting> {
    use Apply::Restart;
    use SettingFile::Program;
    vec![
        Setting::new(
            "sync",
            Program,
            "sync",
            Restart,
            vec![
                is("horizon", text("Horizon")),
                is("standard", key("advanced.syncStandard")),
            ],
            1,
        )
        .typed(),
        Setting::new("four-cores", Program, "four-cores", Restart, on_off(), 0).typed(),
        Setting::new(
            "windows",
            Program,
            "windows",
            Restart,
            vec![
                absent(key("advanced.windowsAuto")),
                is("framebuffer", key("advanced.windowsFramebuffer")),
                is("compositor", key("advanced.windowsCompositor")),
            ],
            0,
        )
        .typed(),
        Setting::new("verbose", Program, "verbose", Restart, off_on(), 0).typed(),
        Setting::new("profile", Program, "profile", Restart, off_on(), 0).typed(),
    ]
}

fn suggestion(kind: FailureKind, setting: &str, value: usize) -> Suggestion {
    Suggestion {
        kind,
        setting: setting.to_owned(),
        value,
        why: format!("catalog.suggest.{}.{setting}", kind.name()),
    }
}

fn suggestions() -> Vec<Suggestion> {
    use FailureKind::{Crash, Memory, Threads};
    vec![
        suggestion(Crash, "fex-tso", 1),
        suggestion(Crash, "fex-x87", 0),
        suggestion(Crash, "fex-smc", 1),
        suggestion(Memory, "d3d9-maxAvailableMemory", 5),
        suggestion(Memory, "dxvk-maxMemoryBudget", 6),
        suggestion(Memory, "fex-maxinst", 1),
        suggestion(Threads, "dxvk-numCompilerThreads", 3),
        suggestion(Threads, "sync", 1),
    ]
}

const NAMED_KEYS: [(u16, &str); 39] = [
    (0x08, "Backspace"),
    (0x09, "Tab"),
    (0x0d, "Enter"),
    (0x10, "Shift"),
    (0x11, "Ctrl"),
    (0x12, "Alt"),
    (0x13, "Pause"),
    (0x14, "Caps Lock"),
    (0x1b, "Esc"),
    (0x20, "Space"),
    (0x21, "Page Up"),
    (0x22, "Page Down"),
    (0x23, "End"),
    (0x24, "Home"),
    (0x25, "←"),
    (0x26, "↑"),
    (0x27, "→"),
    (0x28, "↓"),
    (0x2d, "Insert"),
    (0x2e, "Delete"),
    (0x5b, "Win"),
    (0x6a, "Num *"),
    (0x6b, "Num +"),
    (0x6d, "Num −"),
    (0x6e, "Num ."),
    (0x6f, "Num /"),
    (0x90, "Num Lock"),
    (0x91, "Scroll Lock"),
    (0xba, ";"),
    (0xbb, "="),
    (0xbc, ","),
    (0xbd, "-"),
    (0xbe, "."),
    (0xbf, "/"),
    (0xc0, "`"),
    (0xdb, "["),
    (0xdc, "\\"),
    (0xdd, "]"),
    (0xde, "'"),
];

const MOUSE_ACTIONS: [&str; 13] = [
    "left",
    "right",
    "middle",
    "back",
    "forward",
    "wheelUp",
    "wheelDown",
    "wheelLeft",
    "wheelRight",
    "up",
    "down",
    "moveLeft",
    "moveRight",
];

fn keys() -> Vec<Choice> {
    let mut codes: Vec<(u16, Label)> = NAMED_KEYS
        .iter()
        .map(|&(code, name)| (code, text(name)))
        .collect();
    for code in (b'0'..=b'9').chain(b'A'..=b'Z') {
        codes.push((u16::from(code), text(&char::from(code).to_string())));
    }
    for code in 0x60u16..=0x69 {
        codes.push((code, text(&format!("Num {}", code - 0x60))));
    }
    for code in 0x70u16..=0x7b {
        codes.push((code, text(&format!("F{}", code - 0x6f))));
    }
    codes.sort_by_key(|(code, _)| *code);
    codes
        .into_iter()
        .map(|(code, label)| Choice { code, label })
        .chain(
            MOUSE_ACTIONS
                .iter()
                .zip(0x100u16..)
                .map(|(action, code)| Choice {
                    code,
                    label: key(&format!("mouseShort.{action}")),
                }),
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn ids_are_unique() {
        let catalog = catalog();
        let mut seen = HashSet::new();
        for setting in catalog.settings() {
            assert!(seen.insert(setting.id.clone()), "{} twice", setting.id);
        }
        for group in catalog.groups() {
            assert!(seen.insert(group.id.clone()), "{} twice", group.id);
        }
    }

    #[test]
    fn settings_iterate_into_groups_and_pages() {
        let catalog = catalog();
        assert!(catalog.setting("fex-vector-tso").is_some());
        assert!(catalog.setting("d3d9-maxAvailableMemory").is_some());
        assert!(catalog.setting("box64-purge-age").is_some());
    }

    #[test]
    fn counts_match_autorun() {
        let catalog = catalog();
        let dxvk = catalog
            .groups()
            .find(|group| group.id == "dxvk")
            .unwrap()
            .pages
            .iter()
            .map(|page| page.settings.len())
            .sum::<usize>();
        assert_eq!(dxvk, 51);
        let fex = catalog
            .settings()
            .into_iter()
            .filter(|s| s.key.starts_with("FEX_"))
            .count();
        assert_eq!(fex, 19);
        let box64 = catalog
            .settings()
            .into_iter()
            .filter(|s| s.file == SettingFile::Box64)
            .count();
        assert_eq!(box64, 18);
    }

    #[test]
    fn defaults_and_conditions_point_at_real_values() {
        let catalog = catalog();
        let check = |condition: &Condition| match condition {
            Condition::Always => {}
            Condition::Show { setting, values } | Condition::On { setting, values } => {
                let target = catalog
                    .setting(setting)
                    .unwrap_or_else(|| panic!("no {setting}"));
                assert!(values.iter().all(|&v| v < target.values.len()), "{setting}");
            }
        };
        for setting in catalog.settings() {
            assert!(setting.default < setting.values.len(), "{}", setting.id);
            check(&setting.condition);
        }
        for group in catalog.groups() {
            check(&group.condition);
        }
        for section in &catalog.sections {
            for preset in &section.presets {
                for value in &preset.values {
                    let target = catalog.setting(&value.setting).unwrap();
                    assert!(value.value < target.values.len());
                }
            }
        }
        for suggestion in &catalog.suggestions {
            let target = catalog.setting(&suggestion.setting).unwrap();
            assert!(
                suggestion.value < target.values.len(),
                "{}",
                suggestion.setting
            );
            if target.storage == Storage::Option {
                assert_ne!(suggestion.value, target.default, "{}", suggestion.setting);
            }
        }
    }

    #[test]
    fn values_of_a_setting_differ() {
        for setting in catalog().settings() {
            let mut seen = HashSet::new();
            for value in &setting.values {
                assert!(seen.insert(value.text.clone()), "{}", setting.id);
            }
        }
    }

    #[test]
    fn presets_follow_the_decision() {
        let catalog = catalog();
        let processor = catalog
            .sections
            .iter()
            .find(|section| section.id == SectionId::Processor)
            .unwrap();
        let text_of = |preset: &str, setting: &str| {
            let preset = processor.presets.iter().find(|p| p.id == preset).unwrap();
            let value = preset.values.iter().find(|v| v.setting == setting).unwrap();
            catalog.setting(setting).unwrap().values[value.value]
                .text
                .clone()
                .unwrap()
        };
        assert_eq!(text_of("reliable", "fex-tso"), "1");
        assert_eq!(text_of("reliable", "fex-x87"), "0");
        assert_eq!(text_of("reliable", "fex-smc"), "mtrack");
        assert_eq!(text_of("fast", "fex-maxinst"), "5000");
        assert_eq!(text_of("fast", "fex-l1"), "0");
        for setting in ["fex-tso", "fex-x87", "fex-smc", "fex-maxinst", "fex-l1"] {
            let preset = processor
                .presets
                .iter()
                .find(|p| p.id == "balanced")
                .unwrap();
            let value = preset.values.iter().find(|v| v.setting == setting).unwrap();
            assert_eq!(value.value, catalog.setting(setting).unwrap().default);
        }
    }

    #[test]
    fn keys_cover_letters_and_the_mouse() {
        let keys = &catalog().keys;
        assert!(keys.iter().any(|choice| choice.code == 0x41));
        assert!(keys.iter().any(|choice| choice.code == 0x10c));
        assert_eq!(catalog().pads.len(), 24);
    }
}
