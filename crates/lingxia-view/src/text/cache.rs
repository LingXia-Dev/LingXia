use parley::{Alignment, AlignmentOptions, Layout};
use std::collections::HashMap;
use std::sync::Arc;

type TextLayout = Layout<crate::text::RunBrush>;

const MAX_ENTRIES: usize = 256;
const MAX_TEXT_BYTES: usize = 64 * 1024;
const MAX_ENTRY_BYTES: usize = 2048;
/// Broken layouts kept across frames; a page's widths and alignments per text are few.
const MAX_BROKEN: usize = 1024;

#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct Key {
    text: String,
    size: u32,
    letter_spacing: u32,
    weight: u32,
    line_height: Option<u32>,
    face: (crate::text::style::Family, bool),
    word_break: u8,
    overflow_wrap: u8,
    nowrap: bool,
    features: String,
    rtl: bool,
}
impl Key {
    pub(crate) fn new(
        text: &str,
        size_and_spacing: (f32, f32),
        weight: f32,
        line_height: Option<f32>,
        face: (crate::text::style::Family, bool),
        (word_break, overflow_wrap, nowrap, features): (u8, u8, bool, &str),
        rtl: bool,
    ) -> Option<Self> {
        (text.len() <= MAX_ENTRY_BYTES).then(|| Self {
            text: text.into(),
            size: size_and_spacing.0.to_bits(),
            letter_spacing: size_and_spacing.1.to_bits(),
            weight: weight.to_bits(),
            line_height: line_height.map(f32::to_bits),
            face,
            word_break,
            overflow_wrap,
            nowrap,
            features: features.into(),
            rtl,
        })
    }
}

/// How a layout's lines are broken: the width (bits, `u32::MAX` unbounded) and the alignment
/// applied after (`u8::MAX` none).
type Lines = (u32, u8);

fn lines_of(width: Option<f32>, align: Option<Alignment>) -> Lines {
    (
        width.map_or(u32::MAX, f32::to_bits),
        align.map_or(u8::MAX, |align| align as u8),
    )
}

/// A shaped text, shared with the cache until a caller changes it.
#[derive(Clone)]
pub(crate) struct ShapedText {
    layout: Arc<TextLayout>,
    /// The cache entry it was shaped as; its line breaks are cached under it.
    shape: Option<u64>,
    lines: Option<Lines>,
}

impl std::ops::Deref for ShapedText {
    type Target = TextLayout;
    fn deref(&self) -> &TextLayout {
        &self.layout
    }
}

impl ShapedText {
    pub(crate) fn owned(layout: TextLayout) -> Self {
        Self {
            layout: Arc::new(layout),
            shape: None,
            lines: None,
        }
    }

    /// The cache entry this text was shaped as; equal entries measure the same.
    pub(crate) fn shape_id(&self) -> Option<u64> {
        self.shape
    }

    pub(crate) fn into_layout(self) -> TextLayout {
        Arc::unwrap_or_clone(self.layout)
    }
}

struct Entry {
    layout: Arc<TextLayout>,
    id: u64,
    used: u64,
}

/// Shaped layouts by everything that changes glyphs, and their line breaks by width and
/// alignment, so an unchanged text costs neither shaping nor breaking on later frames.
#[derive(Default)]
pub(crate) struct TextCache {
    entries: HashMap<Key, Entry>,
    broken: HashMap<(u64, Lines), Arc<TextLayout>>,
    text_bytes: usize,
    next: u64,
}
impl TextCache {
    pub(crate) fn get(&mut self, key: &Key) -> Option<ShapedText> {
        self.next += 1;
        let entry = self.entries.get_mut(key)?;
        entry.used = self.next;
        Some(ShapedText {
            layout: entry.layout.clone(),
            shape: Some(entry.id),
            lines: Some(lines_of(None, None)),
        })
    }

