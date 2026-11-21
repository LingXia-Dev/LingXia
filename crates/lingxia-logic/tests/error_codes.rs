//! Every error code an `lx` call can reject with is a member of `LX_ERROR_CODES`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn union() -> BTreeSet<String> {
    let source = fs::read_to_string(repo().join("packages/lingxia-types/src/error.ts")).unwrap();
    let start = source.find("LX_ERROR_CODES = [").unwrap();
    let end = start + source[start..].find(']').unwrap();
    codes_in(&source[start..end])
}

/// `E_` followed by upper-case words, as a quoted literal or a `rong::error::` constant.
fn codes_in(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for (at, _) in text.match_indices("E_") {
        let before = text[..at].chars().last();
        let quoted = matches!(before, Some('"') | Some('\''));
        let constant = text[..at].ends_with("error::");
        if !quoted && !constant {
            continue;
        }
        let code: String = text[at..]
            .chars()
            .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_')
            .collect();
        if code.len() > 2 {
            found.insert(code);
        }
    }
    found
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn every_thrown_code_is_in_the_published_union() {
    let allowed = union();
    assert!(
        allowed.contains("E_NOT_SUPPORTED"),
        "read the union: {allowed:?}"
    );
    let mut files = Vec::new();
    for crate_dir in ["crates/lingxia-logic/src", "crates/lingxia-lxapp/src"] {
        rust_files(&repo().join(crate_dir), &mut files);
    }
    let mut strays = Vec::new();
    for file in files {
        // `lx.automation()` publishes its own code set.
        let name = file.file_name().unwrap().to_string_lossy();
        if name.starts_with("automation") {
            continue;
        }
        let text = fs::read_to_string(&file).unwrap();
        // The View's own reply names, which `view_call.rs` maps onto the union.
        let view_names: &[&str] = if name == "view_call.rs" {
            &["E_INVALID_ARGUMENT", "E_INVALID_PARAMETER", "E_UNSUPPORTED"]
        } else {
            &[]
        };
        for code in codes_in(&text) {
            if !allowed.contains(&code) && !view_names.contains(&code.as_str()) {
                strays.push(format!("{}: {code}", file.display()));
            }
        }
    }
    assert!(
        strays.is_empty(),
        "codes outside LX_ERROR_CODES:\n{}",
        strays.join("\n")
    );
}
