//! Code generation for `foundry build`.
//!
//! `foundry build` does not compile HTML to machine code itself: it generates a
//! small Cargo project whose `main.rs` embeds the (escaped) HTML as a string
//! literal and calls [`crate::run_embedded`], then shells out to `cargo build
//! --release`. The helpers here produce that generated source, and are kept
//! separate from the CLI so they can be unit-tested.

/// Version of `alia-foundry` used for the crates.io fallback dependency.
pub const CRATE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Package name published on crates.io.
pub const CRATE_NAME: &str = "alia-foundry";

/// Build the `[dependencies]` line pointing at the Foundry runtime.
///
/// When the CLI is running from a checkout, `local_crate_path` is the directory
/// containing this crate's `Cargo.toml` and a path dependency is emitted.
/// Otherwise (e.g. `cargo install alia-foundry`) we fall back to crates.io,
/// pinned to the version of the CLI that generated the project.
pub fn foundry_dep_line(local_crate_path: Option<&str>) -> String {
    match local_crate_path {
        Some(path) => format!(
            "foundry_runtime = {{ package = \"{}\", path = \"{}\" }}",
            CRATE_NAME,
            path.replace('\\', "/")
        ),
        None => format!(
            "foundry_runtime = {{ package = \"{}\", version = \"{}\" }}",
            CRATE_NAME, CRATE_VERSION
        ),
    }
}

/// Render the `Cargo.toml` of the generated project.
pub fn generate_cargo_toml(bin_name: &str, dep_line: &str) -> String {
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
{dep}
env_logger = "0.11"

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
strip = true
panic = "abort"
"#,
        name = bin_name,
        dep = dep_line,
    )
}

/// Escape an HTML document so it can be embedded as a Rust string literal.
pub fn escape_html_literal(html: &str) -> String {
    html.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Render the `src/main.rs` of the generated project.
pub fn generate_main_rs(html: &str, title: &str) -> String {
    format!(
        r#"fn main() {{
    foundry_runtime::run_embedded(
        "{html}",
        "{title}",
    );
}}
"#,
        html = escape_html_literal(html),
        title = escape_html_literal(title),
    )
}
