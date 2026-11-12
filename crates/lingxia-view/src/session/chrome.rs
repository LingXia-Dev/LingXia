//! Snapshots of the host's bars that a raised page draws under its top layer, so the
//! dialog's backdrop dims the bars the user saw instead of the page's background.
use crate::scene::display::{ChromeEdge, DisplayList};
use crate::scene::image::Bitmap;

/// Larger than any phone or tablet surface edge; a snapshot past it is refused.
const MAX_EDGE: u32 = 8192;

#[derive(Default)]
pub(crate) struct ChromeSnapshots {
    top: Option<Snapshot>,
    bottom: Option<Snapshot>,
    /// The latest snapshot's generation; frames that draw it report it.
    generation: u64,
    /// The surface the snapshots were drawn at; a different one drops them.
    surface: Option<(u16, u16, u32)>,
}

struct Snapshot {
    bitmap: Bitmap,
    /// Surface pixels.
    rect: [f32; 4],
}

impl ChromeSnapshots {
    /// `rgba` is premultiplied RGBA8, `width` × `height`; `rect` places it in surface pixels.
    pub(crate) fn set(
        &mut self,
        edge: ChromeEdge,
        generation: u64,
        rgba: &[u8],
        width: u32,
        height: u32,
        rect: [f32; 4],
    ) -> Result<(), String> {
        if width == 0 || height == 0 || width > MAX_EDGE || height > MAX_EDGE {
            return Err("UI_CHROME: snapshot size out of range".into());
        }
        if rgba.len() as u64 != u64::from(width) * u64::from(height) * 4 {
            return Err("UI_CHROME: snapshot bytes do not match its size".into());
        }
        if rect.iter().any(|v| !v.is_finite()) || rect[2] <= 0.0 || rect[3] <= 0.0 {
            return Err("UI_CHROME: invalid snapshot rect".into());
        }
        let mut pixels = vello_cpu::Pixmap::new(width as u16, height as u16);
        pixels.data_as_u8_slice_mut().copy_from_slice(rgba);
        let snapshot = Snapshot {
            bitmap: Bitmap::from_pixels(pixels),
            rect,
        };
        match edge {
            ChromeEdge::Top => self.top = Some(snapshot),
            ChromeEdge::Bottom => self.bottom = Some(snapshot),
        }
        self.generation = self.generation.max(generation);
        Ok(())
    }

    /// The host has nothing to draw, or the surface went away: free both images.
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    /// Draw the snapshots into `display`'s slots; the generation drawn, or 0.
    pub(crate) fn attach(&mut self, display: &mut DisplayList) -> u64 {
        if self.top.is_none() && self.bottom.is_none() {
            return 0;
        }
        // Closed, exit included: the bars are back on top, and the images are not needed.
        if !display.top_layer_open() {
            self.clear();
            return 0;
        }
        let surface = (display.width, display.height, display.scale.to_bits());
        if *self.surface.get_or_insert(surface) != surface {
            // Rotation, a fold or new insets moved the bars; the host captures them again.
            self.clear();
            return 0;
        }
        let snapshots: Vec<_> = [
            (ChromeEdge::Top, &self.top),
            (ChromeEdge::Bottom, &self.bottom),
        ]
        .into_iter()
        .filter_map(|(edge, snapshot)| snapshot.as_ref().map(|s| (edge, &s.bitmap, s.rect)))
        .collect();
        if display.attach_chrome(&snapshots) {
            self.generation
        } else {
            0
        }
    }
}
