//! Lightweight structural guardrails, run by the ordinary Cargo test command.

use std::fs;
use std::path::{Path, PathBuf};

const MAX_RUST_LINES: usize = 650;

fn rust_sources(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(rust_sources(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    files.sort();
    files
}

fn is_test_source(path: &Path) -> bool {
    path.components()
        .any(|component| component.as_os_str() == "tests")
        || path
            .file_stem()
            .is_some_and(|stem| stem.to_string_lossy().ends_with("tests"))
}

// Only import/path tokens matter here; comments and literal asset text are not dependencies.
fn code_tokens(source: &str) -> Vec<String> {
    let bytes = source.as_bytes();
    let mut cursor = 0;
    let mut tokens = Vec::new();
    while cursor < bytes.len() {
        if bytes[cursor..].starts_with(b"//") {
            while cursor < bytes.len() && bytes[cursor] != b'\n' {
                cursor += 1;
            }
        } else if bytes[cursor..].starts_with(b"/*") {
            cursor += 2;
            let mut depth = 1;
            while cursor < bytes.len() && depth > 0 {
                if bytes[cursor..].starts_with(b"/*") {
                    depth += 1;
                    cursor += 2;
                } else if bytes[cursor..].starts_with(b"*/") {
                    depth -= 1;
                    cursor += 2;
                } else {
                    cursor += 1;
                }
            }
        } else if bytes[cursor] == b'"' {
            cursor += 1;
            while cursor < bytes.len() {
                if bytes[cursor] == b'\\' {
                    cursor = (cursor + 2).min(bytes.len());
                } else if bytes[cursor] == b'"' {
                    cursor += 1;
                    break;
                } else {
                    cursor += 1;
                }
            }
        } else if bytes[cursor] == b'r' && raw_string_end(bytes, cursor).is_some() {
            cursor = raw_string_end(bytes, cursor).unwrap();
        } else if bytes[cursor].is_ascii_alphabetic() || bytes[cursor] == b'_' {
            let start = cursor;
            cursor += 1;
            while cursor < bytes.len()
                && (bytes[cursor].is_ascii_alphanumeric() || bytes[cursor] == b'_')
            {
                cursor += 1;
            }
            tokens.push(source[start..cursor].to_owned());
        } else if bytes[cursor..].starts_with(b"::") {
            tokens.push("::".to_owned());
            cursor += 2;
        } else if b"{};,".contains(&bytes[cursor]) {
            tokens.push(char::from(bytes[cursor]).to_string());
            cursor += 1;
        } else {
            cursor += 1;
        }
    }
    tokens
}

fn raw_string_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut quote = start + 1;
    while quote < bytes.len() && bytes[quote] == b'#' {
        quote += 1;
    }
    if bytes.get(quote) != Some(&b'"') {
        return None;
    }
    let hashes = quote - start - 1;
    let mut cursor = quote + 1;
    while cursor < bytes.len() {
        if bytes[cursor] == b'"'
            && bytes
                .get(cursor + 1..cursor + 1 + hashes)
                .is_some_and(|suffix| suffix.iter().all(|byte| *byte == b'#'))
        {
            return Some(cursor + 1 + hashes);
        }
        cursor += 1;
    }
    Some(bytes.len())
}

fn allowed_dependencies(layer: &str) -> Option<&'static [&'static str]> {
    match layer {
        "world" => Some(&["world"]),
        "player" => Some(&["player", "world"]),
        "game" => Some(&["game", "player", "world", "lighting"]),
        "save" => Some(&["save", "game", "player", "world"]),
        "mesh" => Some(&["mesh", "world", "water", "lighting"]),
        "lighting" => Some(&["lighting", "world"]),
        "water" => Some(&["water", "world"]),
        "phase" => Some(&["phase", "world"]),
        "camera" => Some(&["camera", "world"]),
        "streaming" => Some(&["streaming", "world", "mesh"]),
        "atlas" => Some(&["atlas"]),
        "hud" => Some(&["hud", "player", "game", "world", "mesh", "atlas"]),
        _ => None,
    }
}

fn local_path_root(token: &str) -> bool {
    ["crate", "self", "super"].contains(&token)
        || allowed_dependencies(token).is_some()
        || ["app", "input", "options", "renderer", "playtest"].contains(&token)
}

fn path_is_local(tokens: &[String], mut index: usize) -> bool {
    while index >= 2 && tokens[index - 1] == "::" {
        index -= 2;
    }
    local_path_root(&tokens[index])
}

fn boundary_violations(layer: &str, source: &str) -> Vec<String> {
    let Some(allowed) = allowed_dependencies(layer) else {
        return if [
            "app", "input", "options", "renderer", "playtest", "lib", "main",
        ]
        .contains(&layer)
        {
            Vec::new()
        } else {
            vec![format!(
                "declare a dependency policy for new subsystem {layer}"
            )]
        };
    };
    let tokens = code_tokens(source);
    let mut violations = Vec::new();
    for path in tokens.windows(3) {
        if path[0] == "crate" && path[1] == "::" && !allowed.contains(&path[2].as_str()) {
            violations.push(format!("{layer} cannot depend on crate::{}", path[2]));
        }
    }
    for native in ["wgpu", "winit", "png", "pollster", "cpal", "rodio"] {
        if tokens.iter().any(|token| token == native) {
            violations.push(format!("{layer} must not use {native}"));
        }
    }
    let mut external_import = false;
    for (index, token) in tokens.iter().enumerate() {
        if token == ";" {
            external_import = false;
        }
        if token == "use" {
            external_import = tokens[index + 1..]
                .iter()
                .find(|token| *token != "::")
                .is_some_and(|root| root != "{" && !local_path_root(root));
        }
        if external_import {
            continue;
        }
        let root_module = allowed_dependencies(token).is_some()
            || ["app", "input", "options", "renderer", "playtest"].contains(&token.as_str());
        if root_module
            && tokens.get(index + 1).is_some_and(|next| next == "::")
            && path_is_local(&tokens, index)
            && !allowed.contains(&token.as_str())
        {
            violations.push(format!("{layer} must not use outward module {token}"));
        }
        if token == "use" {
            for imported in tokens[index + 1..].iter().take_while(|token| *token != ";") {
                if (allowed_dependencies(imported).is_some()
                    || ["app", "input", "options", "renderer", "playtest"]
                        .contains(&imported.as_str()))
                    && !allowed.contains(&imported.as_str())
                {
                    violations.push(format!("{layer} must not import {imported}"));
                }
            }
        }
    }
    violations
}

