//! Writes the `.DS_Store` that lays out the DMG install window, so packaging
//! needs neither Finder nor a GUI session. The file has the shape Finder writes
//! for a small store: a buddy-allocated file holding a single-leaf `DSDB`
//! B-tree, with the background referenced by an alias that resolves by volume
//! name and path.

use anyhow::{Context, Result, ensure};
use plist::Value;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

pub struct DmgLayout<'a> {
    pub volume_name: &'a str,
    pub app_bundle_name: &'a str,
    /// Icon centres, in points.
    pub app_position: (u32, u32),
    pub applications_position: (u32, u32),
    /// Content size; the background fills it exactly.
    pub window_size: (u32, u32),
    /// File name in the volume's `.background` folder.
    pub background_name: &'a str,
}

/// Finder's title bar; `WindowBounds` includes it.
const TITLE_BAR_HEIGHT: u32 = 28;
/// Block placement (offset, log2 size) relative to file byte 4, as Finder
/// lays out a small store: allocator header, DSDB header, leaf, root block.
const DSDB: (u32, u32) = (0x40, 5);
const LEAF: (u32, u32) = (0x1000, 12);
const ROOT: (u32, u32) = (0x2000, 11);
const BLOCKS: [(u32, u32); 4] = [(0, 5), DSDB, LEAF, ROOT];

/// Write `<volume_root>/.DS_Store`. The volume is mounted at `volume_root`
/// only while packaging; nothing in the store depends on that path.
pub fn write(volume_root: &Path, layout: &DmgLayout) -> Result<()> {
    let alias = background_alias(volume_root, layout);
    let mut records = [
        (".", *b"bwsp", Some(window_settings(layout)?)),
        (".", *b"icvp", Some(icon_view_options(alias)?)),
        (".", *b"vSrn", None),
        (
            layout.app_bundle_name,
            *b"Iloc",
            Some(icon_location(layout.app_position)),
        ),
        (
            "Applications",
            *b"Iloc",
            Some(icon_location(layout.applications_position)),
        ),
    ];
    // Finder orders records by case-folded name, then structure id.
    records.sort_by_key(|(name, id, _)| (name.to_lowercase(), *id));

    let mut leaf = Vec::new();
    put_u32(&mut leaf, 0); // no children
    put_u32(&mut leaf, records.len() as u32);
    for (name, id, blob) in &records {
        let name: Vec<u16> = name.encode_utf16().collect();
        put_u32(&mut leaf, name.len() as u32);
        name.iter().for_each(|unit| leaf.extend(unit.to_be_bytes()));
        leaf.extend(id);
        match blob {
            Some(blob) => {
                leaf.extend(b"blob");
                put_u32(&mut leaf, blob.len() as u32);
                leaf.extend(blob);
            }
            None => {
                leaf.extend(b"long");
                put_u32(&mut leaf, 1);
            }
        }
    }
    ensure!(
        leaf.len() <= 1 << LEAF.1,
        "DMG .DS_Store records exceed one page"
    );

    let store = buddy_file(&leaf, records.len() as u32);
    fs::write(volume_root.join(".DS_Store"), store).context("Failed to write the DMG .DS_Store")
}

fn icon_location((x, y): (u32, u32)) -> Vec<u8> {
    let mut out = Vec::with_capacity(16);
    put_u32(&mut out, x);
    put_u32(&mut out, y);
    out.extend([0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0, 0]);
    out
}

fn window_settings(layout: &DmgLayout) -> Result<Vec<u8>> {
    let (width, height) = layout.window_size;
    // Bottom-left origin; Finder keeps the window on screen.
    let bounds = format!(
        "{{{{200, 320}}, {{{width}, {}}}}}",
        height + TITLE_BAR_HEIGHT
    );
    binary_plist([
        ("WindowBounds", bounds.into()),
        ("ShowStatusBar", false.into()),
        ("ShowToolbar", false.into()),
        ("ShowTabView", false.into()),
        ("ShowPathbar", false.into()),
        ("ShowSidebar", false.into()),
        ("ContainerShowSidebar", false.into()),
    ])
}

