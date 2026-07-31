const COMMANDS: &[&str] = &[
    "store_session",
    "clear_session",
    "read_session_metadata",
    "initialize_e2ee_store",
    "read_e2ee_store_status",
    "read_encryption_settings",
    "rotate_root_identity",
];

fn main() {
    let attributes = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS));
    tauri_build::try_build(attributes).expect("Tauri application metadata must be valid");
}
