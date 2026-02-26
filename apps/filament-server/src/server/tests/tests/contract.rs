use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use filament_protocol::gateway_event_manifest;

use super::*;

const HTTP_METHODS: [&str; 5] = ["GET", "POST", "PATCH", "PUT", "DELETE"];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read_doc(path: &str) -> String {
    std::fs::read_to_string(repo_root().join(path))
        .unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

fn extract_backtick_tokens(line: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find('`') {
        let after_start = &rest[start + 1..];
        if let Some(end) = after_start.find('`') {
            tokens.push(&after_start[..end]);
            rest = &after_start[end + 1..];
            continue;
        }
        break;
    }
    tokens
}

fn parse_documented_routes(api_doc: &str) -> BTreeSet<(String, String)> {
    let mut routes = BTreeSet::new();
    for line in api_doc.lines() {
        for token in extract_backtick_tokens(line) {
            let Some((method, path)) = token.split_once(' ') else {
                continue;
            };
            if !HTTP_METHODS.contains(&method) || !path.starts_with('/') {
                continue;
            }
            let normalized_path = path.split('?').next().unwrap_or(path);
            routes.insert((method.to_owned(), normalized_path.to_owned()));
        }
    }
    routes
}

fn parse_documented_gateway_events(gateway_doc: &str) -> BTreeSet<String> {
    gateway_doc
        .lines()
        .filter(|line| !line.contains("(planned)"))
        .filter_map(|line| line.strip_prefix("#### `"))
        .filter_map(|line| line.split('`').next())
        .filter(|event| {
            !event.is_empty()
                && event.chars().all(|character| {
                    character.is_ascii_lowercase()
                        || character.is_ascii_digit()
                        || character == '_'
                        || character == '.'
                })
        })
        .map(ToOwned::to_owned)
        .collect()
}

fn parse_client_override_migration_decode_set(client_source: &str) -> BTreeSet<String> {
    let mut decoded = BTreeSet::new();
    for line in client_source.lines() {
        if !line.contains("workspace_channel_") || !line.contains("_update") {
            continue;
        }
        for token in line.split('"') {
            if token.starts_with("workspace_channel_") && token.ends_with("_update") {
                decoded.insert(token.to_owned());
            }
        }
    }
    decoded
}

fn parse_quoted_tokens(source: &str) -> BTreeSet<String> {
    let mut tokens = BTreeSet::new();
    let mut quote: Option<char> = None;
    let mut start = 0usize;
    for (index, ch) in source.char_indices() {
        match quote {
            None => {
                if ch == '"' || ch == '\'' {
                    quote = Some(ch);
                    start = index + ch.len_utf8();
                }
            }
            Some(active_quote) => {
                if ch == active_quote {
                    let token = &source[start..index];
                    tokens.insert(token.to_owned());
                    quote = None;
                }
            }
        }
    }
    tokens
}

fn collect_gateway_client_sources(root: &Path, collected: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(root)
        .unwrap_or_else(|error| panic!("failed to read directory {}: {error}", root.display()));

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| {
            panic!("failed to enumerate directory {}: {error}", root.display());
        });
        let path = entry.path();
        if path.is_dir() {
            collect_gateway_client_sources(&path, collected);
            continue;
        }

        let file_name = path.file_name().and_then(|name| name.to_str());
        if let Some(file_name) = file_name {
            if file_name.starts_with("gateway-")
                && Path::new(file_name)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("ts"))
            {
                collected.push(path);
            }
        }
    }
}

fn parse_client_gateway_event_literal_set() -> BTreeSet<String> {
    let mut paths = Vec::new();
    let root = repo_root().join("apps/filament-client-web/src/lib");
    collect_gateway_client_sources(&root, &mut paths);

    let mut tokens = BTreeSet::new();
    for path in paths {
        let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!(
                "failed to read client gateway source {}: {error}",
                path.display()
            );
        });
        tokens.extend(parse_quoted_tokens(&source));
    }

    tokens
}

#[test]
fn api_docs_cover_router_manifest_routes() {
    let documented = parse_documented_routes(&read_doc("docs/API.md"));
    let mut undocumented = Vec::new();
    for (method, path) in ROUTE_MANIFEST {
        if !documented.contains(&(String::from(*method), String::from(*path))) {
            undocumented.push(format!("{method} {path}"));
        }
    }

    assert!(
        undocumented.is_empty(),
        "routes present in router manifest but missing in docs/API.md: {}",
        undocumented.join(", ")
    );
}

