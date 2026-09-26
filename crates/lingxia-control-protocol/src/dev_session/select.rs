//! Which live dev session a command addresses.
//!
//! One resolver for every client (`lxdev`, `lingxia dev stop`), so a
//! selector means the same session everywhere:
//!
//! - an explicit selector (`--session`, `LXDEV_SESSION`) is, in order, a
//!   session name (`lingxia dev --name`), a target (`macos`, `lxapp`, …),
//!   `target@<project-dir>`, an ordinal from `lxdev session`, or a
//!   session id prefix;
//! - without one: the single live session whose project contains the
//!   working directory, else the single live session;
//! - anything else is refused with a table of the candidates, never guessed.

use super::broker::SessionInfo;
use std::fmt;
use std::path::{Path, PathBuf};

/// Why no single session was selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectError {
    /// Nothing is live.
    NoSessions,
    /// The selector matches no live session.
    NoMatch { query: String, table: String },
    /// More than one session fits; `query` is `None` without a selector.
    Ambiguous {
        query: Option<String>,
        table: String,
    },
}

impl fmt::Display for SelectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSessions => f.write_str(NO_SESSION_HINT),
            Self::NoMatch { query, table } => write!(
                f,
                "No dev session matches --session {query:?}. Live sessions:\n\n{table}\n\n{PICK_HINT}"
            ),
            Self::Ambiguous { query: None, table } => write!(
                f,
                "Several dev sessions could be meant. Pick one with --session:\n\n{table}\n\n\
                 {PICK_HINT}"
            ),
            Self::Ambiguous {
                query: Some(query),
                table,
            } => write!(
                f,
                "--session {query:?} matches several dev sessions:\n\n{table}\n\n{PICK_HINT}"
            ),
        }
    }
}

impl std::error::Error for SelectError {}

/// What a command that works on a running app says when none is running.
pub const NO_SESSION_HINT: &str = "No running app session for this project. Start one with \
                                   `lingxia dev` (or `lingxia dev --background` in scripts).";

const PICK_HINT: &str = "--session takes a NAME, a TARGET, TARGET@<project-dir>, or the # \
                         column (name a session with `lingxia dev --name NAME`).";

/// Valid `lingxia dev --name`: it must not read as an ordinal or a target.
pub fn validate_name(name: &str) -> Result<(), String> {
    let valid = !name.is_empty()
        && name.len() <= 40
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && name.chars().next().is_some_and(|c| c.is_ascii_alphabetic());
    if !valid {
        return Err(format!(
            "invalid session name {name:?}: start with a letter; use letters, digits, '-', '_' \
             or '.' (at most 40)"
        ));
    }
    if TARGETS
        .iter()
        .any(|target| target.eq_ignore_ascii_case(name))
    {
        return Err(format!(
            "session name {name:?} is a target name; `--session {name}` already selects by target"
        ));
    }
    Ok(())
}

const TARGETS: [&str; 6] = ["android", "ios", "macos", "harmony", "windows", "lxapp"];