#[test]
fn rust_files_stay_reviewable_including_tests_and_diagnostics() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut failures = Vec::new();
    for folder in ["src", "tests", "examples"] {
        for path in rust_sources(&root.join(folder)) {
            let count = fs::read_to_string(&path).unwrap().lines().count();
            if count > MAX_RUST_LINES {
                failures.push(format!(
                    "{}: {count} lines; split by responsibility before exceeding {MAX_RUST_LINES}",
                    path.strip_prefix(root).unwrap().display()
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn cpu_layers_depend_inward_not_on_native_presentation_or_the_application() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut failures = Vec::new();
    for path in rust_sources(&src) {
        let relative = path.strip_prefix(&src).unwrap();
        if is_test_source(relative) {
            continue;
        }
        let first = relative
            .components()
            .next()
            .unwrap()
            .as_os_str()
            .to_string_lossy();
        let layer = first.strip_suffix(".rs").unwrap_or(&first);
        let source = fs::read_to_string(&path).unwrap();
        for failure in boundary_violations(layer, &source) {
            failures.push(format!("src/{}: {failure}", relative.display()));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn package_stays_single_and_the_entry_point_stays_thin() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    assert_eq!(
        manifest.lines().filter(|line| *line == "[package]").count(),
        1
    );
    assert!(
        !manifest.lines().any(|line| line.starts_with("[workspace")),
        "Split crates only when a concrete independent consumer needs one."
    );
    let main = fs::read_to_string(root.join("src/main.rs")).unwrap();
    assert!(
        main.lines().count() <= 40,
        "main must delegate options and game startup"
    );
}

#[test]
fn boundary_checker_rejects_imports_and_qualified_native_paths() {
    assert!(
        !boundary_violations("world", "use crate::renderer::Renderer; wgpu::Device;").is_empty()
    );
    assert!(
        !boundary_violations("game", "use crate::app::run; use winit::window::Window;").is_empty()
    );
    assert!(!boundary_violations("player", "use wgpu as graphics; graphics::Device;").is_empty());
    assert!(!boundary_violations("game", "use cpal::traits::DeviceTrait;").is_empty());
    assert!(!boundary_violations("hud", "cpal::default_host();").is_empty());
    assert!(
        !boundary_violations("world", "use crate::{world::Block, renderer as graphics};")
            .is_empty()
    );
    assert!(!boundary_violations("game", "super::super::renderer::Renderer;").is_empty());
    assert!(!boundary_violations("new_subsystem", "").is_empty());
    assert!(
        boundary_violations(
            "mesh",
            "use crate::world::{Block, Chunk}; use crate::lighting::daylight_through;"
        )
        .is_empty()
    );
    assert!(boundary_violations("app", "wgpu::Device; crate::renderer::Renderer;").is_empty());
    for layer in [
        "player",
        "save",
        "lighting",
        "water",
        "phase",
        "camera",
        "streaming",
        "atlas",
        "hud",
    ] {
        assert!(!boundary_violations(layer, "crate::app::run();").is_empty());
    }
}

#[test]
fn external_crates_can_have_module_names_that_match_internal_layers() {
    for source in [
        "glam::camera::rh::view::look_to_mat4();",
        "use glam::camera::rh::view::look_to_mat4;",
        "use glam::{camera::rh::view::look_to_mat4, Vec3};",
        "use ::glam::{camera::rh::proj, Vec3}; glam::camera::rh::proj::orthographic();",
    ] {
        assert!(
            boundary_violations("lighting", source).is_empty(),
            "{source}"
        );
    }
    for source in [
        "crate::camera::view_projection();",
        "super::super::camera::view_projection();",
        "camera::view_projection();",
        "use crate::{world::Block, camera as views};",
        "use glam::{camera::rh, Vec3}; crate::camera::view_projection();",
        "use wgpu::{Device, Queue};",
    ] {
        assert!(
            !boundary_violations("lighting", source).is_empty(),
            "{source}"
        );
    }
}

#[test]
fn dependency_scanning_ignores_docs_nested_comments_and_escaped_raw_assets() {
    let source = r###"
// use winit::Window;
/* crate::app::run; /* wgpu::Device */ */
let label = "crate::renderer::Renderer \" wgpu::Device";
let shader = r##"crate::app::run " inner "##;
use crate::world::Block;
"###;
    assert!(boundary_violations("player", source).is_empty());
    assert_eq!(
        code_tokens("r\"wgpu::Device\" r#\"winit::Window\"# crate::world::Block"),
        ["crate", "::", "world", "::", "Block"]
    );
    assert!(code_tokens("/* incomplete").is_empty());
    assert!(code_tokens("r##\"incomplete").is_empty());
    assert!(is_test_source(Path::new("player/tests/collision.rs")));
    assert!(is_test_source(Path::new("player/speed_tests.rs")));
    assert!(!is_test_source(Path::new("player/collision.rs")));
}
