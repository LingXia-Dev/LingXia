use crate::error::PlatformError;
use crate::traits::media_runtime::{CompressImageRequest, ImageInfo};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

/// Desktop unified implementation for get_image_info using imagesize crate.
/// Used by macOS, Windows, and other desktop platforms.
pub fn get_image_info_desktop(uri: &str) -> Result<ImageInfo, PlatformError> {
    let path = normalize_uri_to_path(uri)?;

    let file = File::open(&path).map_err(|e| {
        PlatformError::Platform(format!("Failed to open image file {}: {}", path, e))
    })?;

    let reader = BufReader::new(file);

    let size = imagesize::reader_size(reader)
        .map_err(|e| PlatformError::Platform(format!("Failed to read image dimensions: {}", e)))?;

    let mime_type = infer_mime_type(&path);

    Ok(ImageInfo {
        width: size.width as u32,
        height: size.height as u32,
        mime_type: Some(mime_type),
    })
}

fn normalize_uri_to_path(uri: &str) -> Result<String, PlatformError> {
    let trimmed = uri.trim();

    if trimmed.is_empty() {
        return Err(PlatformError::Platform("URI is empty".to_string()));
    }

    // Strip file:// prefix if present
    let path = if let Some(stripped) = trimmed.strip_prefix("file://") {
        stripped
    } else {
        trimmed
    };

    // Reject non-file URIs
    if path.contains("://") {
        return Err(PlatformError::Platform(format!(
            "Unsupported URI scheme: {}",
            path
        )));
    }

    Ok(path.to_string())
}

fn infer_mime_type(path: &str) -> String {
    let lower = path.to_ascii_lowercase();

    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".bmp") {
        "image/bmp"
    } else if lower.ends_with(".svg") {
        "image/svg+xml"
    } else if lower.ends_with(".ico") {
        "image/x-icon"
    } else if lower.ends_with(".tiff") || lower.ends_with(".tif") {
        "image/tiff"
    } else if lower.ends_with(".heic") || lower.ends_with(".heif") {
        "image/heic"
    } else {
        "image/jpeg" // default fallback
    }
    .to_string()
}

/// Desktop unified implementation for compress_image using image crate.
/// Supports resizing and quality compression.
pub fn compress_image_desktop(request: &CompressImageRequest) -> Result<PathBuf, PlatformError> {
    use image::ImageReader;
    use image::codecs::jpeg::JpegEncoder;

    // Normalize source URI to path
    let source_path = normalize_uri_to_path(&request.source_uri)?;

    // Open and decode image
    let img = ImageReader::open(&source_path)
        .map_err(|e| PlatformError::Platform(format!("Failed to open image: {}", e)))?
        .decode()
        .map_err(|e| PlatformError::Platform(format!("Failed to decode image: {}", e)))?;

    // Resize if needed
    let processed_img = if request.max_width.is_some() || request.max_height.is_some() {
        let (orig_width, orig_height) = (img.width(), img.height());
        let (target_width, target_height) = calculate_resize_dimensions(
            orig_width,
            orig_height,
            request.max_width,
            request.max_height,
        );

        if target_width != orig_width || target_height != orig_height {
            img.resize(
                target_width,
                target_height,
                image::imageops::FilterType::Lanczos3,
            )
        } else {
            img
        }
    } else {
        img
    };

    // Prepare output directory
    if let Some(parent) = request.output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            PlatformError::Platform(format!("Failed to create output directory: {}", e))
        })?;
    }

    // Save as JPEG with specified quality
    let output_file = File::create(&request.output_path)
        .map_err(|e| PlatformError::Platform(format!("Failed to create output file: {}", e)))?;

    let quality = request.quality.clamp(1, 100) as u8;
    let mut encoder = JpegEncoder::new_with_quality(output_file, quality);

    encoder
        .encode(
            processed_img.as_bytes(),
            processed_img.width(),
            processed_img.height(),
            processed_img.color().into(),
        )
        .map_err(|e| PlatformError::Platform(format!("Failed to encode JPEG: {}", e)))?;

    Ok(request.output_path.clone())
}

/// Calculate target dimensions based on max_width and max_height constraints.
/// Preserves aspect ratio.
fn calculate_resize_dimensions(
    orig_width: u32,
    orig_height: u32,
    max_width: Option<u32>,
    max_height: Option<u32>,
) -> (u32, u32) {
    match (max_width, max_height) {
        (Some(w), Some(h)) => {
            // Both constraints specified - fit within the box
            let width_ratio = w as f64 / orig_width as f64;
            let height_ratio = h as f64 / orig_height as f64;
            let ratio = width_ratio.min(height_ratio);

            if ratio < 1.0 {
                (
                    (orig_width as f64 * ratio) as u32,
                    (orig_height as f64 * ratio) as u32,
                )
            } else {
                (orig_width, orig_height)
            }
        }
        (Some(w), None) => {
            // Only width constraint
            if w < orig_width {
                let ratio = w as f64 / orig_width as f64;
                (w, (orig_height as f64 * ratio) as u32)
            } else {
                (orig_width, orig_height)
            }
        }
        (None, Some(h)) => {
            // Only height constraint
            if h < orig_height {
                let ratio = h as f64 / orig_height as f64;
                ((orig_width as f64 * ratio) as u32, h)
            } else {
                (orig_width, orig_height)
            }
        }
        (None, None) => {
            // No resize
            (orig_width, orig_height)
        }
    }
}
