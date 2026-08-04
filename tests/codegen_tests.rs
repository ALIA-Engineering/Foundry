//! The generated project must be valid before we hand it to `cargo build`.
//!
//! Regression cover for the crates.io fallback, which used to emit a plain
//! string containing literal `{{`/`}}` braces (a leftover from a `format!`
//! that was turned into a `to_string()`), producing a Cargo.toml that Cargo
//! refused to parse. That path is the one taken by `cargo install
//! alia-foundry` + `foundry build`, i.e. the advertised install route.

use foundry_runtime::codegen::{
    escape_html_literal, foundry_dep_line, generate_cargo_toml, generate_main_rs, CRATE_NAME,
    CRATE_VERSION,
};
use toml::Value;

fn parse(manifest: &str) -> Value {
    toml::from_str::<Value>(manifest)
        .unwrap_or_else(|e| panic!("generated Cargo.toml is not valid TOML: {e}\n---\n{manifest}"))
}

fn dep_table(manifest: &str) -> Value {
    parse(manifest)["dependencies"]["foundry_runtime"].clone()
}

#[test]
fn the_crates_io_fallback_manifest_is_valid_toml() {
    let manifest = generate_cargo_toml("demo", &foundry_dep_line(None));

    let dep = dep_table(&manifest);
    assert_eq!(dep["package"].as_str(), Some(CRATE_NAME));
    assert_eq!(dep["version"].as_str(), Some(CRATE_VERSION));
    assert!(dep.get("path").is_none());
}

#[test]
fn the_crates_io_fallback_has_no_literal_braces() {
    let line = foundry_dep_line(None);
    assert!(!line.contains("{{"), "unexpanded format braces: {line}");
    assert!(!line.contains("}}"), "unexpanded format braces: {line}");
}

#[test]
fn the_fallback_version_tracks_the_crate_version() {
    // A hardcoded "0.1" would resolve to a runtime that predates the current
    // CLI; the pin has to follow Cargo.toml.
    assert_eq!(CRATE_VERSION, env!("CARGO_PKG_VERSION"));
    assert!(foundry_dep_line(None).contains(&format!("version = \"{CRATE_VERSION}\"")));
}

#[test]
fn the_path_manifest_is_valid_toml() {
    let manifest = generate_cargo_toml("demo", &foundry_dep_line(Some("/home/me/Foundry")));

    let dep = dep_table(&manifest);
    assert_eq!(dep["package"].as_str(), Some(CRATE_NAME));
    assert_eq!(dep["path"].as_str(), Some("/home/me/Foundry"));
    assert!(dep.get("version").is_none());
}

#[test]
fn windows_paths_are_emitted_with_forward_slashes() {
    // Backslashes would be read as TOML string escapes.
    let manifest = generate_cargo_toml(
        "demo",
        &foundry_dep_line(Some(r"C:\Users\me\Desktop\Foundry")),
    );

    let dep = dep_table(&manifest);
    assert_eq!(dep["path"].as_str(), Some("C:/Users/me/Desktop/Foundry"));
}

#[test]
fn the_manifest_carries_the_package_metadata_and_release_profile() {
    let manifest = generate_cargo_toml("counter", &foundry_dep_line(None));
    let value = parse(&manifest);

    assert_eq!(value["package"]["name"].as_str(), Some("counter"));
    assert_eq!(value["package"]["edition"].as_str(), Some("2021"));
    assert!(value["dependencies"].get("env_logger").is_some());
    assert_eq!(value["profile"]["release"]["opt-level"].as_str(), Some("z"));
    assert_eq!(value["profile"]["release"]["lto"].as_bool(), Some(true));
    assert_eq!(value["profile"]["release"]["strip"].as_bool(), Some(true));
}

#[test]
fn html_is_escaped_so_the_generated_main_stays_a_single_string_literal() {
    let html = "<p class=\"x\">a\\b\ttab</p>\r\n";
    let escaped = escape_html_literal(html);

    assert_eq!(escaped, "<p class=\\\"x\\\">a\\\\b\\ttab</p>\\r\\n");
    assert!(!escaped.contains('\n'));
    assert!(!escaped.contains('\r'));
}

#[test]
fn generated_main_embeds_the_document_and_the_title() {
    let main_rs = generate_main_rs("<html><body>hi \"there\"</body></html>", "demo");

    assert!(main_rs.starts_with("fn main() {\n"));
    assert!(main_rs.contains("foundry_runtime::run_embedded("));
    assert!(main_rs.contains("<html><body>hi \\\"there\\\"</body></html>"));
    assert!(main_rs.contains("\"demo\""));
    // exactly two string literals: the document and the window title
    assert_eq!(main_rs.matches("\\\"").count(), 2);
}

#[test]
fn a_multiline_document_is_flattened_into_one_line_of_rust() {
    let main_rs = generate_main_rs("<html>\n  <body>\n  </body>\n</html>", "demo");

    // 4 lines: fn main, run_embedded(, the html arg, the title arg, ) and }
    assert_eq!(main_rs.lines().filter(|l| l.contains("<html>")).count(), 1);
    assert!(!main_rs.contains("<html>\n"));
}
