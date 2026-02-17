use std::{collections::BTreeSet, path::PathBuf};

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
fn gateway_docs_cover_emitted_event_manifest() {
    let documented = parse_documented_gateway_events(&read_doc("docs/GATEWAY_EVENTS.md"));
    let mut undocumented = Vec::new();
    for event in gateway_events::EMITTED_EVENT_TYPES {
        if !documented.contains(*event) {
            undocumented.push((*event).to_owned());
        }
    }

    assert!(
        undocumented.is_empty(),
        "events present in emitted manifest but missing in docs/GATEWAY_EVENTS.md: {}",
        undocumented.join(", ")
    );
}
