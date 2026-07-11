//! Chrome-compatible Netscape Bookmark HTML import and export.

use html5ever::tendril::StrTendril;
use html5ever::tokenizer::{
    BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
};
use std::cell::RefCell;

use crate::bookmarks::BookmarksSnapshot;

const MAX_IMPORT_BOOKMARKS: usize = 50_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImportedBookmark {
    pub url: String,
    pub title: String,
    pub group_name: Option<String>,
    pub created_at_ms: Option<u64>,
}

#[derive(Default)]
struct ParserState {
    folders: Vec<Option<String>>,
    pending_folder: Option<String>,
    capture: Option<Capture>,
    bookmarks: Vec<ImportedBookmark>,
}

enum Capture {
    Folder(String),
    Bookmark {
        url: String,
        title: String,
        created_at_ms: Option<u64>,
    },
}

#[derive(Default)]
struct BookmarkSink(RefCell<ParserState>);

impl BookmarkSink {
    fn finish(self) -> Vec<ImportedBookmark> {
        self.0.into_inner().bookmarks
    }
}

impl TokenSink for BookmarkSink {
    type Handle = ();

    fn process_token(&self, token: Token, _line_number: u64) -> TokenSinkResult<Self::Handle> {
        let mut state = self.0.borrow_mut();
        match token {
            Token::TagToken(tag) if tag.kind == TagKind::StartTag => {
                let name = tag.name.as_ref();
                if name.eq_ignore_ascii_case("h3") {
                    state.capture = Some(Capture::Folder(String::new()));
                } else if name.eq_ignore_ascii_case("a") {
                    let url = tag
                        .attrs
                        .iter()
                        .find(|attr| attr.name.local.as_ref().eq_ignore_ascii_case("href"))
                        .map(|attr| attr.value.to_string())
                        .unwrap_or_default();
                    let created_at_ms = tag
                        .attrs
                        .iter()
                        .find(|attr| attr.name.local.as_ref().eq_ignore_ascii_case("add_date"))
                        .and_then(|attr| attr.value.parse::<u64>().ok())
                        .and_then(|seconds| seconds.checked_mul(1_000));
                    state.capture = Some(Capture::Bookmark {
                        url,
                        title: String::new(),
                        created_at_ms,
                    });
                } else if name.eq_ignore_ascii_case("dl") {
                    let folder = state.pending_folder.take();
                    state.folders.push(folder);
                }
            }
            Token::TagToken(tag) if tag.kind == TagKind::EndTag => {
                let name = tag.name.as_ref();
                if name.eq_ignore_ascii_case("h3") {
                    if let Some(Capture::Folder(name)) = state.capture.take() {
                        let name = clean_text(&name);
                        state.pending_folder = (!name.is_empty()).then_some(name);
                    }
                } else if name.eq_ignore_ascii_case("a") {
                    if state.bookmarks.len() >= MAX_IMPORT_BOOKMARKS {
                        return TokenSinkResult::Continue;
                    }
                    if let Some(Capture::Bookmark {
                        url,
                        title,
                        created_at_ms,
                    }) = state.capture.take()
                    {
                        let group_name = flattened_folder_name(&state.folders);
                        state.bookmarks.push(ImportedBookmark {
                            url: url.trim().to_string(),
                            title: clean_text(&title),
                            group_name,
                            created_at_ms,
                        });
                    }
                } else if name.eq_ignore_ascii_case("dl") {
                    state.folders.pop();
                }
            }
            Token::CharacterTokens(text) => {
                if let Some(capture) = state.capture.as_mut() {
                    match capture {
                        Capture::Folder(value) => value.push_str(&text),
                        Capture::Bookmark { title, .. } => title.push_str(&text),
                    }
                }
            }
            _ => {}
        }
        TokenSinkResult::Continue
    }
}

fn clean_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn flattened_folder_name(folders: &[Option<String>]) -> Option<String> {
    let path = folders
        .iter()
        .filter_map(Option::as_deref)
        .map(clean_text)
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>()
        .join(" / ");
    (!path.is_empty()).then_some(path)
}

