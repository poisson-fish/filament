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
#[serde(deny_unknown_fields)]
struct PlatformSupportContract {
    schema_version: u8,
    reviewed_at: String,
    review_interval_days: u16,
    support_policy: PlatformSupportPolicy,
    runtime: PackagedRuntime,
    targets: Vec<PackagedTarget>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlatformSupportPolicy {
    client_release_support_months: u8,
    minimum_os_change_notice_days: u16,
    vendor_security_support_required: bool,
    remote_application_code_allowed: bool,
    automatic_updates_enabled: bool,
    media_default: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackagedRuntime {
    ui: String,
    adapter: String,
    status: String,
    reviewed_version: String,
    accepted_risks: Vec<String>,
    exception_review_due: String,
    patchable_advisories_allowed: bool,
    unsafe_ffi_fallback_allowed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackagedTarget {
    target: String,
    requirement: String,
    minimum_os: String,
    architectures: Vec<String>,
    package_formats: Vec<String>,
    key_custody: String,
    webview: String,
    media: String,
}

#[derive(Debug, Deserialize)]
struct DesktopTauriConfig {
    identifier: String,
    build: DesktopBuild,
    app: DesktopApp,
    bundle: DesktopBundle,
}

#[derive(Debug, Deserialize)]
struct DesktopBuild {
    #[serde(rename = "frontendDist")]
    frontend_dist: String,
    #[serde(rename = "devUrl")]
    dev_url: String,
}

#[derive(Debug, Deserialize)]
struct DesktopApp {
    windows: Vec<DesktopWindow>,
    security: DesktopSecurity,
}

#[derive(Debug, Deserialize)]
struct DesktopWindow {
    label: String,
    url: String,
    devtools: bool,
    #[serde(rename = "dragDropEnabled")]
    drag_drop_enabled: bool,
    #[serde(rename = "useHttpsScheme")]
    use_https_scheme: bool,
}

#[derive(Debug, Deserialize)]
struct DesktopSecurity {
    #[serde(rename = "freezePrototype")]
    freeze_prototype: bool,
    #[serde(rename = "dangerousDisableAssetCspModification")]
    dangerous_disable_asset_csp_modification: bool,
    capabilities: Vec<String>,
    csp: String,
}

#[derive(Debug, Deserialize)]
struct DesktopBundle {
    #[serde(rename = "createUpdaterArtifacts")]
    create_updater_artifacts: bool,
    android: AndroidBundle,
    #[serde(rename = "iOS")]
    ios: IosBundle,
    resources: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct AndroidBundle {
    #[serde(rename = "minSdkVersion")]
    min_sdk_version: u8,
}

#[derive(Debug, Deserialize)]
struct IosBundle {
    #[serde(rename = "minimumSystemVersion")]
    minimum_system_version: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DesktopCapability {
    #[serde(rename = "$schema")]
    schema: String,
    identifier: String,
    description: String,
    windows: Vec<String>,
    permissions: Vec<String>,
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
    #[serde(default)]
    gstreamer_version: Option<String>,
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
fn packaged_platform_contract_is_explicit_and_fail_closed() {
    let root = repo_root();
    let raw = fs::read_to_string(root.join("apps/filament-client-desktop/platform-support.json"))
        .expect("platform support contract should exist");
    let contract: PlatformSupportContract =
        serde_json::from_str(&raw).expect("platform support contract should be strict JSON");

    assert_eq!(contract.schema_version, 2);
    assert_eq!(contract.reviewed_at, "2026-07-23");
    assert!(contract.review_interval_days <= 180);
    assert_eq!(contract.support_policy.client_release_support_months, 12);
    assert!(contract.support_policy.minimum_os_change_notice_days >= 180);
    assert!(contract.support_policy.vendor_security_support_required);
    assert!(!contract.support_policy.remote_application_code_allowed);
    assert!(!contract.support_policy.automatic_updates_enabled);
    assert_eq!(
        contract.support_policy.media_default,
        "disabled_until_packaged_probe_passes"
    );

    assert_eq!(contract.runtime.ui, "bundled_solidjs");
    assert_eq!(contract.runtime.adapter, "tauri_v2_desktop_and_mobile");
    assert_eq!(
        contract.runtime.status,
        "desktop_durable_mls_mailbox_root_rotation_fresh_device_enrollment_sqlcipher_session_install_offline_launch_mobile_packages_ci_gated_fail_closed"
    );
    assert_eq!(contract.runtime.reviewed_version, "2.11.5");
    assert_eq!(
        contract.runtime.accepted_risks,
        [
            "scoped_mpl_transitives",
            "scoped_llvm_exception_transitive",
            "unmaintained_transitives",
            "glib_0.18.5_unsoundness"
        ]
    );
    assert_eq!(contract.runtime.exception_review_due, "2027-01-18");
    assert!(!contract.runtime.patchable_advisories_allowed);
    assert!(!contract.runtime.unsafe_ffi_fallback_allowed);

    let targets: BTreeMap<&str, &PackagedTarget> = contract
        .targets
        .iter()
        .map(|target| (target.target.as_str(), target))
        .collect();
    assert_eq!(targets.len(), contract.targets.len());
    assert_eq!(
        targets.keys().copied().collect::<Vec<_>>(),
        ["android", "ios", "linux", "macos", "windows"]
    );

    for required in ["linux", "macos", "windows", "android"] {
        assert_eq!(targets[required].requirement, "required");
    }
    assert_eq!(targets["ios"].requirement, "feasibility_gated");

    assert_eq!(targets["linux"].minimum_os, "ubuntu_24_04_lts");
    assert_eq!(targets["linux"].architectures, ["x86_64"]);
    assert_eq!(targets["linux"].package_formats, ["deb", "appimage"]);
    assert_eq!(targets["linux"].key_custody, "secret_service_fail_closed");

    assert_eq!(targets["macos"].minimum_os, "15.0");
    assert_eq!(targets["macos"].architectures, ["aarch64", "x86_64"]);
    assert_eq!(targets["macos"].package_formats, ["app", "dmg"]);
    assert_eq!(targets["macos"].key_custody, "keychain");

    assert_eq!(targets["windows"].minimum_os, "windows_11_vendor_supported");
    assert_eq!(targets["windows"].architectures, ["x86_64"]);
    assert_eq!(targets["windows"].package_formats, ["msi"]);
    assert_eq!(targets["windows"].key_custody, "credential_manager");
    assert!(targets["windows"]
        .webview
        .starts_with("webview2_evergreen_"));

    assert_eq!(targets["android"].minimum_os, "api_33");
    assert_eq!(targets["android"].architectures, ["aarch64"]);
    assert_eq!(targets["android"].package_formats, ["apk", "aab"]);
    assert_eq!(targets["android"].key_custody, "android_keystore");

    assert_eq!(targets["ios"].minimum_os, "17.0");
    assert_eq!(
        targets["ios"].architectures,
        ["aarch64", "aarch64_simulator"]
    );
    assert_eq!(targets["ios"].package_formats, ["app", "ipa"]);
    assert_eq!(targets["ios"].key_custody, "keychain");

    for target in targets.values() {
        assert_eq!(target.media, "disabled_until_packaged_probe_passes");
        assert!(!target.webview.is_empty());
    }
}

#[test]
fn tauri_policy_exceptions_are_exact_and_keep_patchable_findings_denied() {
    let root = repo_root();
    let policy =
        fs::read_to_string(root.join("cargo-deny.toml")).expect("cargo deny policy should exist");

    for exact_license_exception in [
        "cssparser:0.36.0",
        "cssparser-macros:0.6.1",
        "dtoa-short:0.3.5",
        "option-ext:0.2.0",
        "selectors:0.36.1",
        "target-lexicon:0.12.16",
    ] {
        assert_eq!(
            policy.matches(exact_license_exception).count(),
            1,
            "Tauri license exception must be exact and unique: {exact_license_exception}"
        );
    }

    for accepted_advisory in [
        "RUSTSEC-2024-0370",
        "RUSTSEC-2024-0411",
        "RUSTSEC-2024-0412",
        "RUSTSEC-2024-0413",
        "RUSTSEC-2024-0414",
        "RUSTSEC-2024-0415",
        "RUSTSEC-2024-0416",
        "RUSTSEC-2024-0417",
        "RUSTSEC-2024-0418",
        "RUSTSEC-2024-0419",
        "RUSTSEC-2024-0420",
        "RUSTSEC-2024-0429",
        "RUSTSEC-2024-0436",
        "RUSTSEC-2025-0075",
        "RUSTSEC-2025-0080",
        "RUSTSEC-2025-0081",
        "RUSTSEC-2025-0098",
        "RUSTSEC-2025-0100",
    ] {
        assert_eq!(
            policy.matches(accepted_advisory).count(),
            1,
            "Tauri advisory exception must be explicit: {accepted_advisory}"
        );
    }

    for patchable_advisory in [
        "RUSTSEC-2026-0009",
        "RUSTSEC-2026-0190",
        "RUSTSEC-2026-0194",
        "RUSTSEC-2026-0195",
    ] {
        assert!(
            !policy.contains(patchable_advisory),
            "patchable Tauri advisory must remain denied: {patchable_advisory}"
        );
    }
}

#[test]
fn tauri_config_enforces_hardening_controls() {
    let root = repo_root();
    let desktop_root = root.join("apps/filament-client-desktop");
    let tauri_root = desktop_root.join("src-tauri");
    let raw =
        fs::read_to_string(tauri_root.join("tauri.conf.json")).expect("tauri config should exist");
    let config: DesktopTauriConfig = serde_json::from_str(&raw).expect("tauri config should parse");

    assert_eq!(config.identifier, "com.filament.desktop");
    assert_eq!(config.build.frontend_dist, "../../filament-client-web/dist");
    assert_eq!(config.build.dev_url, "https://app.filament.local");
    assert_eq!(config.app.windows.len(), 1);
    let window = &config.app.windows[0];
    assert_eq!(window.label, "main");
    assert_eq!(window.url, "index.html");
    assert!(!window.devtools);
    assert!(!window.drag_drop_enabled);
    assert!(window.use_https_scheme);
    assert!(config.app.security.freeze_prototype);
    assert!(!config.app.security.dangerous_disable_asset_csp_modification);
    assert_eq!(config.app.security.capabilities, ["main"]);
    assert_eq!(config.app.security.csp, DESKTOP_CSP);
    assert!(!csp_has_forbidden_tokens(&config.app.security.csp));
    assert!(!config.bundle.create_updater_artifacts);
    assert_eq!(config.bundle.android.min_sdk_version, 33);
    assert_eq!(config.bundle.ios.minimum_system_version, "17.0");
    assert_eq!(
        config
            .bundle
            .resources
            .get("../../../THIRD_PARTY_NOTICES.txt")
            .map(String::as_str),
        Some("THIRD_PARTY_NOTICES.txt")
    );

    let capability_raw = fs::read_to_string(tauri_root.join("capabilities/main.json"))
        .expect("main capability should exist");
    let capability: DesktopCapability =
        serde_json::from_str(&capability_raw).expect("main capability should be strict JSON");
    assert_eq!(capability.schema, "../gen/schemas/desktop-schema.json");
    assert_eq!(capability.identifier, "main");
    assert!(!capability.description.is_empty());
    assert_eq!(capability.windows, ["main"]);
    assert_eq!(
        capability.permissions,
        DesktopCommand::all()
            .iter()
            .map(|command| format!("allow-{command}").replace('_', "-"))
            .collect::<Vec<_>>()
    );

    let build_script = fs::read_to_string(tauri_root.join("build.rs"))
        .expect("Tauri ACL build script should exist");
    for command in DesktopCommand::all() {
        assert_eq!(
            build_script.matches(&format!("\"{command}\"")).count(),
            1,
            "Tauri ACL manifest should contain exactly one {command} command"
        );
    }
    assert!(build_script.contains("AppManifest::new().commands(COMMANDS)"));

    let cargo_manifest = fs::read_to_string(tauri_root.join("Cargo.toml"))
        .expect("packaged-client Cargo manifest should exist");
    assert!(cargo_manifest.contains("tauri = { version = \"=2.11.5\", features = [] }"));
    assert!(cargo_manifest.contains("tauri-build = { version = \"=2.6.3\", features = [] }"));

    let package_raw = fs::read_to_string(desktop_root.join("package.json"))
        .expect("packaged-client npm manifest should exist");
    let package: serde_json::Value =
        serde_json::from_str(&package_raw).expect("packaged-client npm manifest should parse");
    assert_eq!(
        package["devDependencies"]["@tauri-apps/cli"],
        serde_json::Value::String("2.11.4".to_owned())
    );
    let notices = fs::read_to_string(root.join("THIRD_PARTY_NOTICES.txt"))
        .expect("third-party notices should exist");
    for component in [
        "hpke-rs 0.7.0",
        "hpke-rs-crypto 0.7.0",
        "hpke-rs-rust-crypto 0.7.0",
        "cssparser 0.36.0",
        "cssparser-macros 0.6.1",
        "dtoa-short 0.3.5",
        "option-ext 0.2.0",
        "selectors 0.36.1",
        "target-lexicon 0.12.16",
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
fn packaged_artifact_gates_cover_the_reviewed_initial_matrix() {
    let root = repo_root();
    let desktop_root = root.join("apps/filament-client-desktop");
    let package_raw = fs::read_to_string(desktop_root.join("package.json"))
        .expect("packaged-client npm manifest should exist");
    let package: serde_json::Value =
        serde_json::from_str(&package_raw).expect("packaged-client npm manifest should parse");
    assert_eq!(
        package["scripts"]["test:package-policy"],
        serde_json::Value::String(
            "node --test tests/package-artifacts.test.mjs tests/desktop-package-smoke.test.mjs"
                .to_owned()
        )
    );
    assert_eq!(
        package["scripts"]["verify:package"],
        serde_json::Value::String("node tools/verify-package.mjs".to_owned())
    );
    let verifier = fs::read_to_string(desktop_root.join("tools/verify-package.mjs"))
        .expect("packaged-client artifact verifier should exist");
    for required_control in [
        "MAX_WEB_BUNDLE_FILES",
        "MAX_WEB_BUNDLE_BYTES",
        "MAX_WEB_ASSET_BYTES",
        "MAX_INDEX_HTML_BYTES",
        "MAX_ARTIFACT_BYTES",
        "symbolic links are forbidden",
        "remote_application_code_allowed !== false",
        "createUpdaterArtifacts !== false",
        "THIRD_PARTY_NOTICES.txt",
    ] {
        assert!(
            verifier.contains(required_control),
            "artifact verifier should retain {required_control}"
        );
    }

    let workflow = fs::read_to_string(root.join(".github/workflows/ci.yml"))
        .expect("CI workflow should exist");
    for required_job in [
        "packaged-client-linux:",
        "packaged-client-macos:",
        "packaged-client-windows:",
        "packaged-client-android:",
        "packaged-client-ios:",
    ] {
        assert!(
            workflow.contains(required_job),
            "CI should retain {required_job}"
        );
    }
    for required_runner in ["ubuntu-24.04", "macos-15", "macos-15-intel", "windows-2025"] {
        assert!(
            workflow.contains(required_runner),
            "CI should retain package runner {required_runner}"
        );
    }
    for required_bundle in [
        "--bundles deb,appimage",
        "--bundles app --ci --no-sign",
        "hdiutil create",
        "--bundles msi",
        "--target aarch64 --apk --aab --ci",
        "--target aarch64-sim --debug --ci --no-sign",
    ] {
        assert!(
            workflow.contains(required_bundle),
            "CI should retain package format gate {required_bundle}"
        );
    }
    for required_ios_control in [
        "aarch64-apple-ios-sim",
        "xcrun --sdk iphoneos --show-sdk-path",
        "xcrun --sdk iphonesimulator --show-sdk-path",
        "IPHONEOS_DEPLOYMENT_TARGET = 17.0",
        "--platform ios",
        "--architecture aarch64_simulator",
    ] {
        assert!(
            workflow.contains(required_ios_control),
            "CI should retain iOS gate {required_ios_control}"
        );
    }
    assert_eq!(workflow.matches("run verify:package --").count(), 5);
    assert_eq!(workflow.matches("SHA256SUMS").count(), 10);
}

#[test]
fn desktop_package_offline_launch_gate_is_bounded() {
    let root = repo_root();
    let desktop_root = root.join("apps/filament-client-desktop");
    let package_raw = fs::read_to_string(desktop_root.join("package.json"))
        .expect("packaged-client npm manifest should exist");
    let package: serde_json::Value =
        serde_json::from_str(&package_raw).expect("packaged-client npm manifest should parse");
    assert_eq!(
        package["scripts"]["smoke:desktop"],
        serde_json::Value::String("node tools/smoke-desktop-package.mjs".to_owned())
    );

    let smoke = fs::read_to_string(desktop_root.join("tools/smoke-desktop-package.mjs"))
        .expect("desktop package smoke verifier should exist");
    for required_control in [
        "MAX_CAPTURE_BYTES",
        "MAX_OBSERVATION_MS",
        "ENVIRONMENT_ALLOWLIST",
        "HTTP_PROXY: \"http://127.0.0.1:9\"",
        "desktop package opened a network socket during offline launch",
        "desktop package exited before the offline launch observation completed",
        "offline_bundle_launch: true",
    ] {
        assert!(
            smoke.contains(required_control),
            "desktop smoke verifier should retain {required_control}"
        );
    }

    let workflow = fs::read_to_string(root.join(".github/workflows/ci.yml"))
        .expect("CI workflow should exist");
    for required_desktop_smoke in [
        "Install and launch Debian package offline",
        "Launch AppImage offline",
        "Mount packaged disk image",
        "Launch packaged macOS app offline",
        "Install Windows package",
        "Launch installed Windows app offline",
        "run smoke:desktop --",
        "deb-launch.json",
        "appimage-launch.json",
        "dmg-launch.json",
        "msi-launch.json",
    ] {
        assert!(
            workflow.contains(required_desktop_smoke),
            "CI should retain desktop offline launch gate {required_desktop_smoke}"
        );
    }
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
fn webkitgtk_probe_host_and_unsupported_evidence_are_bounded_and_fail_closed() {
    const MAX_RECORD_BYTES: u64 = 8 * 1024;
    const MAX_METADATA_BYTES: usize = 128;

    let root = repo_root();
    let host_root = root.join("spikes/e2ee-webview-check/hosts/webkitgtk");
    let source =
        fs::read_to_string(host_root.join("main.c")).expect("WebKitGTK probe host should exist");
    let makefile = fs::read_to_string(host_root.join("Makefile"))
        .expect("WebKitGTK probe Makefile should exist");
    let container = fs::read_to_string(host_root.join("Dockerfile.ubuntu-24.04"))
        .expect("WebKitGTK probe container should exist");
    let runner = fs::read_to_string(host_root.join("run-ubuntu-24.04.sh"))
        .expect("WebKitGTK probe runner should exist");

    assert!(makefile.contains("-Wall -Wextra -Werror"));
    assert!(source.contains("O_NOFOLLOW"));
    assert!(source.contains("MAX_ASSET_BYTES = 64 * 1024"));
    assert!(source.contains("webkit_network_session_new_ephemeral"));
    assert!(source.contains("webkit_settings_set_enable_webrtc(settings, TRUE)"));
    assert!(source.contains("webkit_permission_request_deny"));
    assert!(source.contains("webkit_policy_decision_ignore"));
    assert!(source.contains("webkit_security_manager_register_uri_scheme_as_secure"));
    assert!(source.contains("default-src 'none'; script-src 'self'; worker-src 'self'"));
    assert!(source.contains("shipping_media_path\\\": \\\"native_livekit_gcm"));
    assert!(source.contains("F_DUPFD_CLOEXEC"));
    assert!(!source.contains("getUserMedia"));

    assert!(container.contains("FROM ubuntu:24.04@sha256:"));
    assert!(container.contains("libwebkitgtk-6.0-dev"));
    assert!(container.contains("GST_AUDIO_SINK=fakesink"));
    assert!(container.contains("FILAMENT_PROBE_OUTPUT_FD=3"));
    assert!(runner.contains("--network none"));
    assert!(runner.contains("--memory 2g"));
    assert!(runner.contains("--pids-limit 512"));
    assert!(!runner.contains("WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS"));

    let record_path =
        root.join("spikes/e2ee-webview-check/results/linux-webkitgtk-ubuntu-24.04-current.json");
    assert!(
        fs::metadata(&record_path)
            .expect("WebKitGTK evidence metadata should exist")
            .len()
            <= MAX_RECORD_BYTES
    );
    let record_raw = fs::read_to_string(record_path).expect("WebKitGTK evidence should exist");
    let record: EncodedTransformProbeRecord =
        serde_json::from_str(&record_raw).expect("WebKitGTK evidence should be strict JSON");

    assert_eq!(record.host_schema_version, 1);
    assert_eq!(record.target, "linux");
    assert_eq!(record.runtime, "webkitgtk");
    assert_eq!(record.host_version, "webkitgtk-diagnostic-v1");
    assert_eq!(record.host_sdk_version, record.runtime_version);
    assert_eq!(record.shipping_media_path, "native_livekit_gcm");
    let gstreamer = record
        .gstreamer_version
        .as_deref()
        .expect("Linux evidence should pin GStreamer");
    for metadata in [
        record.os_version.as_str(),
        record.runtime_version.as_str(),
        record.host_version.as_str(),
        record.host_sdk_version.as_str(),
        gstreamer,
    ] {
        assert!(!metadata.is_empty());
        assert!(metadata.len() <= MAX_METADATA_BYTES);
        assert!(!metadata.chars().any(char::is_control));
    }

    let probe = record.probe;
    assert_eq!(probe.schema_version, 1);
    assert_eq!(probe.outcome, "unsupported");
    assert!(probe.started_at.ends_with('Z'));
    assert!(probe.started_at.len() <= MAX_METADATA_BYTES);
    assert!(!probe.user_agent.is_empty());
    assert!(probe.user_agent.len() <= 1024);
    assert_eq!(
        probe.features,
        BTreeMap::from([
            ("peer_connection".to_owned(), false),
            ("receiver_transform".to_owned(), false),
            ("script_transform".to_owned(), false),
            ("secure_context".to_owned(), true),
            ("sender_transform".to_owned(), false),
            ("worker".to_owned(), true),
        ])
    );
    assert!(probe.observed_directions.is_empty());
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
