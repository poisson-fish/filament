use std::{collections::BTreeMap, fs, path::PathBuf};

use filament_client_desktop_security::{
    csp_has_forbidden_tokens, validate_desktop_navigation, DesktopCommand, DESKTOP_CSP, WEB_CSP,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct SecurityPolicyFile {
    navigation: NavigationPolicy,
    ipc: IpcPolicy,
    e2ee_media: E2eeMediaPolicy,
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
struct E2eeMediaPolicy {
    backend: String,
    allow_webview_media: bool,
    allow_webview_key_material: bool,
    allow_plaintext_fallback: bool,
    unavailable_behavior: String,
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
    resources: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct WebCspConfig {
    csp: String,
    allowed_url_schemes: Vec<String>,
    forbidden_script_behaviors: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EncodedTransformProbeRecord {
    host_schema_version: u8,
    target: String,
    runtime: String,
    os_version: String,
    runtime_version: String,
    host_version: String,
    host_sdk_version: String,
    shipping_media_path: String,
    probe: EncodedTransformProbe,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EncodedTransformProbe {
    schema_version: u8,
    started_at: String,
    user_agent: String,
    outcome: String,
    features: BTreeMap<String, bool>,
    observed_directions: Vec<String>,
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

    assert_eq!(policy.e2ee_media.backend, "native_livekit_gcm");
    assert!(!policy.e2ee_media.allow_webview_media);
    assert!(!policy.e2ee_media.allow_webview_key_material);
    assert!(!policy.e2ee_media.allow_plaintext_fallback);
    assert_eq!(policy.e2ee_media.unavailable_behavior, "disable_calls");

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
    assert_eq!(
        config
            .bundle
            .resources
            .get("../../THIRD_PARTY_NOTICES.txt")
            .map(String::as_str),
        Some("THIRD_PARTY_NOTICES.txt")
    );

    let notices = fs::read_to_string(root.join("THIRD_PARTY_NOTICES.txt"))
        .expect("third-party notices should exist");
    for component in [
        "hpke-rs 0.7.0",
        "hpke-rs-crypto 0.7.0",
        "hpke-rs-rust-crypto 0.7.0",
    ] {
        assert!(
            notices.contains(component),
            "third-party notices should identify {component}"
        );
    }
    assert!(notices.contains("https://www.mozilla.org/MPL/2.0/"));

    let server_dockerfile = fs::read_to_string(root.join("apps/filament-server/Dockerfile"))
        .expect("server Dockerfile should exist");
    assert!(server_dockerfile
        .contains("COPY THIRD_PARTY_NOTICES.txt /usr/share/doc/filament/THIRD_PARTY_NOTICES.txt"));
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

#[test]
fn encoded_transform_probe_is_local_bounded_and_capture_free() {
    let root = repo_root();
    let probe_root = root.join("spikes/e2ee-webview-check");
    let html = fs::read_to_string(probe_root.join("probe.html"))
        .expect("encoded-transform probe page should exist");
    let script = fs::read_to_string(probe_root.join("probe.js"))
        .expect("encoded-transform probe script should exist");
    let worker = fs::read_to_string(probe_root.join("rtp-transform-worker.js"))
        .expect("encoded-transform probe worker should exist");

    assert!(html.contains("default-src 'none'"));
    assert!(html.contains("script-src 'self'"));
    assert!(html.contains("worker-src 'self'"));
    assert!(!html.contains("http://"));
    assert!(!html.contains("https://"));

    assert!(script.contains("const PROBE_TIMEOUT_MS = 10_000;"));
    assert!(script.contains("navigator.userAgent.slice(0, 1024)"));
    assert!(script.contains("error.message.slice(0, 128)"));
    assert!(script.contains("new RTCPeerConnection({ iceServers: [] })"));
    assert!(!script.contains("getUserMedia"));
    assert!(!script.contains("fetch("));
    assert!(!script.contains("WebSocket"));

    assert!(worker.contains("const MAX_REPORTED_FRAMES = 2;"));
    assert!(worker.contains("controller.enqueue(frame)"));
}

#[test]
fn webview2_probe_host_and_evidence_are_pinned_bounded_and_fail_closed() {
    const MAX_RECORD_BYTES: u64 = 8 * 1024;
    const MAX_METADATA_BYTES: usize = 128;

    let root = repo_root();
    let host_root = root.join("spikes/e2ee-webview-check/hosts/webview2");
    let project = fs::read_to_string(host_root.join("FilamentWebView2Probe.csproj"))
        .expect("WebView2 probe host project should exist");
    let lock = fs::read_to_string(host_root.join("packages.lock.json"))
        .expect("WebView2 probe host lock file should exist");
    let host =
        fs::read_to_string(host_root.join("Program.cs")).expect("WebView2 probe host should exist");

    assert!(project.contains("Microsoft.Web.WebView2\" Version=\"1.0.4078.44\""));
    assert!(project.contains("<RestorePackagesWithLockFile>true"));
    assert!(lock.contains("\"resolved\": \"1.0.4078.44\""));
    assert!(lock.contains("TQkHa/aOHUqFHnJIJ/2ZmJ4nLcQJi0Pc0rj9BAs7SP5sT/"));
    assert!(host.contains("CoreWebView2PermissionState.Deny"));
    assert!(host.contains("CoreWebView2WebResourceContext.All"));
    assert!(host.contains("request.Response = core.Environment.CreateWebResourceResponse"));
    assert!(host.contains("settings.AreDevToolsEnabled = false"));
    assert!(host.contains("shipping_media_path = \"native_livekit_gcm\""));
    assert!(!host.contains("GetUserMedia"));

    let record_path = root.join("spikes/e2ee-webview-check/results/windows-webview2-current.json");
    assert!(
        fs::metadata(&record_path)
            .expect("WebView2 evidence metadata should exist")
            .len()
            <= MAX_RECORD_BYTES
    );
    let record_raw = fs::read_to_string(record_path).expect("WebView2 evidence should exist");
    let record: EncodedTransformProbeRecord =
        serde_json::from_str(&record_raw).expect("WebView2 evidence should be strict JSON");

    assert_eq!(record.host_schema_version, 1);
    assert_eq!(record.target, "windows");
    assert_eq!(record.runtime, "webview2");
    assert_eq!(record.host_version, "webview2-diagnostic-v1");
    assert_eq!(record.host_sdk_version, "1.0.4078.44");
    assert_eq!(record.shipping_media_path, "native_livekit_gcm");
    for metadata in [
        &record.os_version,
        &record.runtime_version,
        &record.host_version,
        &record.host_sdk_version,
    ] {
        assert!(!metadata.is_empty());
        assert!(metadata.len() <= MAX_METADATA_BYTES);
        assert!(!metadata.chars().any(char::is_control));
    }

    let probe = record.probe;
    assert_eq!(probe.schema_version, 1);
    assert_eq!(probe.outcome, "supported");
    assert!(probe.started_at.ends_with('Z'));
    assert!(probe.started_at.len() <= MAX_METADATA_BYTES);
    assert!(!probe.user_agent.is_empty());
    assert!(probe.user_agent.len() <= 1024);
    let expected_features = BTreeMap::from([
        ("peer_connection".to_owned(), true),
        ("receiver_transform".to_owned(), true),
        ("script_transform".to_owned(), true),
        ("secure_context".to_owned(), true),
        ("sender_transform".to_owned(), true),
        ("worker".to_owned(), true),
    ]);
    assert_eq!(probe.features, expected_features);
    assert_eq!(probe.observed_directions, ["receiver", "sender"]);
}

#[test]
fn wkwebview_probe_host_and_evidence_are_bounded_and_fail_closed() {
    const MAX_RECORD_BYTES: u64 = 8 * 1024;
    const MAX_METADATA_BYTES: usize = 128;

    let root = repo_root();
    let host_root = root.join("spikes/e2ee-webview-check/hosts/wkwebview");
    let package = fs::read_to_string(host_root.join("Package.swift"))
        .expect("WKWebView probe host package should exist");
    let host = fs::read_to_string(host_root.join("Sources/FilamentWKWebViewProbe/main.swift"))
        .expect("WKWebView probe host should exist");

    assert!(package.contains(".executableTarget(name: \"FilamentWKWebViewProbe\")"));
    assert!(!package.contains("dependencies:"));
    assert!(host
        .contains("parameters.requiredLocalEndpoint = .hostPort(host: \"127.0.0.1\", port: .any)"));
    assert!(host.contains("guard Self.isLoopback(connection.endpoint)"));
    assert!(host.contains("configuration.websiteDataStore = .nonPersistent()"));
    assert!(host.contains("requestMediaCapturePermissionFor"));
    assert!(host.contains("decisionHandler(.deny)"));
    assert!(host.contains("navigationAction.request.url"));
    assert!(host.contains("shipping_media_path\": \"native_livekit_gcm\""));
    assert!(!host.contains("getUserMedia"));

    let record_path = root.join("spikes/e2ee-webview-check/results/macos-wkwebview-current.json");
    assert!(
        fs::metadata(&record_path)
            .expect("WKWebView evidence metadata should exist")
            .len()
            <= MAX_RECORD_BYTES
    );
    let record_raw = fs::read_to_string(record_path).expect("WKWebView evidence should exist");
    let record: EncodedTransformProbeRecord =
        serde_json::from_str(&record_raw).expect("WKWebView evidence should be strict JSON");

    assert_eq!(record.host_schema_version, 1);
    assert_eq!(record.target, "macos");
    assert_eq!(record.runtime, "wkwebview");
    assert_eq!(record.host_version, "wkwebview-diagnostic-v1");
    assert_eq!(record.host_sdk_version, "system-webkit");
    assert_eq!(record.shipping_media_path, "native_livekit_gcm");
    for metadata in [
        &record.os_version,
        &record.runtime_version,
        &record.host_version,
        &record.host_sdk_version,
    ] {
        assert!(!metadata.is_empty());
        assert!(metadata.len() <= MAX_METADATA_BYTES);
        assert!(!metadata.chars().any(char::is_control));
    }

    let probe = record.probe;
    assert_eq!(probe.schema_version, 1);
    assert_eq!(probe.outcome, "supported");
    assert!(probe.started_at.ends_with('Z'));
    assert!(probe.started_at.len() <= MAX_METADATA_BYTES);
    assert!(!probe.user_agent.is_empty());
    assert!(probe.user_agent.len() <= 1024);
    let expected_features = BTreeMap::from([
        ("peer_connection".to_owned(), true),
        ("receiver_transform".to_owned(), true),
        ("script_transform".to_owned(), true),
        ("secure_context".to_owned(), true),
        ("sender_transform".to_owned(), true),
        ("worker".to_owned(), true),
    ]);
    assert_eq!(probe.features, expected_features);
    assert_eq!(probe.observed_directions, ["receiver", "sender"]);
}

#[test]
fn macos_native_media_link_retains_libwebrtc_objective_c_categories() {
    let root = repo_root();
    let cargo_config = fs::read_to_string(root.join(".cargo/config.toml"))
        .expect("workspace Cargo config should exist");

    assert!(cargo_config.contains("[target.'cfg(target_os = \"macos\")']"));
    assert!(cargo_config.contains("rustflags = [\"-C\", \"link-arg=-ObjC\"]"));
    assert!(!cargo_config.contains("all_load"));
    assert!(!cargo_config.contains("force_load"));
}
