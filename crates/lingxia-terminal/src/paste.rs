//! Paste-risk classification.
//!
//! Pure functions hosts apply to clipboard text before writing it to a
//! PTY. The crate only classifies; confirmation UI and ask/allow/deny
//! policy belong to hosts.

use serde::Serialize;

/// Risk flags for text about to be pasted into a terminal.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteRisk {
    /// Newlines beyond a single trailing terminator: pasting would
    /// run multiple commands.
    pub multiline: bool,
    /// Ends with a line terminator: the last command would execute
    /// immediately on paste.
    pub trailing_newline: bool,
    /// C0/C1/DEL control characters other than tab/CR/LF — escape
    /// sequences and control bytes can smuggle arbitrary terminal
    /// behavior into a paste.
    pub control_chars: bool,
}

impl PasteRisk {
    /// Conservative default: any flag warrants a host confirmation.
    pub fn is_dangerous(&self) -> bool {
        self.multiline || self.trailing_newline || self.control_chars
    }
}

/// Classify clipboard text for paste risk.
pub fn classify_paste(text: &str) -> PasteRisk {
    let trailing_newline = text.ends_with('\n') || text.ends_with('\r');
    let body = if trailing_newline {
        text.trim_end_matches(['\n', '\r'])
    } else {
        text
    };
    let multiline = body.contains('\n') || body.contains('\r');
    let control_chars = text.chars().any(|ch| {
        let code = ch as u32;
        (code < 0x20 && !matches!(ch, '\t' | '\n' | '\r')) || (0x7f..=0x9f).contains(&code)
    });
    PasteRisk {
        multiline,
        trailing_newline,
        control_chars,
    }
}

/// JSON variant of [`classify_paste`] for FFI hosts.
pub fn classify_paste_json(text: &str) -> String {
    serde_json::to_string(&classify_paste(text)).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_single_line_is_safe() {
        let risk = classify_paste("git status");
        assert_eq!(risk, PasteRisk::default());
        assert!(!risk.is_dangerous());
        // Tabs are legitimate paste content.
        assert_eq!(classify_paste("a\tb"), PasteRisk::default());
    }

    #[test]
    fn trailing_newline_executes_immediately() {
        let risk = classify_paste("make install\n");
        assert!(risk.trailing_newline);
        assert!(!risk.multiline, "a lone trailing newline is one command");
        assert!(risk.is_dangerous());
    }

    #[test]
    fn embedded_newlines_are_multiline() {
        let risk = classify_paste("ls\nrm -rf build\n");
        assert!(risk.multiline);
        assert!(risk.trailing_newline);
        let risk = classify_paste("ls\r\nrm -rf build");
        assert!(risk.multiline);
        assert!(!risk.trailing_newline);
    }

    #[test]
    fn control_characters_are_flagged() {
        assert!(classify_paste("a\u{1b}[2jb").control_chars);
        assert!(classify_paste("a\u{7}b").control_chars);
        assert!(classify_paste("a\u{7f}b").control_chars);
        assert!(classify_paste("a\u{85}b").control_chars);
        assert!(!classify_paste("ok").control_chars);
    }

    #[test]
    fn json_shape_is_camel_case_flags() {
        let json = classify_paste_json("a\nb\n");
        assert!(json.contains("\"multiline\":true"), "{json}");
        assert!(json.contains("\"trailingNewline\":true"), "{json}");
        assert!(json.contains("\"controlChars\":false"), "{json}");
    }
}