fn icon_view_options(background_alias: Vec<u8>) -> Result<Vec<u8>> {
    binary_plist([
        ("viewOptionsVersion", 1.into()),
        ("arrangeBy", "none".into()),
        ("iconSize", 128.0.into()),
        ("textSize", 14.0.into()),
        ("gridSpacing", 100.0.into()),
        ("gridOffsetX", 0.0.into()),
        ("gridOffsetY", 0.0.into()),
        ("labelOnBottom", true.into()),
        ("showItemInfo", false.into()),
        ("showIconPreview", true.into()),
        ("backgroundType", 2.into()), // picture
        ("backgroundColorRed", 1.0.into()),
        ("backgroundColorGreen", 1.0.into()),
        ("backgroundColorBlue", 1.0.into()),
        ("backgroundImageAlias", Value::Data(background_alias)),
    ])
}

fn binary_plist<const N: usize>(entries: [(&str, Value); N]) -> Result<Vec<u8>> {
    let dict = entries
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect();
    let mut out = Vec::new();
    plist::to_writer_binary(&mut out, &Value::Dictionary(dict))
        .context("Failed to encode a DMG plist")?;
    Ok(out)
}

/// A version-2 alias record, as Finder writes one for a file on an APFS
/// image: no catalog ids (`0xFFFFFFFF`), resolved by volume name and path.
fn background_alias(volume_root: &Path, layout: &DmgLayout) -> Vec<u8> {
    let (volume, file) = (layout.volume_name, layout.background_name);
    let background = volume_root.join(".background").join(file);

    let mut out = vec![0; 8]; // app info, size and version, filled below
    out.extend(0u16.to_be_bytes()); // kind: file
    out.extend(pascal(volume, 28));
    put_u32(&mut out, hfs_time(volume_root));
    out.extend(b"BD\0\x01"); // file system signature, disk type
    put_u32(&mut out, u32::MAX); // parent id
    out.extend(pascal(file, 64));
    put_u32(&mut out, u32::MAX); // file id
    put_u32(&mut out, hfs_time(&background));
    out.extend([0; 8]); // creator, type
    out.extend([0xff; 4]); // levels from / to
    put_u32(&mut out, 0x0e02); // volume attributes
    out.extend(b"cu");
    out.extend([0; 10]);

    let utf16 = |text: &str| -> Vec<u8> {
        let units: Vec<u16> = text.encode_utf16().collect();
        let mut out = (units.len() as u16).to_be_bytes().to_vec();
        units.iter().for_each(|unit| out.extend(unit.to_be_bytes()));
        out
    };
    let tags: [(u16, Vec<u8>); 6] = [
        (0, b".background".to_vec()),
        (2, format!("{volume}:.background:{file}").into_bytes()),
        (14, utf16(file)),
        (15, utf16(volume)),
        (18, format!("/.background/{file}").into_bytes()),
        (19, format!("/Volumes/{volume}").into_bytes()),
    ];
    for (tag, value) in tags {
        out.extend(tag.to_be_bytes());
        out.extend((value.len() as u16).to_be_bytes());
        out.extend(&value);
        if value.len() % 2 == 1 {
            out.push(0);
        }
    }
    out.extend([0xff, 0xff, 0, 0]);

    let size = out.len() as u16;
    out[4..6].copy_from_slice(&size.to_be_bytes());
    out[6..8].copy_from_slice(&2u16.to_be_bytes());
    out
}

/// A Pascal string in a fixed-width field; non-ASCII becomes `?` (the UTF-16
/// tags carry the real name).
fn pascal(text: &str, width: usize) -> Vec<u8> {
    let mut out = vec![0; width];
    let bytes = text
        .chars()
        .take(width - 1)
        .map(|c| if c.is_ascii() { c as u8 } else { b'?' });
    let mut len = 0;
    for (slot, byte) in out[1..].iter_mut().zip(bytes) {
        *slot = byte;
        len += 1;
    }
    out[0] = len;
    out
}

/// Creation time in seconds since 1904-01-01.
fn hfs_time(path: &Path) -> u32 {
    const HFS_EPOCH_OFFSET: u64 = 2_082_844_800;
    let unix = fs::metadata(path)
        .and_then(|meta| meta.created())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |since| since.as_secs());
    (unix + HFS_EPOCH_OFFSET) as u32
}