pub(crate) fn parse_chrome_html(input: &str) -> Vec<ImportedBookmark> {
    let sink = BookmarkSink::default();
    let mut queue = BufferQueue::default();
    queue.push_back(StrTendril::from(input));
    let tokenizer = Tokenizer::new(sink, TokenizerOpts::default());
    let _ = tokenizer.feed(&mut queue);
    tokenizer.end();
    tokenizer.sink.finish()
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn bookmark_line(url: &str, title: &str, created_at_ms: u64, indent: &str) -> String {
    format!(
        "{indent}<DT><A HREF=\"{}\" ADD_DATE=\"{}\">{}</A>\n",
        escape_html(url),
        created_at_ms / 1_000,
        escape_html(title)
    )
}

pub(crate) fn export_chrome_html(snapshot: &BookmarksSnapshot, exported_at_ms: u64) -> String {
    let exported_at = exported_at_ms / 1_000;
    let mut output = String::from(
        "<!DOCTYPE NETSCAPE-Bookmark-file-1>\n\
<!-- This is an automatically generated file.\n\
     It will be read and overwritten.\n\
     DO NOT EDIT! -->\n\
<META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n\
<TITLE>Bookmarks</TITLE>\n\
<H1>Bookmarks</H1>\n\
<DL><p>\n",
    );

    for entry in snapshot
        .entries
        .iter()
        .filter(|entry| entry.group_id.is_none())
    {
        output.push_str(&bookmark_line(
            &entry.url,
            &entry.title,
            entry.created_at_ms,
            "    ",
        ));
    }

    for group in &snapshot.groups {
        output.push_str(&format!(
            "    <DT><H3 ADD_DATE=\"{exported_at}\" LAST_MODIFIED=\"{exported_at}\">{}</H3>\n",
            escape_html(&group.name)
        ));
        output.push_str("    <DL><p>\n");
        for entry in snapshot
            .entries
            .iter()
            .filter(|entry| entry.group_id.as_deref() == Some(group.id.as_str()))
        {
            output.push_str(&bookmark_line(
                &entry.url,
                &entry.title,
                entry.created_at_ms,
                "        ",
            ));
        }
        output.push_str("    </DL><p>\n");
    }
    output.push_str("</DL><p>\n");
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bookmarks::{BookmarkEntry, BookmarkGroup};

    #[test]
    fn parses_chrome_folders_entities_and_dates() {
        let html = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p>
  <DT><H3>Bookmarks bar</H3>
  <DL><p>
    <DT><A HREF="https://example.com/?a=1&amp;b=2" ADD_DATE="123">A &amp; B</A>
    <DT><H3>Work</H3>
    <DL><p><DT><A HREF="https://docs.example.com">Docs</A></DL><p>
  </DL><p>
  <DT><A HREF="https://root.example.com">Root</A>
</DL><p>"#;
        let parsed = parse_chrome_html(html);
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].url, "https://example.com/?a=1&b=2");
        assert_eq!(parsed[0].title, "A & B");
        assert_eq!(parsed[0].group_name.as_deref(), Some("Bookmarks bar"));
        assert_eq!(parsed[0].created_at_ms, Some(123_000));
        assert_eq!(
            parsed[1].group_name.as_deref(),
            Some("Bookmarks bar / Work")
        );
        assert_eq!(parsed[2].group_name, None);
    }

    #[test]
    fn exports_netscape_html_that_round_trips() {
        let mut snapshot = BookmarksSnapshot::default();
        snapshot.groups = vec![BookmarkGroup {
            id: "g1".into(),
            name: "R&D <Docs>".into(),
        }];
        snapshot.entries = vec![BookmarkEntry {
            id: "b1".into(),
            url: "https://example.com/?a=1&b=2".into(),
            title: "A \"useful\" page".into(),
            group_id: Some("g1".into()),
            pinned: true,
            favicon_png_base64: None,
            created_at_ms: 123_000,
        }];
        let exported = export_chrome_html(&snapshot, 456_000);
        assert!(exported.starts_with("<!DOCTYPE NETSCAPE-Bookmark-file-1>"));
        assert!(exported.contains("R&amp;D &lt;Docs&gt;"));
        assert!(exported.contains("HREF=\"https://example.com/?a=1&amp;b=2\""));

        let parsed = parse_chrome_html(&exported);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].url, snapshot.entries[0].url);
        assert_eq!(parsed[0].title, snapshot.entries[0].title);
        assert_eq!(parsed[0].group_name.as_deref(), Some("R&D <Docs>"));
        assert_eq!(parsed[0].created_at_ms, Some(123_000));
    }
}
