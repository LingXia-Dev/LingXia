use parley::Layout;
use std::collections::VecDeque;

const MAX_ENTRIES: usize = 128;
const MAX_TEXT_BYTES: usize = 64 * 1024;
const MAX_ENTRY_BYTES: usize = 2048;

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Key {
    text: String,
    size: u32,
    letter_spacing: u32,
    weight: u32,
    line_height: Option<u32>,
    face: (bool, bool),
    word_break: u8,
    overflow_wrap: u8,
}
impl Key {
    pub(crate) fn new(
        text: &str,
        size_and_spacing: (f32, f32),
        weight: f32,
        line_height: Option<f32>,
        face: (bool, bool),
        word_break: u8,
        overflow_wrap: u8,
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
        })
    }
}

/// Unbroken layouts only. Each caller owns its line breaking and alignment.
#[derive(Default)]
pub(crate) struct TextCache {
    entries: VecDeque<(Key, Layout<()>)>,
    text_bytes: usize,
}
impl TextCache {
    pub(crate) fn get(&mut self, key: &Key) -> Option<Layout<()>> {
        let index = self.entries.iter().position(|(stored, _)| stored == key)?;
        let entry = self.entries.remove(index)?;
        let layout = entry.1.clone();
        self.entries.push_back(entry);
        Some(layout)
    }
    pub(crate) fn insert(&mut self, key: Key, layout: &Layout<()>) {
        while self.entries.len() >= MAX_ENTRIES || self.text_bytes + key.text.len() > MAX_TEXT_BYTES
        {
            if let Some((old, _)) = self.entries.pop_front() {
                self.text_bytes -= old.text.len();
            }
        }
        self.text_bytes += key.text.len();
        self.entries.push_back((key, layout.clone()));
    }
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.text_bytes = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_keys_include_all_shape_styles_and_bound_retention() {
        let key =
            |text: &str| Key::new(text, (16.0, 0.0), 400.0, None, (false, false), 0, 0).unwrap();
        let mut cache = TextCache::default();
        let layout = Layout::default();
        cache.insert(key("label"), &layout);
        assert!(cache.get(&key("label")).is_some());
        for changed in [
            Key::new("label", (16.0, 2.0), 400.0, None, (false, false), 0, 0),
            Key::new("label", (16.0, 0.0), 400.0, None, (false, true), 0, 0),
            Key::new("label", (17.0, 0.0), 400.0, None, (false, false), 0, 0),
            Key::new("label", (16.0, 0.0), 600.0, None, (false, false), 0, 0),
            Key::new(
                "label",
                (16.0, 0.0),
                400.0,
                Some(24.0),
                (false, false),
                0,
                0,
            ),
            Key::new("label", (16.0, 0.0), 400.0, None, (true, false), 0, 0),
            Key::new("label", (16.0, 0.0), 400.0, None, (false, false), 1, 0),
            Key::new("label", (16.0, 0.0), 400.0, None, (false, false), 2, 0),
            Key::new("label", (16.0, 0.0), 400.0, None, (false, false), 0, 1),
            Key::new("label", (16.0, 0.0), 400.0, None, (false, false), 0, 2),
        ] {
            assert!(cache.get(&changed.unwrap()).is_none());
        }
        for i in 0..200 {
            cache.insert(key(&format!("{i:04}{}", "x".repeat(2000))), &layout);
        }
        assert!(cache.entries.len() <= MAX_ENTRIES && cache.text_bytes <= MAX_TEXT_BYTES);
        assert!(cache.get(&key("label")).is_none());
        assert!(
            Key::new(
                &"x".repeat(MAX_ENTRY_BYTES + 1),
                (16.0, 0.0),
                400.0,
                None,
                (false, false),
                0,
                0
            )
            .is_none()
        );
        cache.clear();
        assert!(cache.entries.is_empty());
        assert_eq!(cache.text_bytes, 0);
        for i in 0..MAX_ENTRIES {
            cache.insert(key(&i.to_string()), &layout);
        }
        assert!(cache.get(&key("0")).is_some());
        cache.insert(key("new"), &layout);
        assert_eq!(cache.entries.len(), MAX_ENTRIES);
        assert!(cache.get(&key("0")).is_some());
        assert!(cache.get(&key("1")).is_none());
        assert!(cache.get(&key("new")).is_some());
    }
}