fn buddy_file(leaf: &[u8], record_count: u32) -> Vec<u8> {
    let mut root = Vec::new();
    put_u32(&mut root, 3); // blocks: root, DSDB header, leaf
    put_u32(&mut root, 0);
    for slot in 0..256 {
        let address = [ROOT, DSDB, LEAF]
            .get(slot)
            .map_or(0, |(offset, log2)| offset | log2);
        put_u32(&mut root, address);
    }
    put_u32(&mut root, 1); // table of contents: DSDB -> block 1
    root.push(4);
    root.extend(b"DSDB");
    put_u32(&mut root, 1);
    let mut free = vec![Vec::new(); 32];
    free_blocks(0, 31, &mut free);
    for offsets in free {
        put_u32(&mut root, offsets.len() as u32);
        offsets
            .into_iter()
            .for_each(|offset| put_u32(&mut root, offset));
    }

    let mut dsdb = Vec::new();
    for value in [2, 0, record_count, 1, 1 << LEAF.1] {
        put_u32(&mut dsdb, value); // root node, levels, records, nodes, page size
    }

    let mut file = vec![0; 4 + (ROOT.0 + (1 << ROOT.1)) as usize];
    let mut head = Vec::new();
    for value in [
        1,
        u32::from_be_bytes(*b"Bud1"),
        ROOT.0,
        1 << ROOT.1,
        ROOT.0,
        LEAF.0 | LEAF.1,
    ] {
        put_u32(&mut head, value);
    }
    file[..head.len()].copy_from_slice(&head);
    for ((offset, _), bytes) in [(DSDB, &dsdb[..]), (LEAF, leaf), (ROOT, &root[..])] {
        let start = 4 + offset as usize;
        file[start..start + bytes.len()].copy_from_slice(bytes);
    }
    file
}

/// Free lists of a 2^31-byte buddy allocator with [`BLOCKS`] taken: every
/// maximal block that holds none of them, by size class.
fn free_blocks(offset: u32, log2: u32, free: &mut [Vec<u32>]) {
    if BLOCKS.contains(&(offset, log2)) {
        return;
    }
    let end = offset as u64 + (1u64 << log2);
    if !BLOCKS
        .iter()
        .any(|&(used, _)| (offset as u64..end).contains(&(used as u64)))
    {
        free[log2 as usize].push(offset);
        return;
    }
    free_blocks(offset, log2 - 1, free);
    free_blocks(offset + (1 << (log2 - 1)), log2 - 1, free);
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend(value.to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_lists_match_finders_for_the_same_placement() {
        let mut free = vec![Vec::new(); 32];
        free_blocks(0, 31, &mut free);
        assert_eq!(free[5], [0x20, 0x60]);
        assert_eq!(free[7], [0x80]);
        assert_eq!(free[11], [0x800, 0x2800]);
        assert_eq!(free[12], [0x3000]);
        assert!(free[13].is_empty());
        assert_eq!(free[30], [0x4000_0000]);
    }

    #[test]
    fn writes_the_window_layout() {
        let volume = tempfile::tempdir().unwrap();
        fs::create_dir(volume.path().join(".background")).unwrap();
        fs::write(volume.path().join(".background/background.tiff"), b"x").unwrap();
        let layout = DmgLayout {
            volume_name: "Fusheng",
            app_bundle_name: "Alpha.app",
            app_position: (180, 190),
            applications_position: (480, 190),
            window_size: (660, 400),
            background_name: "background.tiff",
        };
        write(volume.path(), &layout).unwrap();

        let bytes = fs::read(volume.path().join(".DS_Store")).unwrap();
        assert_eq!(&bytes[..8], b"\0\0\0\x01Bud1");
        assert_eq!(bytes.len(), 4 + 0x2800);
        assert_eq!(&bytes[4 + 0x1000..4 + 0x1008], &[0, 0, 0, 0, 0, 0, 0, 5]);
        let at = |needle: &[u8]| {
            bytes
                .windows(needle.len())
                .position(|w| w == needle)
                .unwrap()
        };
        assert!(at(b"{{200, 320}, {660, 428}}") > 0);
        assert!(at(b"/.background/background.tiff") > 0);
        // Case-folded order: "Alpha.app" before "Applications".
        let utf16 = |s: &str| {
            s.encode_utf16()
                .flat_map(u16::to_be_bytes)
                .collect::<Vec<_>>()
        };
        assert!(at(&utf16("Alpha.app")) < at(&utf16("Applications")));
    }
}
