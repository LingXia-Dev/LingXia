use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};

/// Validate native video placement in built HTML. Dynamic native trees are
/// validated by the runtime; standard Web media needs no build-time audit.
pub(crate) fn audit_output_media(output_dir: &Path) -> Result<()> {
    let mut findings = Vec::new();
    for path in collect_files(output_dir)? {
        let rel = relative_path(output_dir, &path);
        match extension(&path).as_deref() {
            Some("html" | "htm") => {
                let source = fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read {}", path.display()))?;
                scan_html(&rel, &source, &mut findings);
            }
            Some("tsx" | "vue" | "ts") => {
                let source = fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read {}", path.display()))?;
                if is_html_document(&source) {
                    scan_html(&rel, &source, &mut findings);
                }
            }
            _ => {}
        }
    }

    if findings.is_empty() {
        return Ok(());
    }

    findings.sort();
    findings.dedup();

    bail!(
        "Invalid native video placement:\n{}",
        findings
            .into_iter()
            .map(|finding| format!(
                "  {}:{}: {}",
                finding.path,
                finding.line,
                finding.kind.message()
            ))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MediaKind {
    BareLxVideo,
}

impl MediaKind {
    fn message(self) -> &'static str {
        match self {
            Self::BareLxVideo => {
                "bare `<lx-video>` is not allowed. Make it a direct child of `<lx-native-root>`"
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Finding {
    path: String,
    line: usize,
    kind: MediaKind,
}

fn scan_html(path: &str, source: &str, findings: &mut Vec<Finding>) {
    let bytes = source.as_bytes();
    let mut i = 0;
    let mut stack: Vec<String> = Vec::new();
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        // `i` is on ASCII `<`, so every `&source[i..]` slice below is a
        // char boundary even when the page contains CJK or other multibyte text.
        if starts_with_ignore_ascii_case(&source[i..], "<!--") {
            i += 4;
            match source[i..].find("-->") {
                Some(end) => i += end + 3,
                None => break,
            }
            continue;
        }

        let after_lt = i + 1;
        if after_lt >= source.len() {
            break;
        }
        let (is_close, name_start) = if source.as_bytes()[after_lt] == b'/' {
            (true, after_lt + 1)
        } else {
            (false, after_lt)
        };
        let name = read_tag_name(&source[name_start..]);
        if name.is_empty() {
            i += 1;
            continue;
        }
        let name_lower = name.to_ascii_lowercase();

        if is_close {
            if let Some(position) = stack.iter().rposition(|open| open == &name_lower) {
                stack.truncate(position);
            }
            i += 1;
            continue;
        }

        match name_lower.as_str() {
            "lx-video" if stack.last().map(String::as_str) != Some("lx-native-root") => findings
                .push(Finding {
                    path: path.to_string(),
                    line: line_number(source, i),
                    kind: MediaKind::BareLxVideo,
                }),
            "script" => {
                if let Some(end) = raw_text_body_end(source, name_start, "script") {
                    i = end;
                    continue;
                }
            }
            "style" => {
                // `<style/>` closes itself; skipping to the next `</style>`
                // would step over everything between the two.
                if let Some(end) = raw_text_body_end(source, name_start, "style") {
                    i = end;
                    continue;
                }
            }
            _ => {}
        }

        if let Some(tag_end) = source[name_start..].find('>') {
            let open_end = name_start + tag_end;
            if !source[..open_end].trim_end().ends_with('/') {
                stack.push(name_lower);
            }
        }

        i += 1;
    }
}

/// Skip script/style raw text, but leave self-closing tags to the main scan.
fn raw_text_body_end(source: &str, name_start: usize, name: &str) -> Option<usize> {
    let rel_gt = source[name_start..].find('>')?;
    let open_end = name_start + rel_gt;
    if source[..open_end].trim_end().ends_with('/') {
        return None;
    }
    find_closing_tag(source, open_end + 1, name)
}

fn find_closing_tag(source: &str, from: usize, name: &str) -> Option<usize> {
    let needle = format!("</{name}");
    let rel = find_ignore_ascii_case(&source[from..], &needle)?;
    let close_gt = source[from + rel..].find('>')?;
    Some(from + rel + close_gt + 1)
}

fn read_tag_name(source: &str) -> &str {
    let end = source
        .char_indices()
        .find(|(_, ch)| !matches!(ch, 'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | ':'))
        .map(|(idx, _)| idx)
        .unwrap_or(source.len());
    &source[..end]
}

fn starts_with_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    haystack.len() >= needle.len()
        && haystack.as_bytes()[..needle.len()].eq_ignore_ascii_case(needle.as_bytes())
}

fn find_ignore_ascii_case(haystack: &str, needle: &str) -> Option<usize> {
    let needle = needle.as_bytes();
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .as_bytes()
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle))
}

fn is_html_document(source: &str) -> bool {
    let trimmed = source.trim_start();
    let lower = trimmed
        .chars()
        .take(128)
        .collect::<String>()
        .to_ascii_lowercase();
    lower.starts_with("<!doctype html")
        || lower.starts_with("<html")
        || (lower.contains("<head") && lower.contains("<body"))
}

fn collect_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files_inner(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files_inner(path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(path).with_context(|| format!("Failed to read {}", path.display()))? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_files_inner(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn line_number(source: &str, offset: usize) -> usize {
    let mut end = offset.min(source.len());
    while end > 0 && !source.is_char_boundary(end) {
        end -= 1;
    }
    source[..end].bytes().filter(|b| *b == b'\n').count() + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn audit_one(name: &str, source: &str) -> Result<()> {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join(name), source).unwrap();
        audit_output_media(temp.path())
    }

    #[test]
    fn allows_web_media_in_html_and_generated_documents() {
        for name in ["index.html", "index.tsx"] {
            audit_one(
                name,
                r#"<!doctype html><html><body>
<video src="https://cdn.example.com/video.mp4" controls></video>
<audio src="https://cdn.example.com/audio.mp3" controls></audio>
<script>new Audio("https://cdn.example.com/audio.mp3");</script>
</body></html>"#,
            )
            .unwrap();
        }
    }

    #[test]
    fn allows_web_media_in_bundles() {
        audit_one(
            "view.js",
            r#"
import { jsx as e } from 'react/jsx-runtime';
import { createElementVNode as v } from 'vue';
e("video", { controls: true });
v("audio", null);
document.createElement("video");
new Audio("./beep.mp3");
el.innerHTML = '<video></video><audio></audio>';
"#,
        )
        .unwrap();
    }

    #[test]
    fn allows_root_wrapped_lx_video() {
        audit_one(
            "index.html",
            "<lx-native-root><lx-video></lx-video></lx-native-root>",
        )
        .unwrap();
    }

    #[test]
    fn rejects_bare_and_indirect_lx_video() {
        for source in [
            "<lx-video></lx-video>",
            "<lx-native-root><div><lx-video></lx-video></div></lx-native-root>",
        ] {
            let err = audit_one("index.html", source).unwrap_err().to_string();
            assert!(err.contains("index.html:1:"), "{err}");
            assert!(err.contains("direct child"), "{err}");
        }
    }

    #[test]
    fn skips_comments_scripts_and_styles() {
        audit_one(
            "index.html",
            r#"<!-- <lx-video> -->
<style>.x::after { content: '<lx-video>'; }</style>
<script>const example = '<lx-video>';</script>
<lx-native-root><lx-video></lx-video></lx-native-root>"#,
        )
        .unwrap();
    }

    #[test]
    fn checks_after_self_closing_style_and_cjk() {
        let err = audit_one("index.html", "<style/>首页\n<lx-video></lx-video>")
            .unwrap_err()
            .to_string();
        assert!(err.contains("index.html:2:"), "{err}");
    }
}
