// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Embed Info.plist into the Mach-O so macOS Dock/menu can read CFBundleDisplayName
// during `tauri dev` (unbundled binary). Cargo package names must stay lowercase.
#[cfg(target_os = "macos")]
embed_plist::embed_info_plist!("../Info.plist");

fn main() {
    coreside_lib::run();
}
