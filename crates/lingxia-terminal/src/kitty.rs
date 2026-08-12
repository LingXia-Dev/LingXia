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
