//! Prepares the Tauri context: configuration, icons, permissions.

fn main() {
    println!("cargo:rerun-if-changed=../../../assets/logo/icon.ico");
    tauri_build::build();
}
