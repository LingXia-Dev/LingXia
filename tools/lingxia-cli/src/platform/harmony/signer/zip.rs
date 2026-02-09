use anyhow::{Result, anyhow};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const MIN_EOCD_SIZE: u64 = 22;
const MAX_COMMENT_SIZE: u64 = 65535;

/// Information about a ZIP file structure relevant for signing.
#[derive(Debug)]
pub struct ZipInfo {
    pub cd_offset: u64,
    pub cd_size: u64,
    pub eocd_record: Vec<u8>,
}

/// Parse a ZIP file to find its structure.
pub fn parse_zip(path: &Path) -> Result<ZipInfo> {
    let mut file = File::open(path)?;
    let file_size = file.metadata()?.len();

    if file_size < MIN_EOCD_SIZE {
        return Err(anyhow!("File too small to be a ZIP"));
    }

    // Search for EOCD from the end
    // EOCD record is at end of file, variable length comment (0-65535 bytes)
    let max_search = std::cmp::min(file_size, MIN_EOCD_SIZE + MAX_COMMENT_SIZE);
    let start_search = file_size - max_search;

    file.seek(SeekFrom::Start(start_search))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    // Scan backwards for signature
    let buffer_len = buffer.len();
    for i in (0..=buffer_len - MIN_EOCD_SIZE as usize).rev() {
        if buffer[i] == 0x50
            && buffer[i + 1] == 0x4b
            && buffer[i + 2] == 0x05
            && buffer[i + 3] == 0x06
        {
            // Found candidate signature
            let comment_len = u16::from_le_bytes([buffer[i + 20], buffer[i + 21]]) as usize;

            if buffer_len - i == 22 + comment_len {
                // Valid EOCD found
                let eocd_record = buffer[i..].to_vec();

                // Extract CD offset and size from EOCD
                // Offset 12: Size of central directory (4 bytes)
                // Offset 16: Offset of start of central directory (4 bytes)
                let cd_size = u32::from_le_bytes([
                    buffer[i + 12],
                    buffer[i + 13],
                    buffer[i + 14],
                    buffer[i + 15],
                ]) as u64;
                let cd_offset = u32::from_le_bytes([
                    buffer[i + 16],
                    buffer[i + 17],
                    buffer[i + 18],
                    buffer[i + 19],
                ]) as u64;

                return Ok(ZipInfo {
                    cd_offset,
                    cd_size,
                    eocd_record,
                });
            }
        }
    }

    Err(anyhow!("End of Central Directory record not found"))
}
