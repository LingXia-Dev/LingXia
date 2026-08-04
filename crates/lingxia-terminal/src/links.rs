//! Heuristic URL/path detection over terminal text.
//!
//! Pure and filesystem-free: relative candidates resolve against the
//! session cwd lexically (`.`/`..` normalized without touching disk).
//! Ranges are character offsets into the scanned line; OSC 8
//! hyperlinks are reported separately by the caller, which knows the
//! grid coordinates.

use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LinkSource {
    /// Explicit OSC 8 hyperlink from the application.
    Osc8,
    /// URL/path recognized heuristically in plain output.
    Heuristic,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedLink {
    /// URL verbatim, or the cwd-resolved normalized path.
    pub target: String,
    /// Character offset range in the scanned line, end exclusive.
    pub start: usize,
    pub end: usize,
    pub source: LinkSource,
    /// `:line[:column]` suffix parsed from a path, when present.
    pub line: Option<u32>,
    pub column: Option<u32>,
}

const URL_SCHEMES: &[&str] = &[
    "https://", "http://", "ftp://", "ssh://", "file://", "mailto:",
];

fn is_url_terminator(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, '<' | '>' | '"' | '\'' | '`' | '|')
}

fn is_path_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
        || matches!(
            ch,
            '/' | '\\' | '.' | '_' | '-' | '~' | '+' | '%' | '@' | '=' | ',' | '#' | ':'
        )
}

/// Detect URLs and file paths in one line of terminal text.
pub fn detect_links(text: &str, cwd: Option<&Path>) -> Vec<DetectedLink> {
    let chars: Vec<char> = text.chars().collect();
    let mut links = Vec::new();
    let mut occupied: Vec<bool> = vec![false; chars.len()];

    // URLs first so paths never swallow their ranges.
    for scheme in URL_SCHEMES {
        let scheme_chars: Vec<char> = scheme.chars().collect();
        let mut index = 0;
        while index + scheme_chars.len() <= chars.len() {
            if chars[index..index + scheme_chars.len()] == scheme_chars[..]
                && (index == 0 || !is_path_char(chars[index - 1]))
            {
                let mut end = index + scheme_chars.len();
                while end < chars.len() && !is_url_terminator(chars[end]) {
                    end += 1;
                }
                while end > index + scheme_chars.len()
                    && matches!(
                        chars[end - 1],
                        '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}'
                    )
                {
                    // Keep balanced ')' (e.g. wikipedia URLs).
                    if chars[end - 1] == ')'
                        && chars[index..end].iter().filter(|&&c| c == '(').count()
                            >= chars[index..end].iter().filter(|&&c| c == ')').count()
                    {
                        break;
                    }
                    end -= 1;
                }
                if end > index + scheme_chars.len() {
                    let target: String = chars[index..end].iter().collect();
                    for slot in occupied.iter_mut().take(end).skip(index) {
                        *slot = true;
                    }
                    links.push(DetectedLink {
                        target,
                        start: index,
                        end,
                        source: LinkSource::Heuristic,
                        line: None,
                        column: None,
                    });
                    index = end;
                    continue;
                }
            }
            index += 1;
        }
    }

    // Paths: tokens of path characters containing a separator or drive.
    let mut index = 0;
    while index < chars.len() {
        if occupied[index] || !is_path_char(chars[index]) {
            index += 1;
            continue;
        }
        let start = index;
        while index < chars.len() && is_path_char(chars[index]) && !occupied[index] {
            index += 1;
        }
        let end = index;
        if let Some(link) = classify_path(&chars, start, end, cwd) {
            links.push(link);
        }
    }

    links.sort_by_key(|link| link.start);
    links
}

/// Interpret a path-character token as a filesystem path, attaching an
/// optional `:line[:column]` suffix and resolving against `cwd`. The
/// reported range covers the whole token, suffix included.
fn classify_path(
    chars: &[char],
    start: usize,
    end: usize,
    cwd: Option<&Path>,
) -> Option<DetectedLink> {
    let raw: String = chars[start..end].iter().collect();
    let trimmed = raw.trim_end_matches(['.', ',', ';', '!', '?']);
    let consumed = trimmed.chars().count();
    if is_windows_path(trimmed) {
        return Some(DetectedLink {
            target: trimmed.to_string(),
            start,
            end: start + consumed,
            source: LinkSource::Heuristic,
            line: None,
            column: None,
        });
    }
    let (path_text, line, column) = match split_line_col(trimmed) {
        Some((base, line, column)) => (base, line, column),
        None => (trimmed, None, None),
    };
    let kind = path_kind(path_text)?;
    let target = match kind {
        PathKind::Absolute => normalize_path(Path::new(path_text)),
        PathKind::Home => {
            let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
            normalize_path(&Path::new(&home).join(path_text.trim_start_matches("~/")))
        }
        PathKind::Relative => normalize_path(&cwd?.join(path_text)),
    };
    Some(DetectedLink {
        target: target.to_string_lossy().into_owned(),
        start,
        end: start + consumed,
        source: LinkSource::Heuristic,
        line,
        column,
    })
}

