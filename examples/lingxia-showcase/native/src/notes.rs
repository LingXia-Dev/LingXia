//! The native-route demo behind the `native` page: a call, a stream, and an app error.

use std::fmt;
use std::sync::Mutex;
use std::time::Duration;

#[derive(Clone, serde::Serialize)]
pub struct Note {
    id: u32,
    title: String,
}

#[derive(serde::Deserialize)]
pub struct NewNote {
    title: String,
}

#[derive(Debug, serde::Serialize, lingxia::NativeError)]
#[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NotesError {
    EmptyTitle,
    NotFound { id: u32 },
}

impl fmt::Display for NotesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTitle => f.write_str("a note needs a title"),
            Self::NotFound { id } => write!(f, "note {id} not found"),
        }
    }
}

#[derive(serde::Serialize)]
pub struct ExportStep {
    done: u32,
    total: u32,
}

static NOTES: Mutex<Vec<Note>> = Mutex::new(Vec::new());

fn notes() -> std::sync::MutexGuard<'static, Vec<Note>> {
    NOTES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[lingxia::native("notes.add")]
fn add(input: NewNote) -> Result<Note, NotesError> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err(NotesError::EmptyTitle);
    }
    let mut notes = notes();
    let note = Note {
        id: notes.len() as u32 + 1,
        title: title.to_string(),
    };
    notes.push(note.clone());
    Ok(note)
}

#[lingxia::native("notes.get")]
fn get(id: u32) -> Result<Note, NotesError> {
    notes()
        .iter()
        .find(|note| note.id == id)
        .cloned()
        .ok_or(NotesError::NotFound { id })
}

/// Reports each note as it is written; the result is the exported text.
#[lingxia::native("notes.export", stream)]
fn export(progress: lingxia::Progress<ExportStep>) -> lingxia::Result<String> {
    let notes = notes().clone();
    let total = notes.len() as u32;
    let mut text = String::new();
    for (done, note) in (1..).zip(&notes) {
        std::thread::sleep(Duration::from_millis(300));
        text.push_str(&format!("{}. {}\n", note.id, note.title));
        progress.send(ExportStep { done, total })?;
    }
    Ok(text)
}