#[test]
fn gateway_event_manifest_is_aligned_across_server_and_docs() {
    let manifest_events: BTreeSet<String> = gateway_event_manifest()
        .events
        .iter()
        .map(|entry| entry.event_type.clone())
        .collect();
    assert!(
        !manifest_events.is_empty(),
        "gateway event manifest unexpectedly empty"
    );

    let emitted_events: BTreeSet<String> = gateway_events::EMITTED_EVENT_TYPES
        .iter()
        .map(|event| (*event).to_owned())
        .collect();
    assert_eq!(
        emitted_events, manifest_events,
        "server emitted event set drifted from protocol gateway event manifest"
    );

    let documented = parse_documented_gateway_events(&read_doc("docs/GATEWAY_EVENTS.md"));
    assert_eq!(
        documented, manifest_events,
        "docs/GATEWAY_EVENTS.md documented event set drifted from protocol gateway event manifest (planned events excluded)"
    );
}

#[test]
fn gateway_docs_capture_override_migration_contract() {
    let gateway_doc = read_doc("docs/GATEWAY_EVENTS.md");
    let documented = parse_documented_gateway_events(&gateway_doc);

    for required_event in [
        "workspace_channel_override_update",
        "workspace_channel_role_override_update",
        "workspace_channel_permission_override_update",
    ] {
        assert!(
            documented.contains(required_event),
            "docs/GATEWAY_EVENTS.md must document override migration event `{required_event}`"
        );
    }

    assert!(
        gateway_doc.contains("legacy migration event"),
        "docs/GATEWAY_EVENTS.md must mark the legacy override event as migration-only"
    );
}

#[test]
fn override_migration_event_set_is_aligned_across_server_docs_and_client_decoder() {
    let required_override_events = BTreeSet::from([
        String::from("workspace_channel_override_update"),
        String::from("workspace_channel_role_override_update"),
        String::from("workspace_channel_permission_override_update"),
    ]);

    let server_emitted: BTreeSet<String> = gateway_events::EMITTED_EVENT_TYPES
        .iter()
        .filter(|event| event.starts_with("workspace_channel_") && event.ends_with("_update"))
        .map(|event| (*event).to_owned())
        .collect();
    assert_eq!(
        server_emitted, required_override_events,
        "server emitted override migration event set drifted"
    );

    let documented = parse_documented_gateway_events(&read_doc("docs/GATEWAY_EVENTS.md"));
    let documented_required: BTreeSet<String> = documented
        .into_iter()
        .filter(|event| required_override_events.contains(event))
        .collect();
    assert_eq!(
        documented_required, required_override_events,
        "docs/GATEWAY_EVENTS.md override migration event set drifted"
    );

    let client_decode_set = parse_client_override_migration_decode_set(&read_doc(
        "apps/filament-client-web/src/lib/gateway-workspace-channel-override-events.ts",
    ));
    assert_eq!(
        client_decode_set, required_override_events,
        "client override decoder event set drifted"
    );
}

#[test]
fn emitted_domain_event_manifest_is_aligned_across_server_docs_and_client() {
    let manifest_events: BTreeSet<String> = gateway_event_manifest()
        .events
        .iter()
        .map(|entry| entry.event_type.clone())
        .collect();
    let emitted_events: BTreeSet<String> = gateway_events::EMITTED_EVENT_TYPES
        .iter()
        .map(|event| (*event).to_owned())
        .collect();
    assert_eq!(
        emitted_events, manifest_events,
        "server emitted event set drifted from protocol gateway event manifest"
    );

    let documented = parse_documented_gateway_events(&read_doc("docs/GATEWAY_EVENTS.md"));
    let undocumented: Vec<String> = manifest_events
        .iter()
        .filter(|event| !documented.contains(*event))
        .cloned()
        .collect();
    assert!(
        undocumented.is_empty(),
        "events present in emitted manifest but missing in docs/GATEWAY_EVENTS.md: {}",
        undocumented.join(", ")
    );

    let client_literals = parse_client_gateway_event_literal_set();
    let undecodable_by_client: Vec<String> = manifest_events
        .iter()
        .filter(|event| !client_literals.contains(*event))
        .cloned()
        .collect();
    assert!(
        undecodable_by_client.is_empty(),
        "events present in emitted manifest but absent from client gateway decoder/dispatch source: {}",
        undecodable_by_client.join(", ")
    );
}
