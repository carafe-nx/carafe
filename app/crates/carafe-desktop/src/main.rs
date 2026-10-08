//! Entry point of the Carafe application.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    carafe_desktop_lib::run();
}
