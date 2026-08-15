//! Static-image subset of the Kitty graphics protocol.
//!
//! The parser and image/placement state live in the shared terminal engine so
//! every host observes identical terminal semantics. Renderers consume a
//! generation-based snapshot separately from the character-cell frame.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use flate2::read::ZlibDecoder;
use image::codecs::png::PngEncoder;
use image::{ColorType, GenericImageView, ImageEncoder, ImageFormat, ImageReader, Limits};
use serde::Serialize;
use std::collections::BTreeMap;
use std::io::{Cursor, Read};

const MAX_TRANSFER_BYTES: usize = 64 * 1024 * 1024;
const MAX_IMAGE_BYTES: usize = 128 * 1024 * 1024;
const MAX_IMAGE_DIMENSION: u32 = 16_384;
const MAX_IMAGES: usize = 128;
const MAX_PLACEMENTS: usize = 512;

/// Kitty's canonical row/column diacritic table for Unicode placeholders.
/// The position in this table is the encoded coordinate value.
const PLACEHOLDER_DIACRITICS: &[u32] = &[
    0x0305, 0x030D, 0x030E, 0x0310, 0x0312, 0x033D, 0x033E, 0x033F, 0x0346, 0x034A, 0x034B, 0x034C,
    0x0350, 0x0351, 0x0352, 0x0357, 0x035B, 0x0363, 0x0364, 0x0365, 0x0366, 0x0367, 0x0368, 0x0369,
    0x036A, 0x036B, 0x036C, 0x036D, 0x036E, 0x036F, 0x0483, 0x0484, 0x0485, 0x0486, 0x0487, 0x0592,
    0x0593, 0x0594, 0x0595, 0x0597, 0x0598, 0x0599, 0x059C, 0x059D, 0x059E, 0x059F, 0x05A0, 0x05A1,
    0x05A8, 0x05A9, 0x05AB, 0x05AC, 0x05AF, 0x05C4, 0x0610, 0x0611, 0x0612, 0x0613, 0x0614, 0x0615,
    0x0616, 0x0617, 0x0657, 0x0658, 0x0659, 0x065A, 0x065B, 0x065D, 0x065E, 0x06D6, 0x06D7, 0x06D8,
    0x06D9, 0x06DA, 0x06DB, 0x06DC, 0x06DF, 0x06E0, 0x06E1, 0x06E2, 0x06E4, 0x06E7, 0x06E8, 0x06EB,
    0x06EC, 0x0730, 0x0732, 0x0733, 0x0735, 0x0736, 0x073A, 0x073D, 0x073F, 0x0740, 0x0741, 0x0743,
    0x0745, 0x0747, 0x0749, 0x074A, 0x07EB, 0x07EC, 0x07ED, 0x07EE, 0x07EF, 0x07F0, 0x07F1, 0x07F3,
    0x0816, 0x0817, 0x0818, 0x0819, 0x081B, 0x081C, 0x081D, 0x081E, 0x081F, 0x0820, 0x0821, 0x0822,
    0x0823, 0x0825, 0x0826, 0x0827, 0x0829, 0x082A, 0x082B, 0x082C, 0x082D, 0x0951, 0x0953, 0x0954,
    0x0F82, 0x0F83, 0x0F86, 0x0F87, 0x135D, 0x135E, 0x135F, 0x17DD, 0x193A, 0x1A17, 0x1A75, 0x1A76,
    0x1A77, 0x1A78, 0x1A79, 0x1A7A, 0x1A7B, 0x1A7C, 0x1B6B, 0x1B6D, 0x1B6E, 0x1B6F, 0x1B70, 0x1B71,
    0x1B72, 0x1B73, 0x1CD0, 0x1CD1, 0x1CD2, 0x1CDA, 0x1CDB, 0x1CE0, 0x1DC0, 0x1DC1, 0x1DC3, 0x1DC4,
    0x1DC5, 0x1DC6, 0x1DC7, 0x1DC8, 0x1DC9, 0x1DCB, 0x1DCC, 0x1DD1, 0x1DD2, 0x1DD3, 0x1DD4, 0x1DD5,
    0x1DD6, 0x1DD7, 0x1DD8, 0x1DD9, 0x1DDA, 0x1DDB, 0x1DDC, 0x1DDD, 0x1DDE, 0x1DDF, 0x1DE0, 0x1DE1,
    0x1DE2, 0x1DE3, 0x1DE4, 0x1DE5, 0x1DE6, 0x1DFE, 0x20D0, 0x20D1, 0x20D4, 0x20D5, 0x20D6, 0x20D7,
    0x20DB, 0x20DC, 0x20E1, 0x20E7, 0x20E9, 0x20F0, 0x2CEF, 0x2CF0, 0x2CF1, 0x2DE0, 0x2DE1, 0x2DE2,
    0x2DE3, 0x2DE4, 0x2DE5, 0x2DE6, 0x2DE7, 0x2DE8, 0x2DE9, 0x2DEA, 0x2DEB, 0x2DEC, 0x2DED, 0x2DEE,
    0x2DEF, 0x2DF0, 0x2DF1, 0x2DF2, 0x2DF3, 0x2DF4, 0x2DF5, 0x2DF6, 0x2DF7, 0x2DF8, 0x2DF9, 0x2DFA,
    0x2DFB, 0x2DFC, 0x2DFD, 0x2DFE, 0x2DFF, 0xA66F, 0xA67C, 0xA67D, 0xA6F0, 0xA6F1, 0xA8E0, 0xA8E1,
    0xA8E2, 0xA8E3, 0xA8E4, 0xA8E5, 0xA8E6, 0xA8E7, 0xA8E8, 0xA8E9, 0xA8EA, 0xA8EB, 0xA8EC, 0xA8ED,
    0xA8EE, 0xA8EF, 0xA8F0, 0xA8F1, 0xAAB0, 0xAAB2, 0xAAB3, 0xAAB7, 0xAAB8, 0xAABE, 0xAABF, 0xAAC1,
    0xFE20, 0xFE21, 0xFE22, 0xFE23, 0xFE24, 0xFE25, 0xFE26, 0x10A0F, 0x10A38, 0x1D185, 0x1D186,
    0x1D187, 0x1D188, 0x1D189, 0x1D1AA, 0x1D1AB, 0x1D1AC, 0x1D1AD, 0x1D242, 0x1D243, 0x1D244,
];