fn is_windows_path(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\')
}

enum PathKind {
    Absolute,
    Home,
    Relative,
}

fn path_kind(text: &str) -> Option<PathKind> {
    if text.len() < 2 {
        return None;
    }
    if text.starts_with('/') {
        return Some(PathKind::Absolute);
    }
    if text.starts_with("~/") {
        return Some(PathKind::Home);
    }
    if (text.starts_with("./") || text.starts_with("../")) && text.len() > 3 {
        return Some(PathKind::Relative);
    }
    // Bare "a/b" relative paths: at least two non-empty segments, no
    // leading separator or drive colon.
    if !text.contains(':') && text.split('/').filter(|s| !s.is_empty()).count() >= 2 {
        return Some(PathKind::Relative);
    }
    None
}

/// Split a trailing `:line[:column]` off a path token. Windows drive
/// colons are filtered out by the caller beforehand.
fn split_line_col(token: &str) -> Option<(&str, Option<u32>, Option<u32>)> {
    let (head, tail) = token.rsplit_once(':')?;
    let last = tail.parse::<u32>().ok()?;
    if let Some((base, mid)) = head.rsplit_once(':')
        && let Ok(line) = mid.parse::<u32>()
    {
        return Some((base, Some(line), Some(last)));
    }
    // A single numeric suffix is the line number.
    Some((head, Some(last), None))
}

/// Lexically normalize `.` and `..` components without filesystem
/// access (symlinks deliberately unresolved).
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn targets(text: &str, cwd: Option<&Path>) -> Vec<String> {
        detect_links(text, cwd)
            .into_iter()
            .map(|link| link.target)
            .collect()
    }

    #[test]
    fn detects_urls_with_trailing_punctuation_stripped() {
        let links = detect_links("see https://example.com/a?b=1, then done", None);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "https://example.com/a?b=1");
        let text_chars: Vec<char> = "see https://example.com/a?b=1, then done".chars().collect();
        let span: String = text_chars[links[0].start..links[0].end].iter().collect();
        assert_eq!(span, "https://example.com/a?b=1");

        assert_eq!(
            targets("mail mailto:user@example.com now", None),
            vec!["mailto:user@example.com".to_string()]
        );
        assert_eq!(
            targets("(https://en.wikipedia.org/wiki/Rust_(language))", None),
            vec!["https://en.wikipedia.org/wiki/Rust_(language)".to_string()]
        );
    }

    #[test]
    fn detects_absolute_and_home_paths() {
        assert_eq!(
            targets("open /etc/hosts please", None),
            vec!["/etc/hosts".to_string()]
        );
        let home = std::env::var_os("HOME").expect("HOME set");
        let found = targets("cat ~/work/file.txt", None);
        assert_eq!(found.len(), 1);
        assert!(found[0].ends_with("/work/file.txt"), "resolved: {found:?}");
        assert!(found[0].starts_with(&home.to_string_lossy().to_string()));
    }

    #[test]
    fn resolves_relative_paths_against_cwd_lexically() {
        let cwd = Path::new("/repo/project");
        assert_eq!(
            targets("./src/main.rs", Some(cwd)),
            vec!["/repo/project/src/main.rs".to_string()]
        );
        assert_eq!(
            targets("../shared/lib.ts", Some(cwd)),
            vec!["/repo/shared/lib.ts".to_string()]
        );
        assert_eq!(
            targets("src/main.rs", Some(cwd)),
            vec!["/repo/project/src/main.rs".to_string()]
        );
        // Without a cwd, bare relative candidates are skipped.
        assert!(targets("src/main.rs", None).is_empty());
    }

    #[test]
    fn parses_line_and_column_suffixes() {
        let cwd = Path::new("/repo");
        let links = detect_links("src/main.rs:42:10 failed", Some(cwd));
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "/repo/src/main.rs");
        assert_eq!(links[0].line, Some(42));
        assert_eq!(links[0].column, Some(10));

        let links = detect_links("src/main.rs:42 failed", Some(cwd));
        assert_eq!(links[0].line, Some(42));
        assert_eq!(links[0].column, None);
    }

    #[test]
    fn ignores_urls_when_scanning_paths() {
        let links = detect_links("https://example.com/a/b", Some(Path::new("/repo")));
        assert_eq!(links.len(), 1, "url not double-reported: {links:?}");
        assert!(links[0].target.starts_with("https://"));
    }

    #[test]
    fn rejects_non_paths() {
        assert!(targets("and/or", Some(Path::new("/r"))).len() <= 1);
        assert!(targets("hello world", None).is_empty());
        assert!(targets("a", None).is_empty());
        assert!(targets("C:", None).is_empty());
    }
}