    /// Cache `layout`, unbroken lines (`break_all_lines(None)`) already applied.
    pub(crate) fn insert(&mut self, key: Key, layout: TextLayout) -> ShapedText {
        while self.entries.len() >= MAX_ENTRIES || self.text_bytes + key.text.len() > MAX_TEXT_BYTES
        {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.used)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            let entry = self.entries.remove(&oldest).unwrap();
            self.text_bytes -= oldest.text.len();
            self.broken.retain(|(id, _), _| *id != entry.id);
        }
        self.next += 1;
        let layout = Arc::new(layout);
        self.text_bytes += key.text.len();
        self.entries.insert(
            key,
            Entry {
                layout: layout.clone(),
                id: self.next,
                used: self.next,
            },
        );
        ShapedText {
            layout,
            shape: Some(self.next),
            lines: Some(lines_of(None, None)),
        }
    }

    /// Break `text` at `width` and apply `align`, reusing an earlier frame's result.
    pub(crate) fn break_lines(
        &mut self,
        text: &mut ShapedText,
        width: Option<f32>,
        align: Option<Alignment>,
    ) {
        let lines = lines_of(width, align);
        if text.lines == Some(lines) {
            return;
        }
        text.lines = Some(lines);
        if let Some(id) = text.shape
            && let Some(hit) = self.broken.get(&(id, lines))
        {
            text.layout = hit.clone();
            return;
        }
        let layout = Arc::make_mut(&mut text.layout);
        layout.break_all_lines(width);
        if let Some(align) = align {
            layout.align(align, AlignmentOptions::default());
        }
        if let Some(id) = text.shape {
            if self.broken.len() >= MAX_BROKEN {
                self.broken.clear();
            }
            self.broken.insert((id, lines), text.layout.clone());
        }
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.broken.clear();
        self.text_bytes = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_keys_include_all_shape_styles_and_bound_retention() {
        let key = |text: &str| {
            Key::new(
                text,
                (16.0, 0.0),
                400.0,
                None,
                (crate::text::style::Family::Sans, false),
                (0, 0, false, ""),
                false,
            )
            .unwrap()
        };
        let mut cache = TextCache::default();
        let layout = Layout::default();
        let layout = || layout.clone();
        cache.insert(key("label"), layout());
        assert!(cache.get(&key("label")).is_some());
        for changed in [
            Key::new(
                "label",
                (16.0, 2.0),
                400.0,
                None,
                (crate::text::style::Family::Sans, false),
                (0, 0, false, ""),
                false,
            ),
            Key::new(
                "label",
                (16.0, 0.0),
                400.0,
                None,
                (crate::text::style::Family::Sans, true),
                (0, 0, false, ""),
                false,
            ),
            Key::new(
                "label",
                (17.0, 0.0),
                400.0,
                None,
                (crate::text::style::Family::Sans, false),
                (0, 0, false, ""),
                false,
            ),
            Key::new(
                "label",
                (16.0, 0.0),
                600.0,
                None,
                (crate::text::style::Family::Sans, false),
                (0, 0, false, ""),
                false,
            ),
            Key::new(
                "label",
                (16.0, 0.0),
                400.0,
                Some(24.0),
                (crate::text::style::Family::Sans, false),
                (0, 0, false, ""),
                false,
            ),
            Key::new(
                "label",
                (16.0, 0.0),
                400.0,
                None,
                (crate::text::style::Family::Mono, false),
                (0, 0, false, ""),
                false,
            ),
            Key::new(
                "label",
                (16.0, 0.0),
                400.0,
                None,
                (crate::text::style::Family::Sans, false),
                (1, 0, false, ""),
                false,
            ),
            Key::new(
                "label",
                (16.0, 0.0),
                400.0,
                None,
                (crate::text::style::Family::Sans, false),
                (2, 0, false, ""),
                false,
            ),
            Key::new(
                "label",
                (16.0, 0.0),
                400.0,
                None,
                (crate::text::style::Family::Sans, false),
                (0, 1, false, ""),
                false,
            ),
            Key::new(
                "label",
                (16.0, 0.0),
                400.0,
                None,
                (crate::text::style::Family::Sans, false),
                (0, 2, false, ""),
                false,
            ),
            Key::new(
                "label",
                (16.0, 0.0),
                400.0,
                None,
                (crate::text::style::Family::Sans, false),
                (0, 0, true, ""),
                false,
            ),
        ] {
            assert!(cache.get(&changed.unwrap()).is_none());
        }
        for i in 0..200 {
            cache.insert(key(&format!("{i:04}{}", "x".repeat(2000))), layout());
        }
        assert!(cache.entries.len() <= MAX_ENTRIES && cache.text_bytes <= MAX_TEXT_BYTES);
        assert!(cache.get(&key("label")).is_none());
        assert!(
            Key::new(
                &"x".repeat(MAX_ENTRY_BYTES + 1),
                (16.0, 0.0),
                400.0,
                None,
                (crate::text::style::Family::Sans, false),
                (0, 0, false, ""),
                false
            )
            .is_none()
        );
        cache.clear();
        assert!(cache.entries.is_empty());
        assert_eq!(cache.text_bytes, 0);
        for i in 0..MAX_ENTRIES {
            cache.insert(key(&i.to_string()), layout());
        }
        assert!(cache.get(&key("0")).is_some());
        cache.insert(key("new"), layout());
        assert_eq!(cache.entries.len(), MAX_ENTRIES);
        assert!(cache.get(&key("0")).is_some());
        assert!(cache.get(&key("1")).is_none());
        assert!(cache.get(&key("new")).is_some());
    }

    #[test]
    fn line_breaks_are_shared_across_frames_by_width_and_alignment() {
        let key = Key::new(
            "label",
            (16.0, 0.0),
            400.0,
            None,
            (crate::text::style::Family::Sans, false),
            (0, 0, false, ""),
            false,
        )
        .unwrap();
        let mut cache = TextCache::default();
        let mut first = cache.insert(key.clone(), Layout::default());
        cache.break_lines(&mut first, Some(100.0), Some(parley::Alignment::Center));
        let mut again = cache.get(&key).unwrap();
        cache.break_lines(&mut again, Some(100.0), Some(parley::Alignment::Center));
        assert!(Arc::ptr_eq(&first.layout, &again.layout));
        cache.break_lines(&mut again, Some(80.0), Some(parley::Alignment::Center));
        assert!(!Arc::ptr_eq(&first.layout, &again.layout));
        let mut owned = ShapedText::owned(Layout::default());
        cache.break_lines(&mut owned, Some(100.0), None);
        assert_eq!(
            cache.broken.len(),
            2,
            "an uncached text leaves nothing behind"
        );
    }
}