pub fn placeholder_diacritic_index(scalar: u32) -> Option<usize> {
    PLACEHOLDER_DIACRITICS
        .iter()
        .position(|candidate| *candidate == scalar)
}

/// Image coordinates encoded by one Kitty Unicode placeholder cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnicodePlaceholder {
    pub image_id: u32,
    pub placement_id: u32,
    pub image_row: usize,
    pub image_col: usize,
}

/// Decode the colors and combining marks used by Kitty's U+10EEEE placeholders.
///
/// Kitty permits later cells to omit coordinates that follow the preceding
/// placeholder. `previous` supplies that state; callers reset it when ordinary
/// terminal content interrupts a placeholder run.
pub fn decode_unicode_placeholder(
    cluster: &str,
    foreground: u32,
    underline_color: u32,
    previous: Option<UnicodePlaceholder>,
) -> Option<UnicodePlaceholder> {
    let mut scalars = cluster.chars();
    if scalars.next()? != '\u{10EEEE}' {
        return None;
    }
    let mut diacritics = scalars
        .take(3)
        .map(|scalar| placeholder_diacritic_index(u32::from(scalar)));
    let mut image_row = diacritics.next().flatten();
    let mut image_col = diacritics.next().flatten();
    let mut high_byte = diacritics.next().flatten();
    if high_byte.is_some_and(|value| value > u8::MAX as usize) {
        return None;
    }

    let color_image_id = foreground >> 8;
    let placement_id = underline_color >> 8;
    if let Some(previous) = previous.filter(|previous| {
        previous.image_id & 0x00FF_FFFF == color_image_id && previous.placement_id == placement_id
    }) {
        if image_row.is_none() {
            image_row = Some(previous.image_row);
            image_col = Some(previous.image_col + 1);
            high_byte = Some((previous.image_id >> 24) as usize);
        } else if image_col.is_none() && image_row == Some(previous.image_row) {
            image_col = Some(previous.image_col + 1);
            high_byte = Some((previous.image_id >> 24) as usize);
        } else if high_byte.is_none()
            && image_row == Some(previous.image_row)
            && image_col == Some(previous.image_col + 1)
        {
            high_byte = Some((previous.image_id >> 24) as usize);
        }
    }

    Some(UnicodePlaceholder {
        image_id: color_image_id | ((high_byte.unwrap_or(0) as u32) << 24),
        placement_id,
        image_row: image_row.unwrap_or(0),
        image_col: image_col.unwrap_or(0),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphicsAnchor {
    pub line: i64,
    pub col: u16,
    pub alternate_screen: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalImageSnapshot {
    pub changed: bool,
    pub generation: u64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<TerminalImage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub placements: Vec<TerminalImagePlacement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalImage {
    pub id: u32,
    pub width: u32,
    pub height: u32,
    pub png_base64: String,
    /// Original PNG bytes for in-process renderers. FFI/JSON hosts keep using
    /// `pngBase64`, so this never expands the serialized contract.
    #[serde(skip)]
    pub png: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalImagePlacement {
    pub image_id: u32,
    pub placement_id: u32,
    /// Absolute terminal line (oldest retained scrollback line = 0).
    pub line: i64,
    pub col: u16,
    pub columns: u16,
    pub rows: u16,
    pub x_offset: u16,
    pub y_offset: u16,
    pub source_x: u32,
    pub source_y: u32,
    pub source_width: u32,
    pub source_height: u32,
    pub z_index: i32,
    pub alternate_screen: bool,
    /// Prototype used by U+10EEEE cells; it has no screen position itself.
    pub virtual_placement: bool,
}

#[derive(Debug, Clone)]
struct StoredImage {
    width: u32,
    height: u32,
    png: Vec<u8>,
    byte_cost: usize,
}

#[derive(Debug, Clone)]
struct PendingTransfer {
    command: Command,
    encoded: Vec<u8>,
}

#[derive(Debug, Clone)]
struct Command {
    action: u8,
    format: u32,
    medium: u8,
    compression: Option<u8>,
    more: bool,
    quiet: u8,
    image_id: u32,
    placement_id: u32,
    width: u32,
    height: u32,
    columns: u16,
    rows: u16,
    source_x: u32,
    source_y: u32,
    source_width: u32,
    source_height: u32,
    x_offset: u16,
    y_offset: u16,
    z_index: i32,
    no_cursor_move: bool,
    unicode_placeholder: bool,
    delete: Option<u8>,
}

impl Default for Command {
    fn default() -> Self {
        Self {
            action: b't',
            format: 32,
            medium: b'd',
            compression: None,
            more: false,
            quiet: 0,
            image_id: 0,
            placement_id: 0,
            width: 0,
            height: 0,
            columns: 0,
            rows: 0,
            source_x: 0,
            source_y: 0,
            source_width: 0,
            source_height: 0,
            x_offset: 0,
            y_offset: 0,
            z_index: 0,
            no_cursor_move: false,
            unicode_placeholder: false,
            delete: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraphicsResult {
    pub response: Option<Vec<u8>>,
    /// Kitty's default placement policy advances by the placement rectangle.
    pub cursor_move: Option<(u16, u16)>,
}

#[derive(Default)]
pub struct KittyGraphics {
    generation: u64,
    next_image_id: u32,
    images: BTreeMap<u32, StoredImage>,
    placements: Vec<TerminalImagePlacement>,
    pending: Option<PendingTransfer>,
    total_bytes: usize,
}

impl KittyGraphics {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn snapshot(&self, since_generation: u64) -> TerminalImageSnapshot {
        if since_generation == self.generation {
            return TerminalImageSnapshot {
                changed: false,
                generation: self.generation,
                ..TerminalImageSnapshot::default()
            };
        }
        TerminalImageSnapshot {
            changed: true,
            generation: self.generation,
            images: self
                .images
                .iter()
                .map(|(&id, image)| TerminalImage {
                    id,
                    width: image.width,
                    height: image.height,
                    png_base64: BASE64.encode(&image.png),
                    png: image.png.clone(),
                })
                .collect(),
            placements: self.placements.clone(),
        }
    }

    /// Move physical placements with an alternate-screen scroll. Alternate
    /// grids have no history, so their anchors must move when rows scroll.
    pub fn scroll_alternate_screen(&mut self, rows: u16) {
        if rows == 0 {
            return;
        }
        let before = self.placements.len();
        let rows = i64::from(rows);
        for placement in &mut self.placements {
            if placement.alternate_screen && !placement.virtual_placement {
                placement.line -= rows;
            }
        }
        self.placements.retain(|placement| {
            placement.virtual_placement
                || !placement.alternate_screen
                || placement.line + i64::from(placement.rows) > 0
        });
        if self.placements.len() != before
            || self
                .placements
                .iter()
                .any(|placement| placement.alternate_screen && !placement.virtual_placement)
        {
            self.bump_generation();
        }
    }

    /// Drop pixel-positioned placements when the character grid reflows.
    /// Their stored image data remains available for a fresh placement.
    pub fn clear_physical_placements(&mut self) {
        let before = self.placements.len();
        self.placements
            .retain(|placement| placement.virtual_placement);
        if self.placements.len() != before {
            self.bump_generation();
        }
    }

    /// Handle the body of APC G ... ST (the leading G is omitted).
    pub fn handle(
        &mut self,
        body: &[u8],
        anchor: GraphicsAnchor,
        cell_width: u16,
        cell_height: u16,
    ) -> GraphicsResult {
        let (control, payload) = split_once_byte(body, b';').unwrap_or((body, &[]));
        let command = match parse_command(control) {
            Ok(command) => command,
            Err(error) => return GraphicsResult::reply(0, 0, 0, Err(error)),
        };

        if command.action == b'd' {
            return self.delete(&command);
        }
        if command.action == b'p' {
            return self.place(&command, anchor, cell_width, cell_height);
        }
        if !matches!(command.action, b't' | b'T' | b'q') {
            return GraphicsResult::reply_for(&command, Err("EINVAL:unsupported action"));
        }
        if command.medium != b'd' {
            return GraphicsResult::reply_for(&command, Err("ENOTSUP:direct transmission only"));
        }

        let continuing = self.pending.is_some()
            && control
                .split(|byte| *byte == b',')
                .all(|field| field.is_empty() || field.starts_with(b"m="));
        if command.more || continuing {
            if continuing {
                let Some(pending) = self.pending.as_mut() else {
                    unreachable!()
                };
                if pending.encoded.len().saturating_add(payload.len()) > MAX_TRANSFER_BYTES * 2 {
                    let command = pending.command.clone();
                    self.pending = None;
                    return GraphicsResult::reply_for(
                        &command,
                        Err("EFBIG:encoded payload too large"),
                    );
                }
                pending.encoded.extend_from_slice(payload);
                if command.more {
                    return GraphicsResult::default();
                }
            } else {
                if payload.len() > MAX_TRANSFER_BYTES * 2 {
                    return GraphicsResult::reply_for(
                        &command,
                        Err("EFBIG:encoded payload too large"),
                    );
                }
                self.pending = Some(PendingTransfer {
                    command: command.clone(),
                    encoded: payload.to_vec(),
                });
                return GraphicsResult::default();
            }
        }

        let (command, payload) = match self.pending.take() {
            Some(pending) => (pending.command, pending.encoded),
            None => (command, payload.to_vec()),
        };
        self.finish_transfer(command, anchor, &payload, cell_width, cell_height)
    }

    fn finish_transfer(
        &mut self,
        mut command: Command,
        anchor: GraphicsAnchor,
        encoded: &[u8],
        cell_width: u16,
        cell_height: u16,
    ) -> GraphicsResult {
        let decoded = match BASE64.decode(encoded) {
            Ok(decoded) if decoded.len() <= MAX_TRANSFER_BYTES => decoded,
            Ok(_) => return GraphicsResult::reply_for(&command, Err("EFBIG:payload too large")),
            Err(_) => return GraphicsResult::reply_for(&command, Err("EINVAL:invalid base64")),
        };
        let decoded = match command.compression {
            None => decoded,
            Some(b'z') => match inflate_limited(&decoded) {
                Ok(decoded) => decoded,
                Err(error) => return GraphicsResult::reply_for(&command, Err(error)),
            },
            Some(_) => return GraphicsResult::reply_for(&command, Err("ENOTSUP:compression")),
        };
        let image = match decode_image(&command, decoded) {
            Ok(image) => image,
            Err(error) => return GraphicsResult::reply_for(&command, Err(error)),
        };
        if command.action == b'q' {
            return GraphicsResult::reply_for(&command, Ok(()));
        }

        if command.image_id == 0 {
            command.image_id = self.allocate_image_id();
        }
        if self.images.len() >= MAX_IMAGES && !self.images.contains_key(&command.image_id) {
            return GraphicsResult::reply_for(&command, Err("ENOSPC:image count limit"));
        }
        let previous = self
            .images
            .get(&command.image_id)
            .map_or(0, |item| item.byte_cost);
        let next_total = self
            .total_bytes
            .saturating_sub(previous)
            .saturating_add(image.byte_cost);
        if next_total > MAX_IMAGE_BYTES {
            return GraphicsResult::reply_for(&command, Err("ENOSPC:image quota"));
        }
        self.total_bytes = next_total;
        self.images.insert(command.image_id, image);
        self.placements
            .retain(|placement| placement.image_id != command.image_id);
        self.bump_generation();

        let cursor_move = if command.action == b'T' {
            match self.place_inner(&command, anchor, cell_width, cell_height) {
                Ok(cursor_move) => cursor_move,
                Err(error) => return GraphicsResult::reply_for(&command, Err(error)),
            }
        } else {
            None
        };
        let mut result = GraphicsResult::reply_for(&command, Ok(()));
        result.cursor_move = cursor_move;
        result
    }

    fn place(
        &mut self,
        command: &Command,
        anchor: GraphicsAnchor,
        cell_width: u16,
        cell_height: u16,
    ) -> GraphicsResult {
        let cursor_move = match self.place_inner(command, anchor, cell_width, cell_height) {
            Ok(cursor_move) => cursor_move,
            Err(error) => return GraphicsResult::reply_for(command, Err(error)),
        };
        let mut result = GraphicsResult::reply_for(command, Ok(()));
        result.cursor_move = cursor_move;
        result
    }

    fn place_inner(
        &mut self,
        command: &Command,
        anchor: GraphicsAnchor,
        cell_width: u16,
        cell_height: u16,
    ) -> Result<Option<(u16, u16)>, &'static str> {
        let Some(image) = self.images.get(&command.image_id) else {
            return Err("ENOENT:image not found");
        };
        if self.placements.len() >= MAX_PLACEMENTS {
            return Err("ENOSPC:placement count limit");
        }
        let source_x = command.source_x.min(image.width.saturating_sub(1));
        let source_y = command.source_y.min(image.height.saturating_sub(1));
        let source_width = if command.source_width == 0 {
            image.width.saturating_sub(source_x)
        } else {
            command
                .source_width
                .min(image.width.saturating_sub(source_x))
        }
        .max(1);
        let source_height = if command.source_height == 0 {
            image.height.saturating_sub(source_y)
        } else {
            command
                .source_height
                .min(image.height.saturating_sub(source_y))
        }
        .max(1);
        let (columns, rows) = placement_cells(
            command.columns,
            command.rows,
            source_width,
            source_height,
            cell_width.max(1),
            cell_height.max(1),
        );
        if command.placement_id != 0 {
            self.placements.retain(|placement| {
                placement.image_id != command.image_id
                    || placement.placement_id != command.placement_id
            });
        }
        self.placements.push(TerminalImagePlacement {
            image_id: command.image_id,
            placement_id: command.placement_id,
            line: anchor.line,
            col: anchor.col,
            columns,
            rows,
            x_offset: command.x_offset.min(cell_width.saturating_sub(1)),
            y_offset: command.y_offset.min(cell_height.saturating_sub(1)),
            source_x,
            source_y,
            source_width,
            source_height,
            z_index: command.z_index,
            alternate_screen: anchor.alternate_screen,
            virtual_placement: command.unicode_placeholder,
        });
        self.bump_generation();
        Ok((!command.no_cursor_move && !command.unicode_placeholder).then_some((columns, rows)))
    }

    fn delete(&mut self, command: &Command) -> GraphicsResult {
        // Any delete command aborts an in-flight chunked upload, regardless
        // of whether its selector matches an existing image.
        self.pending = None;
        let before_placements = self.placements.len();
        let before_images = self.images.len();
        match command.delete.unwrap_or(b'a') {
            b'i' if command.image_id != 0 => {
                self.placements.retain(|placement| {
                    placement.image_id != command.image_id
                        || (command.placement_id != 0
                            && placement.placement_id != command.placement_id)
                });
            }
            b'p' if command.placement_id != 0 => {
                self.placements.retain(|placement| {
                    placement.virtual_placement || placement.placement_id != command.placement_id
                });
            }
            b'a' => self
                .placements
                .retain(|placement| placement.virtual_placement),
            b'I' if command.image_id != 0 => {
                self.placements.retain(|placement| {
                    placement.image_id != command.image_id
                        || (command.placement_id != 0
                            && placement.placement_id != command.placement_id)
                });
                if !self
                    .placements
                    .iter()
                    .any(|placement| placement.image_id == command.image_id)
                {
                    self.remove_image(command.image_id);
                }
            }
            b'A' => self
                .placements
                .retain(|placement| placement.virtual_placement),
            _ => return GraphicsResult::reply_for(command, Err("ENOTSUP:delete selector")),
        }
        if command.delete == Some(b'A') {
            self.remove_unreferenced_images();
        }
        if self.placements.len() != before_placements || self.images.len() != before_images {
            self.bump_generation();
        }
        GraphicsResult::reply_for(command, Ok(()))
    }

    fn remove_image(&mut self, image_id: u32) {
        if let Some(image) = self.images.remove(&image_id) {
            self.total_bytes = self.total_bytes.saturating_sub(image.byte_cost);
        }
    }

    fn remove_unreferenced_images(&mut self) {
        let referenced: std::collections::BTreeSet<_> = self
            .placements
            .iter()
            .map(|placement| placement.image_id)
            .collect();
        self.images.retain(|image_id, image| {
            if referenced.contains(image_id) {
                true
            } else {
                self.total_bytes = self.total_bytes.saturating_sub(image.byte_cost);
                false
            }
        });
    }

    fn allocate_image_id(&mut self) -> u32 {
        loop {
            self.next_image_id = self.next_image_id.wrapping_add(1).max(1);
            if !self.images.contains_key(&self.next_image_id) {
                return self.next_image_id;
            }
        }
    }

    fn bump_generation(&mut self) {
        self.generation = self.generation.wrapping_add(1).max(1);
    }
}

impl GraphicsResult {
    fn reply_for(command: &Command, result: Result<(), &'static str>) -> Self {
        Self::reply(
            command.image_id,
            command.placement_id,
            command.quiet,
            result,
        )
    }

    fn reply(
        image_id: u32,
        placement_id: u32,
        quiet: u8,
        result: Result<(), &'static str>,
    ) -> Self {
        let message = match result {
            Ok(()) if quiet >= 1 => return Self::default(),
            Err(_) if quiet >= 2 => return Self::default(),
            Ok(()) => "OK",
            Err(error) => error,
        };
        let placement = (placement_id != 0).then(|| format!(",p={placement_id}"));
        let response = format!(
            "\x1b_Gi={image_id}{};{message}\x1b\\",
            placement.as_deref().unwrap_or("")
        );
        Self {
            response: Some(response.into_bytes()),
            cursor_move: None,
        }
    }
}

fn parse_command(control: &[u8]) -> Result<Command, &'static str> {
    let mut command = Command::default();
    for field in control
        .split(|byte| *byte == b',')
        .filter(|field| !field.is_empty())
    {
        let Some((key, value)) = split_once_byte(field, b'=') else {
            return Err("EINVAL:control field");
        };
        let value_number = || parse_u32(value).ok_or("EINVAL:number");
        match key {
            b"a" => command.action = *value.first().ok_or("EINVAL:action")?,
            b"f" => command.format = value_number()?,
            b"t" => command.medium = *value.first().ok_or("EINVAL:medium")?,
            b"o" => command.compression = value.first().copied(),
            b"m" => command.more = value_number()? != 0,
            b"q" => command.quiet = value_number()?.min(2) as u8,
            b"i" => command.image_id = value_number()?,
            b"p" => command.placement_id = value_number()?,
            b"s" => command.width = value_number()?,
            b"v" => command.height = value_number()?,
            b"c" => command.columns = value_number()?.min(u16::MAX as u32) as u16,
            b"r" => command.rows = value_number()?.min(u16::MAX as u32) as u16,
            b"x" => command.source_x = value_number()?,
            b"y" => command.source_y = value_number()?,
            b"w" => command.source_width = value_number()?,
            b"h" => command.source_height = value_number()?,
            b"X" => command.x_offset = value_number()?.min(u16::MAX as u32) as u16,
            b"Y" => command.y_offset = value_number()?.min(u16::MAX as u32) as u16,
            b"z" => {
                command.z_index = std::str::from_utf8(value)
                    .ok()
                    .and_then(|value| value.parse().ok())
                    .ok_or("EINVAL:z-index")?;
            }
            b"C" => command.no_cursor_move = value_number()? == 1,
            b"U" => command.unicode_placeholder = value_number()? == 1,
            b"d" => command.delete = value.first().copied(),
            // Image number, byte size/offset and usage hints are accepted but
            // not needed by the direct static-image subset.
            b"I" | b"S" | b"O" | b"N" => {}
            _ => {}
        }
    }
    Ok(command)
}

fn split_once_byte(bytes: &[u8], needle: u8) -> Option<(&[u8], &[u8])> {
    let index = bytes.iter().position(|byte| *byte == needle)?;
    Some((&bytes[..index], &bytes[index + 1..]))
}

fn parse_u32(value: &[u8]) -> Option<u32> {
    std::str::from_utf8(value).ok()?.parse().ok()
}

fn inflate_limited(bytes: &[u8]) -> Result<Vec<u8>, &'static str> {
    let mut decoded = Vec::new();
    ZlibDecoder::new(bytes)
        .take(MAX_TRANSFER_BYTES as u64 + 1)
        .read_to_end(&mut decoded)
        .map_err(|_| "EINVAL:zlib payload")?;
    if decoded.len() > MAX_TRANSFER_BYTES {
        return Err("EFBIG:inflated payload too large");
    }
    Ok(decoded)
}

fn decode_image(command: &Command, bytes: Vec<u8>) -> Result<StoredImage, &'static str> {
    match command.format {
        100 => {
            let mut reader = ImageReader::with_format(Cursor::new(&bytes), ImageFormat::Png);
            let mut limits = Limits::default();
            limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
            limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
            limits.max_alloc = Some(MAX_TRANSFER_BYTES as u64);
            reader.limits(limits);
            let image = reader.decode().map_err(|_| "EINVAL:invalid PNG")?;
            let (width, height) = image.dimensions();
            validate_dimensions(width, height)?;
            Ok(StoredImage {
                width,
                height,
                png: bytes,
                byte_cost: decoded_byte_cost(width, height),
            })
        }
        24 | 32 => {
            validate_dimensions(command.width, command.height)?;
            let channels = if command.format == 24 { 3 } else { 4 };
            let expected = command.width as usize * command.height as usize * channels;
            if bytes.len() != expected {
                return Err("EINVAL:pixel payload size");
            }
            let rgba = if channels == 4 {
                bytes
            } else {
                bytes
                    .chunks_exact(3)
                    .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 0xff])
                    .collect()
            };
            let mut png = Vec::new();
            PngEncoder::new(&mut png)
                .write_image(
                    &rgba,
                    command.width,
                    command.height,
                    ColorType::Rgba8.into(),
                )
                .map_err(|_| "EINVAL:PNG encode")?;
            Ok(StoredImage {
                width: command.width,
                height: command.height,
                png,
                byte_cost: decoded_byte_cost(command.width, command.height),
            })
        }
        _ => Err("ENOTSUP:image format"),
    }
}

fn validate_dimensions(width: u32, height: u32) -> Result<(), &'static str> {
    if width == 0 || height == 0 {
        return Err("EINVAL:image dimensions");
    }
    if width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION {
        return Err("EFBIG:image dimensions");
    }
    let decoded = width as u64 * height as u64 * 4;
    if decoded > MAX_TRANSFER_BYTES as u64 {
        return Err("EFBIG:decoded image");
    }
    Ok(())
}

fn decoded_byte_cost(width: u32, height: u32) -> usize {
    (u64::from(width) * u64::from(height) * 4).min(usize::MAX as u64) as usize
}

fn placement_cells(
    columns: u16,
    rows: u16,
    width: u32,
    height: u32,
    cell_width: u16,
    cell_height: u16,
) -> (u16, u16) {
    let ceil = |value: u64, divisor: u64| value.div_ceil(divisor).min(u16::MAX as u64) as u16;
    match (columns, rows) {
        (0, 0) => (
            ceil(width as u64, cell_width as u64).max(1),
            ceil(height as u64, cell_height as u64).max(1),
        ),
        (columns, 0) => {
            let pixel_width = u64::from(columns) * u64::from(cell_width);
            let pixel_height = pixel_width * u64::from(height) / u64::from(width.max(1));
            (
                columns.max(1),
                ceil(pixel_height, cell_height as u64).max(1),
            )
        }
        (0, rows) => {
            let pixel_height = u64::from(rows) * u64::from(cell_height);
            let pixel_width = pixel_height * u64::from(width) / u64::from(height.max(1));
            (ceil(pixel_width, cell_width as u64).max(1), rows.max(1))
        }
        (columns, rows) => (columns.max(1), rows.max(1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anchor() -> GraphicsAnchor {
        GraphicsAnchor {
            line: 7,
            col: 3,
            alternate_screen: false,
        }
    }

    #[test]
    fn query_reports_direct_rgb_support_without_storing() {
        let mut graphics = KittyGraphics::default();
        let result = graphics.handle(b"a=q,i=31,f=24,s=1,v=1;AAAA", anchor(), 8, 16);
        assert_eq!(
            result.response.as_deref(),
            Some(b"\x1b_Gi=31;OK\x1b\\".as_slice())
        );
        assert!(graphics.images.is_empty());
    }

    #[test]
    fn chunked_rgba_transmit_and_place_produces_one_snapshot() {
        let mut graphics = KittyGraphics::default();
        let first = graphics.handle(b"a=T,f=32,s=1,v=1,i=9,c=2,r=3,m=1;", anchor(), 8, 16);
        assert!(first.response.is_none());
        let done = graphics.handle(b"m=0;/wAA/w==", anchor(), 8, 16);
        assert_eq!(done.cursor_move, Some((2, 3)));
        let snapshot = graphics.snapshot(0);
        assert!(snapshot.changed);
        assert_eq!(snapshot.images.len(), 1);
        assert_eq!(snapshot.placements.len(), 1);
        assert_eq!(snapshot.placements[0].line, 7);
        assert_eq!(snapshot.placements[0].col, 3);
    }

    #[test]
    fn chunked_placement_uses_the_final_chunk_anchor() {
        let mut graphics = KittyGraphics::default();
        let _ = graphics.handle(b"a=T,f=32,s=1,v=1,i=9,m=1;", anchor(), 8, 16);
        let final_anchor = GraphicsAnchor {
            line: 11,
            col: 5,
            alternate_screen: false,
        };
        let _ = graphics.handle(b"m=0;/wAA/w==", final_anchor, 8, 16);
        let snapshot = graphics.snapshot(0);
        assert_eq!(snapshot.placements[0].line, 11);
        assert_eq!(snapshot.placements[0].col, 5);
    }

    #[test]
    fn placement_derives_cell_size_and_delete_clears_it() {
        let mut graphics = KittyGraphics::default();
        let pixels = vec![0_u8; 16 * 32 * 3];
        let payload = BASE64.encode(pixels);
        let transmit = format!("a=t,f=24,s=16,v=32,i=4;{payload}");
        let _ = graphics.handle(transmit.as_bytes(), anchor(), 8, 16);
        let placed = graphics.handle(b"a=p,i=4,p=2,C=1", anchor(), 8, 16);
        assert_eq!(placed.cursor_move, None);
        let snapshot = graphics.snapshot(0);
        assert_eq!(
            (snapshot.placements[0].columns, snapshot.placements[0].rows),
            (2, 2)
        );
        let generation = snapshot.generation;
        let _ = graphics.handle(b"a=d,d=p,p=2", anchor(), 8, 16);
        let snapshot = graphics.snapshot(generation);
        assert!(snapshot.changed);
        assert!(snapshot.placements.is_empty());
    }

    #[test]
    fn unicode_placeholder_is_an_invisible_non_moving_prototype() {
        let mut graphics = KittyGraphics::default();
        let _ = graphics.handle(
            b"a=T,f=32,s=1,v=1,i=42,p=7,c=14,r=8,U=1;/wAA/w==",
            anchor(),
            8,
            16,
        );
        let placed = graphics.snapshot(0).placements.remove(0);
        assert!(placed.virtual_placement);
        assert_eq!((placed.image_id, placed.placement_id), (42, 7));
        assert_eq!((placed.columns, placed.rows), (14, 8));

        let result = graphics.handle(b"a=p,i=42,p=7,c=14,r=8,U=1", anchor(), 8, 16);
        assert_eq!(result.cursor_move, None);

        let _ = graphics.handle(b"a=d,d=a", anchor(), 8, 16);
        assert!(graphics.snapshot(0).placements[0].virtual_placement);
    }

    #[test]
    fn unicode_placeholder_decodes_color_ids_and_diacritics() {
        let cluster = format!("\u{10EEEE}{}{}{}", '\u{030D}', '\u{030E}', '\u{0310}');
        let decoded = decode_unicode_placeholder(&cluster, 0x1234_56FF_u32, 0x0000_07FF, None)
            .expect("placeholder should decode");
        assert_eq!(decoded.image_id, 0x03_123456);
        assert_eq!(decoded.placement_id, 7);
        assert_eq!((decoded.image_row, decoded.image_col), (1, 2));
    }

    #[test]
    fn unicode_placeholder_infers_a_horizontal_run() {
        let first = decode_unicode_placeholder(
            &format!("\u{10EEEE}{}{}{}", '\u{030D}', '\u{030E}', '\u{0310}'),
            0x1234_56FF,
            0x0000_07FF,
            None,
        )
        .unwrap();
        let next = decode_unicode_placeholder("\u{10EEEE}", 0x1234_56FF, 0x0000_07FF, Some(first))
            .unwrap();
        assert_eq!(next.image_id, first.image_id);
        assert_eq!(next.placement_id, first.placement_id);
        assert_eq!((next.image_row, next.image_col), (1, 3));
    }

    #[test]
    fn unicode_placeholder_rejects_an_oversized_high_byte() {
        let high = char::from_u32(PLACEHOLDER_DIACRITICS[256]).unwrap();
        let cluster = format!("\u{10EEEE}{}{}{}", '\u{0305}', '\u{0305}', high);
        assert!(decode_unicode_placeholder(&cluster, 0x2A_FF, 0, None).is_none());
    }

    #[test]
    fn in_process_png_bytes_are_not_serialized() {
        let mut graphics = KittyGraphics::default();
        let _ = graphics.handle(b"a=t,f=32,s=1,v=1,i=9;/wAA/w==", anchor(), 8, 16);
        let snapshot = graphics.snapshot(0);
        assert!(!snapshot.images[0].png.is_empty());
        let json = serde_json::to_value(snapshot).unwrap();
        assert!(json["images"][0].get("png").is_none());
        assert!(json["images"][0].get("pngBase64").is_some());
    }

    #[test]
    fn image_delete_honors_virtual_placement_id() {
        let mut graphics = KittyGraphics::default();
        for placement_id in [7, 8] {
            let command = format!("a=T,f=32,s=1,v=1,i=42,p={placement_id},c=14,r=8,U=1;/wAA/w==");
            let _ = graphics.handle(command.as_bytes(), anchor(), 8, 16);
        }

        let _ = graphics.handle(b"a=d,d=i,i=42,p=7", anchor(), 8, 16);
        let snapshot = graphics.snapshot(0);
        assert_eq!(snapshot.placements.len(), 1);
        assert_eq!(snapshot.placements[0].placement_id, 8);
        assert!(snapshot.images.iter().any(|image| image.id == 42));

        let _ = graphics.handle(b"a=d,d=I,i=42,p=8", anchor(), 8, 16);
        let snapshot = graphics.snapshot(0);
        assert!(snapshot.placements.is_empty());
        assert!(snapshot.images.iter().all(|image| image.id != 42));
    }

    #[test]
    fn freeing_one_placement_keeps_image_data_referenced_by_another() {
        let mut graphics = KittyGraphics::default();
        let _ = graphics.handle(
            b"a=T,f=32,s=1,v=1,i=42,p=7,c=2,r=2,C=1;/wAA/w==",
            anchor(),
            8,
            16,
        );
        let _ = graphics.handle(b"a=p,i=42,p=8,c=2,r=2,C=1", anchor(), 8, 16);

        let _ = graphics.handle(b"a=d,d=I,i=42,p=7", anchor(), 8, 16);
        let snapshot = graphics.snapshot(0);
        assert_eq!(snapshot.placements.len(), 1);
        assert_eq!(snapshot.placements[0].placement_id, 8);
        assert!(snapshot.images.iter().any(|image| image.id == 42));

        let _ = graphics.handle(b"a=d,d=I,i=42,p=8", anchor(), 8, 16);
        let snapshot = graphics.snapshot(0);
        assert!(snapshot.placements.is_empty());
        assert!(snapshot.images.iter().all(|image| image.id != 42));
    }

    #[test]
    fn delete_aborts_an_incomplete_transfer() {
        let mut graphics = KittyGraphics::default();
        let _ = graphics.handle(b"a=T,f=32,s=1,v=1,i=42,m=1;/w", anchor(), 8, 16);
        assert!(graphics.pending.is_some());

        let _ = graphics.handle(b"a=d,d=a", anchor(), 8, 16);
        assert!(graphics.pending.is_none());

        let result = graphics.handle(b"m=0;AA/w==", anchor(), 8, 16);
        assert!(result.response.as_deref().is_some_and(|response| {
            response
                .windows(b"EINVAL".len())
                .any(|part| part == b"EINVAL")
        }));
        assert!(graphics.snapshot(0).images.is_empty());
    }

    #[test]
    fn delete_all_and_free_preserves_only_virtual_prototype_data() {
        let mut graphics = KittyGraphics::default();
        let _ = graphics.handle(
            b"a=T,f=32,s=1,v=1,i=41,p=1,c=2,r=2,C=1;/wAA/w==",
            anchor(),
            8,
            16,
        );
        let _ = graphics.handle(
            b"a=T,f=32,s=1,v=1,i=42,p=2,c=2,r=2,U=1;/wAA/w==",
            anchor(),
            8,
            16,
        );

        let _ = graphics.handle(b"a=d,d=A", anchor(), 8, 16);
        let snapshot = graphics.snapshot(0);
        assert_eq!(snapshot.placements.len(), 1);
        assert!(snapshot.placements[0].virtual_placement);
        assert!(snapshot.images.iter().all(|image| image.id != 41));
        assert!(snapshot.images.iter().any(|image| image.id == 42));
    }

    #[test]
    fn alternate_screen_scroll_moves_and_clips_physical_placements() {
        let mut graphics = KittyGraphics::default();
        let alternate = GraphicsAnchor {
            line: 3,
            col: 0,
            alternate_screen: true,
        };
        let _ = graphics.handle(
            b"a=T,f=32,s=1,v=1,i=9,c=2,r=2,C=1;/wAA/w==",
            alternate,
            8,
            16,
        );
        let generation = graphics.generation();
        graphics.scroll_alternate_screen(2);
        assert!(graphics.generation() > generation);
        assert_eq!(graphics.snapshot(0).placements[0].line, 1);
        graphics.scroll_alternate_screen(3);
        assert!(graphics.snapshot(0).placements.is_empty());
    }
}
