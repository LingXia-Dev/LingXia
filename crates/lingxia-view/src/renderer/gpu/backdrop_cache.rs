//! A backdrop's filtered image, reused across frames while what lies beneath it and its filter
//! stay the same: a caret blink or a spinner above the glass composites the cached image instead
//! of capturing and blurring again.

use crate::scene::{
    composite::{Layer, Plan, Step},
    node::ClipSnapshot,
    paint::Paint,
};
use std::collections::VecDeque;

/// Everything a filtered backdrop's pixels depend on.
#[derive(Clone, PartialEq)]
pub(crate) struct BackdropKey {
    pub size: [u16; 2],
    pub scale: f32,
    /// Image sources and the pixel buffers they resolved to.
    pub images: Vec<(String, usize)>,
    /// Every command before the backdrop: the content beneath it and the groups it sits in.
    /// Scrolling under the glass changes these.
    pub beneath: Vec<Paint>,
    pub clip: ClipSnapshot,
    pub blur: f32,
    pub saturation: f32,
    pub filter_scale: f32,
}

/// The key of each backdrop `plan` filters, in the order it filters them.
pub(crate) fn keys(plan: &Plan, display: &crate::DisplayList) -> VecDeque<BackdropKey> {
    let (width, height) = plan.dimensions();
    let mut images: Vec<(String, usize)> = display
        .images
        .iter()
        .map(|(src, image)| (src.clone(), std::sync::Arc::as_ptr(image.pixels()) as usize))
        .collect();
    images.sort();
    let mut drawn = 0;
    let mut keys = VecDeque::new();
    for step in plan.steps() {
        match step {
            Step::Draw(range) => drawn = range.end,
            Step::Push(Layer::Backdrop {
                clip,
                blur,
                saturation,
                ..
            }) => keys.push_back(BackdropKey {
                size: [width, height],
                scale: display.scale,
                images: images.clone(),
                beneath: display.commands[..drawn.min(display.commands.len())].to_vec(),
                clip: *clip,
                blur: *blur,
                saturation: saturation.amount(),
                filter_scale: crate::scene::backdrop_region::filter_scale(*blur),
            }),
            _ => {}
        }
    }
    keys
}

/// Least-recently-used filtered images, bounded in bytes.
pub(crate) struct BackdropCache<T> {
    entries: VecDeque<(BackdropKey, T, u64)>,
    bytes: u64,
}

impl<T> Default for BackdropCache<T> {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
            bytes: 0,
        }
    }
}

impl<T: Clone> BackdropCache<T> {
    pub fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }

    /// Drop the least recently used image; false when none is left.
    pub fn evict_oldest(&mut self) -> bool {
        let Some((.., bytes)) = self.entries.pop_front() else {
            return false;
        };
        self.bytes -= bytes;
        true
    }

    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// The image filtered for `key`, now the most recently used.
    pub fn get(&mut self, key: &BackdropKey) -> Option<T> {
        let index = self.entries.iter().position(|(stored, ..)| stored == key)?;
        let entry = self.entries.remove(index)?;
        let value = entry.1.clone();
        self.entries.push_back(entry);
        Some(value)
    }

    /// Keep `value` for `key`, evicting the least recently used until `budget` holds.
    pub fn insert(&mut self, key: BackdropKey, value: T, bytes: u64, budget: u64) {
        if let Some(index) = self.entries.iter().position(|(stored, ..)| *stored == key) {
            let (.., old) = self.entries.remove(index).unwrap();
            self.bytes -= old;
        }
        if bytes > budget {
            return;
        }
        self.entries.push_back((key, value, bytes));
        self.bytes += bytes;
        while self.bytes > budget {
            let Some((.., evicted)) = self.entries.pop_front() else {
                break;
            };
            self.bytes -= evicted;
        }
    }
}

#[cfg(test)]
pub(crate) fn test_key(index: u16) -> BackdropKey {
    BackdropKey {
        size: [index, 0],
        scale: 1.0,
        images: Vec::new(),
        beneath: Vec::new(),
        clip: ClipSnapshot {
            bounds: [0.0; 4],
            radius: 0.0,
            radii: None,
            transform: None,
        },
        blur: 0.0,
        saturation: 1.0,
        filter_scale: 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> BackdropKey {
        BackdropKey {
            size: [1084, 2412],
            scale: 3.0,
            images: Vec::new(),
            beneath: vec![Paint::PopClip],
            clip: ClipSnapshot {
                bounds: [40.0, 400.0, 1000.0, 1500.0],
                radius: 48.0,
                radii: None,
                transform: None,
            },
            blur: 72.0,
            saturation: 1.0,
            filter_scale: 4.0,
        }
    }

    #[test]
    fn any_change_beneath_or_to_the_filter_misses() {
        let mut cache = BackdropCache::default();
        cache.insert(key(), 1, 10, 100);
        assert_eq!(cache.get(&key()), Some(1), "an unchanged backdrop hits");
        let changes: [fn(&mut BackdropKey); 7] = [
            |k| k.beneath.push(Paint::PopOpacity),
            |k| k.clip.bounds[1] = 380.0,
            |k| k.blur = 24.0,
            |k| k.saturation = 1.8,
            |k| k.filter_scale = 2.0,
            |k| k.size = [1084, 2400],
            |k| k.images.push(("cover.png".into(), 7)),
        ];
        for change in changes {
            let mut changed = key();
            change(&mut changed);
            assert_eq!(cache.get(&changed), None);
        }
    }

    #[test]
    fn least_recently_used_images_leave_first_within_the_budget() {
        let mut cache = BackdropCache::default();
        let named = |n: f32| BackdropKey { blur: n, ..key() };
        cache.insert(named(1.0), 'a', 40, 100);
        cache.insert(named(2.0), 'b', 40, 100);
        assert_eq!(cache.get(&named(1.0)), Some('a'));
        cache.insert(named(3.0), 'c', 40, 100);
        assert_eq!(cache.bytes(), 80);
        assert_eq!(cache.get(&named(2.0)), None, "b was used least recently");
        assert_eq!(cache.get(&named(1.0)), Some('a'));
        assert_eq!(cache.get(&named(3.0)), Some('c'));
        cache.insert(named(4.0), 'd', 200, 100);
        assert_eq!(
            cache.get(&named(4.0)),
            None,
            "an image over budget is not kept"
        );
        assert_eq!(cache.len(), 2);
        cache.insert(named(1.0), 'A', 40, 100);
        assert_eq!((cache.get(&named(1.0)), cache.bytes()), (Some('A'), 80));
    }
}