/// Resolve `query` against `sessions` (ordered as `lxdev session`
/// prints them) from the working directory `cwd`.
pub fn select<'a>(
    sessions: &'a [SessionInfo],
    query: Option<&str>,
    cwd: &Path,
) -> Result<&'a SessionInfo, SelectError> {
    if sessions.is_empty() {
        return Err(SelectError::NoSessions);
    }
    let Some(query) = query.map(str::trim).filter(|query| !query.is_empty()) else {
        return select_default(sessions, cwd);
    };
    let pick = |matches: Vec<&'a SessionInfo>| -> Result<&'a SessionInfo, SelectError> {
        match matches.as_slice() {
            [] => Err(SelectError::NoMatch {
                query: query.to_string(),
                table: candidate_table(sessions, &sessions.iter().collect::<Vec<_>>()),
            }),
            [only] => Ok(*only),
            _ => {
                // Several fit: the one of this directory's project, if unique.
                let local: Vec<_> = matches
                    .iter()
                    .copied()
                    .filter(|session| contains_cwd(session, cwd))
                    .collect();
                if let [only] = local.as_slice() {
                    return Ok(*only);
                }
                Err(SelectError::Ambiguous {
                    query: Some(query.to_string()),
                    table: candidate_table(sessions, &matches),
                })
            }
        }
    };

    let named: Vec<_> = sessions
        .iter()
        .filter(|session| {
            session
                .name
                .as_deref()
                .is_some_and(|name| name.eq_ignore_ascii_case(query))
        })
        .collect();
    if !named.is_empty() {
        return pick(named);
    }
    let by_target: Vec<_> = sessions
        .iter()
        .filter(|session| session.target.eq_ignore_ascii_case(query))
        .collect();
    if !by_target.is_empty() {
        return pick(by_target);
    }
    if let Some((target, dir)) = query.rsplit_once('@') {
        let wanted = absolute(Path::new(dir), cwd);
        let matches: Vec<_> = sessions
            .iter()
            .filter(|session| session.target.eq_ignore_ascii_case(target))
            .filter(|session| {
                project_paths(session).iter().any(|path| {
                    same_path(path, &wanted)
                        || (!dir.contains(['/', '\\'])
                            && path.file_name().is_some_and(|name| name == dir))
                })
            })
            .collect();
        return pick(matches);
    }
    if query.bytes().all(|byte| byte.is_ascii_digit())
        && let Ok(ordinal) = query.parse::<usize>()
        && (1..=sessions.len()).contains(&ordinal)
    {
        return Ok(&sessions[ordinal - 1]);
    }
    pick(
        sessions
            .iter()
            .filter(|session| session.session_id.starts_with(query))
            .collect(),
    )
}

fn select_default<'a>(
    sessions: &'a [SessionInfo],
    cwd: &Path,
) -> Result<&'a SessionInfo, SelectError> {
    let local: Vec<_> = sessions
        .iter()
        .filter(|session| contains_cwd(session, cwd))
        .collect();
    match local.as_slice() {
        [only] => return Ok(*only),
        [] => {}
        _ => {
            return Err(SelectError::Ambiguous {
                query: None,
                table: candidate_table(sessions, &local),
            });
        }
    }
    match sessions {
        [only] => Ok(only),
        _ => Err(SelectError::Ambiguous {
            query: None,
            table: candidate_table(sessions, &sessions.iter().collect::<Vec<_>>()),
        }),
    }
}

/// What a printed hint (a Rerun line, a stop command) should pass as
/// `--session` to reach `chosen` again from `cwd`: nothing when the default
/// resolution already reaches it, else its name, its target, or
/// `target@<project-dir>` — never an id, which dies with the session.
pub fn hint_selector(sessions: &[SessionInfo], chosen: &SessionInfo, cwd: &Path) -> Option<String> {
    let reaches = |query: Option<&str>| {
        select(sessions, query, cwd).is_ok_and(|found| found.session_id == chosen.session_id)
    };
    if reaches(None) {
        return None;
    }
    if let Some(name) = &chosen.name
        && reaches(Some(name))
    {
        return Some(name.clone());
    }
    if reaches(Some(&chosen.target)) {
        return Some(chosen.target.clone());
    }
    let project = PathBuf::from(&chosen.project_root);
    if let Some(dir) = project.file_name().and_then(|name| name.to_str()) {
        let short = format!("{}@{dir}", chosen.target);
        if reaches(Some(&short)) {
            return Some(short);
        }
    }
    Some(format!("{}@{}", chosen.target, chosen.project_root))
}

/// The candidate table: `#` is the ordinal among all live `sessions`.
pub fn candidate_table(sessions: &[SessionInfo], shown: &[&SessionInfo]) -> String {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64);
    let rows: Vec<[String; 6]> = shown
        .iter()
        .map(|session| {
            let ordinal = sessions
                .iter()
                .position(|candidate| candidate.session_id == session.session_id)
                .map_or_else(|| "-".to_string(), |index| (index + 1).to_string());
            [
                ordinal,
                session.session_id.clone(),
                session.name.clone().unwrap_or_else(|| "-".to_string()),
                session.target.clone(),
                abbreviate_home(
                    session
                        .content
                        .as_ref()
                        .map(|content| content.display())
                        .unwrap_or(&session.project_root),
                ),
                started_ago(session.started_at, now_ms),
            ]
        })
        .collect();
    let header = ["#", "ID", "NAME", "TARGET", "PROJECT", "STARTED"];
    let mut widths = header.map(str::len);
    for row in &rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    let line = |cells: [&str; 6]| {
        let mut out = String::from("  ");
        for (index, (cell, width)) in cells.iter().zip(widths).enumerate() {
            if index == cells.len() - 1 {
                out.push_str(cell);
            } else {
                out.push_str(&format!("{cell:<width$}  "));
            }
        }
        out.trim_end().to_string()
    };
    let mut table = vec![line(header)];
    for row in &rows {
        table.push(line(row.each_ref().map(String::as_str)));
    }
    table.join("\n")
}

