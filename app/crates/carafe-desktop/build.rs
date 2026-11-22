//! Prepares the Tauri context: configuration, icons, permissions.

fn main() {
    println!("cargo:rerun-if-changed=../../../assets/logo/icon.ico");
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build();
}
