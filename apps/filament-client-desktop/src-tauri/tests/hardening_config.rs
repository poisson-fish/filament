use std::{fs, path::PathBuf};

use filament_client_desktop_security::{
    csp_has_forbidden_tokens, validate_desktop_navigation, DesktopCommand, DESKTOP_CSP, WEB_CSP,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct SecurityPolicyFile {
    navigation: NavigationPolicy,
    ipc: IpcPolicy,
    updates: UpdatePolicy,
}

#[derive(Debug, Deserialize)]
struct NavigationPolicy {
    allow: Vec<String>,
    deny_remote_http: bool,
    deny_remote_https_hosts: bool,
}

#[derive(Debug, Deserialize)]
struct IpcPolicy {
    allowed_commands: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct UpdatePolicy {
    signed_only: bool,
}

#[derive(Debug, Deserialize)]
struct DesktopTauriConfig {
    app: DesktopApp,
    bundle: DesktopBundle,
}

#[derive(Debug, Deserialize)]
struct DesktopApp {
    security: DesktopSecurity,
}

#[derive(Debug, Deserialize)]
struct DesktopSecurity {
    #[serde(rename = "freezePrototype")]
    freeze_prototype: bool,
    #[serde(rename = "dangerousDisableAssetCspModification")]
    dangerous_disable_asset_csp_modification: bool,
    csp: String,
}

#[derive(Debug, Deserialize)]
struct DesktopBundle {
    #[serde(rename = "createUpdaterArtifacts")]
    create_updater_artifacts: bool,
}

#[derive(Debug, Deserialize)]
struct WebCspConfig {
    csp: String,
    allowed_url_schemes: Vec<String>,
    forbidden_script_behaviors: Vec<String>,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repo root should resolve")
}

#[test]
fn desktop_security_policy_is_strict() {
    let root = repo_root();
    let raw = fs::read_to_string(root.join("apps/filament-client-desktop/security-policy.json"))
        .expect("security policy should exist");
    let policy: SecurityPolicyFile =
        serde_json::from_str(&raw).expect("security policy should be valid json");

    assert!(policy.navigation.deny_remote_http);
    assert!(policy.navigation.deny_remote_https_hosts);
    assert_eq!(policy.navigation.allow.len(), 2);
    for allowed in &policy.navigation.allow {
        assert!(
            validate_desktop_navigation(allowed).is_ok(),
            "allowed navigation entry should pass policy: {allowed}"
        );
    }

    let allowed_commands: Vec<String> = DesktopCommand::all()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(policy.ipc.allowed_commands, allowed_commands);

    assert!(policy.updates.signed_only);
}

#[test]
fn tauri_config_enforces_hardening_controls() {
    let root = repo_root();
    let raw = fs::read_to_string(root.join("apps/filament-client-desktop/tauri.conf.json"))
        .expect("tauri config should exist");
    let config: DesktopTauriConfig = serde_json::from_str(&raw).expect("tauri config should parse");

    assert!(config.app.security.freeze_prototype);
    assert!(!config.app.security.dangerous_disable_asset_csp_modification);
    assert_eq!(config.app.security.csp, DESKTOP_CSP);
    assert!(!csp_has_forbidden_tokens(&config.app.security.csp));
    assert!(config.bundle.create_updater_artifacts);
}

#[test]
fn web_csp_baseline_stays_locked_down() {
    let root = repo_root();
    let raw = fs::read_to_string(root.join("apps/filament-client-web/security/csp.json"))
        .expect("web csp config should exist");
    let config: WebCspConfig = serde_json::from_str(&raw).expect("web csp config should parse");

    assert_eq!(config.csp, WEB_CSP);
    assert!(!csp_has_forbidden_tokens(&config.csp));
    assert_eq!(config.allowed_url_schemes, vec!["https", "wss"]);
    assert_eq!(
        config.forbidden_script_behaviors,
        vec!["eval", "new Function", "inline-script"]
    );
}