fn started_ago(started_at: u64, now_ms: u64) -> String {
    if started_at == 0 || now_ms < started_at {
        return "-".to_string();
    }
    let secs = (now_ms - started_at) / 1000;
    match secs {
        0..60 => format!("{secs}s ago"),
        60..3600 => format!("{}m ago", secs / 60),
        3600..86_400 => format!("{}h {}m ago", secs / 3600, (secs % 3600) / 60),
        _ => format!("{}d ago", secs / 86_400),
    }
}

fn abbreviate_home(path: &str) -> String {
    let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) else {
        return path.to_string();
    };
    let home = home.to_string_lossy();
    match path.strip_prefix(home.as_ref()) {
        Some(rest) if rest.starts_with(['/', '\\']) => format!("~{rest}"),
        _ => path.to_string(),
    }
}

/// The directories a session belongs to: its context root and, for mounted
/// lxapp/host content, that path.
fn project_paths(session: &SessionInfo) -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from(&session.project_root)];
    if let Some(
        super::broker::SessionContent::Host { path }
        | super::broker::SessionContent::LxApp { path },
    ) = &session.content
    {
        paths.push(PathBuf::from(path));
    }
    paths
}

fn contains_cwd(session: &SessionInfo, cwd: &Path) -> bool {
    let cwd = canonical(cwd);
    project_paths(session)
        .iter()
        .any(|path| !path.as_os_str().is_empty() && cwd.starts_with(canonical(path)))
}

fn absolute(path: &Path, cwd: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    canonical(a) == canonical(b)
}

