//! Domain model and use cases of the Carafe builder.
//!
//! The core performs no I/O: files, processes, the device and randomness come in through traits
//! from [`ports`] or as arguments.

pub mod autorun_files;
pub mod dbi;
pub mod disk_space;
pub mod exe_icon;
pub mod icon;
pub mod keys;
pub mod library;
pub mod links;
pub mod metadata;
pub mod nacp;
pub mod npdm;
pub mod package;
pub mod path_limit;
pub mod pe;
pub mod pfs0;
pub mod ports;
pub mod record;
pub mod runtime_files;
pub mod settings;
pub mod title_id;
pub mod wizard;

pub use record::BuildRecord;
pub use title_id::TitleId;
