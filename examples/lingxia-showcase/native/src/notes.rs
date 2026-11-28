//! The native-route demo behind the `native` page: a call, a stream, and an app error,
//! over notes each lxapp keeps in its own `LxAppState`.

use std::fmt;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

lingxia::route_defaults!(prefix = "notes");

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

#[derive(Default, lingxia::FromLxApp)]
pub struct Notes(Mutex<Vec<Note>>);

impl Notes {
    fn list(&self) -> MutexGuard<'_, Vec<Note>> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[lingxia::native]
fn add(notes: lingxia::LxAppState<Notes>, input: NewNote) -> Result<Note, NotesError> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err(NotesError::EmptyTitle);
    }
    let mut notes = notes.list();
    let note = Note {
        id: notes.len() as u32 + 1,
        title: title.to_string(),
    };
    notes.push(note.clone());
    Ok(note)
}

#[lingxia::native]
fn get(notes: lingxia::LxAppState<Notes>, id: u32) -> Result<Note, NotesError> {
    notes
        .list()
        .iter()
        .find(|note| note.id == id)
        .cloned()
        .ok_or(NotesError::NotFound { id })
}

/// Reports each note as it is written; the result is the exported text.
#[lingxia::native(stream)]
fn export(
    notes: lingxia::LxAppState<Notes>,
    progress: lingxia::Progress<ExportStep>,
) -> lingxia::Result<String> {
    let notes = notes.list().clone();
    let total = notes.len() as u32;
    let mut text = String::new();
    for (done, note) in (1..).zip(&notes) {
        std::thread::sleep(Duration::from_millis(300));
        text.push_str(&format!("{}. {}\n", note.id, note.title));
        progress.send(ExportStep { done, total })?;
    }
    Ok(text)
}