/// Canonical form without the Windows verbatim prefix, which session
/// records never carry.
fn canonical(path: &Path) -> PathBuf {
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let text = resolved.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        return PathBuf::from(rest);
    }
    resolved
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev_session::broker::SessionContent;

    fn session(id: &str, target: &str, root: &str, name: Option<&str>) -> SessionInfo {
        SessionInfo {
            session_id: id.to_string(),
            project_root: root.to_string(),
            content: Some(SessionContent::Host {
                path: root.to_string(),
            }),
            target: target.to_string(),
            pid: 1,
            started_at: 1,
            executable: String::new(),
            ws_url: "ws://127.0.0.1:1".to_string(),
            log_file: String::new(),
            name: name.map(str::to_string),
            build: None,
            extra: Default::default(),
        }
    }

    fn ids(result: Result<&SessionInfo, SelectError>) -> Result<&str, SelectError> {
        result.map(|session| session.session_id.as_str())
    }

    const NOWHERE: &str = "/nonexistent-cwd";

    #[test]
    fn nothing_live_is_its_own_error() {
        assert_eq!(
            ids(select(&[], None, Path::new(NOWHERE))),
            Err(SelectError::NoSessions)
        );
    }

    #[test]
    fn without_a_selector_the_directory_decides_then_the_only_session() {
        let one = [session("a1b2c3", "macos", "/work/app", None)];
        assert_eq!(ids(select(&one, None, Path::new(NOWHERE))), Ok("a1b2c3"));

        let two = [
            session("a1b2c3", "macos", "/work/app", None),
            session("d4e5f6", "lxapp", "/work/other", None),
        ];
        assert_eq!(
            ids(select(&two, None, Path::new("/work/other/tests"))),
            Ok("d4e5f6")
        );
        let Err(SelectError::Ambiguous { query: None, table }) =
            select(&two, None, Path::new(NOWHERE))
        else {
            panic!("expected an ambiguity");
        };
        assert!(
            table.contains("a1b2c3") && table.contains("d4e5f6"),
            "{table}"
        );
        assert!(table.lines().next().unwrap().contains("NAME"), "{table}");

        // Two sessions of this very project: still refused, with only them.
        let same = [
            session("a1b2c3", "macos", "/work/app", None),
            session("d4e5f6", "android", "/work/app", None),
            session("0a0b0c", "lxapp", "/work/other", None),
        ];
        let Err(SelectError::Ambiguous { table, .. }) = select(&same, None, Path::new("/work/app"))
        else {
            panic!("expected an ambiguity");
        };
        assert!(!table.contains("0a0b0c"), "{table}");
        assert!(table.contains("  1 ") && table.contains("  2 "), "{table}");
    }

    #[test]
    fn selectors_match_name_target_project_ordinal_then_id() {
        let live = [
            session("a1b2c3", "macos", "/work/app", Some("demo")),
            session("d4e5f6", "macos", "/work/other", None),
            session("123456", "lxapp", "/work/tool", None),
        ];
        let cwd = Path::new(NOWHERE);
        assert_eq!(ids(select(&live, Some("demo"), cwd)), Ok("a1b2c3"));
        assert_eq!(ids(select(&live, Some("DEMO"), cwd)), Ok("a1b2c3"));
        assert_eq!(ids(select(&live, Some("lxapp"), cwd)), Ok("123456"));
        assert_eq!(ids(select(&live, Some("macos@other"), cwd)), Ok("d4e5f6"));
        assert_eq!(
            ids(select(&live, Some("macos@/work/app"), cwd)),
            Ok("a1b2c3")
        );
        assert_eq!(ids(select(&live, Some("2"), cwd)), Ok("d4e5f6"));
        // Past the ordinals, digits are an id prefix.
        assert_eq!(ids(select(&live, Some("1234"), cwd)), Ok("123456"));
        assert_eq!(ids(select(&live, Some("d4e"), cwd)), Ok("d4e5f6"));

        // A shared target is ambiguous, unless one is this directory's.
        let Err(SelectError::Ambiguous { query, table }) = select(&live, Some("macos"), cwd) else {
            panic!("expected an ambiguity");
        };
        assert_eq!(query.as_deref(), Some("macos"));
        assert!(!table.contains("123456"), "{table}");
        assert_eq!(
            ids(select(&live, Some("macos"), Path::new("/work/other/src"))),
            Ok("d4e5f6")
        );

        let Err(SelectError::NoMatch { query, table }) = select(&live, Some("nope"), cwd) else {
            panic!("expected no match");
        };
        assert_eq!(query, "nope");
        assert!(
            table.contains("a1b2c3") && table.contains("demo"),
            "{table}"
        );
        assert!(select(&live, Some("android@app"), cwd).is_err());
    }

    #[test]
    fn hints_never_teach_ids() {
        let live = [
            session("a1b2c3", "macos", "/work/app", Some("demo")),
            session("d4e5f6", "macos", "/work/other", None),
            session("0a0b0c", "lxapp", "/work/tool", None),
        ];
        let cwd = Path::new(NOWHERE);
        assert_eq!(hint_selector(&live, &live[0], cwd).as_deref(), Some("demo"));
        assert_eq!(
            hint_selector(&live, &live[1], cwd).as_deref(),
            Some("macos@other")
        );
        assert_eq!(
            hint_selector(&live, &live[2], cwd).as_deref(),
            Some("lxapp")
        );
        // From inside its project the default already reaches it.
        assert_eq!(
            hint_selector(&live, &live[1], Path::new("/work/other")),
            None
        );
        assert_eq!(hint_selector(&live[..1], &live[0], cwd), None);
    }

    #[test]
    fn names_cannot_pass_for_targets_or_ordinals() {
        assert!(validate_name("demo-1").is_ok());
        assert!(validate_name("1").is_err());
        assert!(validate_name("macos").is_err());
        assert!(validate_name("a b").is_err());
        assert!(validate_name("").is_err());
    }

    #[test]
    fn a_selector_error_reads_as_a_table() {
        let live = [session("a1b2c3", "macos", "/work/app", None)];
        let message = select(&live, Some("zz"), Path::new(NOWHERE))
            .unwrap_err()
            .to_string();
        assert!(message.contains("No dev session matches"), "{message}");
        assert!(message.contains("--session takes a NAME"), "{message}");
    }
}
