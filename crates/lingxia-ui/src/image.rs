//! Worker-side bitmap decoding and shared CPU/GPU image geometry. No resource IO.
use ::image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};
use std::{io::Cursor, sync::Arc};
use vello_cpu::Pixmap;

pub const MAX_ENCODED_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_DIMENSION: u32 = 4096;
pub const MAX_PIXELS: u64 = 4 * 1024 * 1024;
const MAX_SOURCE_PIXELS: u64 = 12 * 1024 * 1024;

/// Decoded pixels are immutable and premultiplied, shared by reference frames.
#[derive(Clone, Debug)]
pub struct Bitmap {
    pixels: Arc<Pixmap>,
}

impl PartialEq for Bitmap {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.pixels, &other.pixels)
    }
}
impl Eq for Bitmap {}

impl Bitmap {
    pub(crate) fn from_pixels(pixels: Pixmap) -> Self {
        Self {
            pixels: Arc::new(pixels),
        }
    }
    /// The host supplies authorized bytes; call off the UI thread.
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > MAX_ENCODED_BYTES {
            return Err("UI_IMAGE: encoded size exceeds limit or is empty".into());
        }
        let format = ::image::guess_format(bytes).map_err(image_error)?;
        // Feature unification must not silently broaden the accepted formats.
        if !matches!(format, ImageFormat::Png | ImageFormat::Jpeg) {
            return Err("UI_IMAGE: only PNG and JPEG are supported".into());
        }
        let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
        let mut limits = Limits::default();
        limits.max_image_width = Some(MAX_DIMENSION);
        limits.max_image_height = Some(MAX_DIMENSION);
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        let mut decoder = reader.into_decoder().map_err(image_error)?;
        let (width, height) = decoder.dimensions();
        if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_SOURCE_PIXELS {
            return Err("UI_IMAGE: decoded dimensions exceed limit or are empty".into());
        }
        let orientation = decoder.orientation().map_err(image_error)?;
        let mut decoded = DynamicImage::from_decoder(decoder).map_err(image_error)?;
        decoded.apply_orientation(orientation);
        // Keep UHD sources useful without increasing the retained texture budget.
        let pixels = u64::from(decoded.width()) * u64::from(decoded.height());
        if pixels > MAX_PIXELS {
            let scale = (MAX_PIXELS as f64 / pixels as f64).sqrt();
            decoded = decoded.resize_exact(
                ((decoded.width() as f64 * scale).floor() as u32).max(1),
                ((decoded.height() as f64 * scale).floor() as u32).max(1),
                ::image::imageops::FilterType::Triangle,
            );
        }
        let rgba = decoded.into_rgba8();
        let mut pixels = Pixmap::new(rgba.width() as u16, rgba.height() as u16);
        for (source, target) in rgba
            .as_raw()
            .chunks_exact(4)
            .zip(pixels.data_as_u8_slice_mut().chunks_exact_mut(4))
        {
            for channel in 0..3 {
                target[channel] =
                    ((u16::from(source[channel]) * u16::from(source[3]) + 127) / 255) as u8;
            }
            target[3] = source[3];
        }
        Ok(Self {
            pixels: Arc::new(pixels),
        })
    }

    pub fn pixels(&self) -> &Arc<Pixmap> {
        &self.pixels
    }
    pub fn byte_len(&self) -> usize {
        usize::from(self.pixels.width()) * usize::from(self.pixels.height()) * 4
    }
}

fn image_error(error: impl std::fmt::Display) -> String {
    format!("UI_IMAGE: {error}")
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fit {
    #[default]
    Cover,
    Contain,
    Fill,
}

/// Centered image rectangle; the caller clips cover images to the node bounds.
pub fn placement(size: [u16; 2], bounds: [f32; 4], fit: Fit) -> Result<[f32; 4], String> {
    if size.contains(&0)
        || !bounds.iter().all(|value| value.is_finite())
        || bounds[2] < 0.0
        || bounds[3] < 0.0
    {
        return Err("UI_IMAGE: invalid image geometry".into());
    }
    let [x, y, width, height] = bounds.map(f64::from);
    let [iw, ih] = size.map(f64::from);
    let (w, h) = match fit {
        Fit::Fill => (width, height),
        _ => {
            let scale = if fit == Fit::Cover {
                (width / iw).max(height / ih)
            } else {
                (width / iw).min(height / ih)
            };
            (iw * scale, ih * scale)
        }
    };
    let placed = [
        (x + (width - w) / 2.0) as f32,
        (y + (height - h) / 2.0) as f32,
        w as f32,
        h as f32,
    ];
    if !placed.iter().all(|value| value.is_finite()) {
        return Err("UI_IMAGE: image geometry overflow".into());
    }
    Ok(placed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::image::{
        ImageEncoder,
        codecs::{jpeg::JpegEncoder, png::PngEncoder},
    };

    fn png(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        PngEncoder::new(&mut bytes)
            .write_image(pixels, width, height, ::image::ExtendedColorType::Rgba8)
            .unwrap();
        bytes
    }

    #[test]
    fn png_alpha_is_premultiplied_and_shared_without_copying() {
        let bitmap = Bitmap::decode(&png(2, 1, &[200, 100, 50, 128, 255, 100, 10, 0])).unwrap();
        assert_eq!(
            bitmap.pixels.data_as_u8_slice(),
            &[100, 50, 25, 128, 0, 0, 0, 0]
        );
        assert_eq!(bitmap.byte_len(), 8);
        assert!(Arc::ptr_eq(bitmap.pixels(), bitmap.clone().pixels()));
    }

    #[test]
    fn jpeg_orientation_is_applied_before_layout() {
        let mut jpeg = Vec::new();
        JpegEncoder::new(&mut jpeg)
            .encode(
                &[200, 30, 10, 10, 30, 200],
                1,
                2,
                ::image::ExtendedColorType::Rgb8,
            )
            .unwrap();
        // EXIF orientation 6: rotate a portrait image clockwise.
        let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
        let mut tagged = jpeg[..2].to_vec();
        tagged.extend([0xff, 0xe1]);
        tagged.extend(((exif.len() + 2) as u16).to_be_bytes());
        tagged.extend(exif);
        tagged.extend(&jpeg[2..]);
        let bitmap = Bitmap::decode(&tagged).unwrap();
        assert_eq!((bitmap.pixels.width(), bitmap.pixels.height()), (2, 1));
        assert!(
            bitmap
                .pixels
                .data_as_u8_slice()
                .chunks_exact(4)
                .all(|p| p[3] == 255)
        );
    }

    #[test]
    fn malformed_unsupported_and_oversized_images_fail() {
        for bytes in [&[][..], b"not an image", b"GIF89a"] {
            assert!(Bitmap::decode(bytes).is_err());
        }
        assert!(Bitmap::decode(&vec![0; MAX_ENCODED_BYTES + 1]).is_err());
        assert!(
            Bitmap::decode(&png(
                MAX_DIMENSION + 1,
                1,
                &vec![0; (MAX_DIMENSION as usize + 1) * 4]
            ))
            .is_err()
        );
        let bytes = png(2, 1, &[0; 8]);
        assert!(Bitmap::decode(&bytes[..bytes.len() / 2]).is_err());
        let too_many_pixels = png(4096, 3073, &vec![0; 4096 * 3073 * 4]);
        assert!(
            Bitmap::decode(&too_many_pixels)
                .unwrap_err()
                .contains("dimensions")
        );
    }

    #[test]
    fn uhd_background_is_downsampled_to_the_retained_pixel_budget() {
        let mut jpeg = Vec::new();
        JpegEncoder::new(&mut jpeg)
            .encode(
                &vec![90; 3840 * 2160 * 3],
                3840,
                2160,
                ::image::ExtendedColorType::Rgb8,
            )
            .unwrap();
        let bitmap = Bitmap::decode(&jpeg).unwrap();
        let (width, height) = (bitmap.pixels.width(), bitmap.pixels.height());
        assert!(u64::from(width) * u64::from(height) <= MAX_PIXELS);
        assert!(width > 2000 && height > 1000);
        assert!((f64::from(width) / f64::from(height) - 16.0 / 9.0).abs() < 0.002);
        assert!(
            bitmap
                .pixels
                .data_as_u8_slice()
                .chunks_exact(4)
                .all(|pixel| pixel[3] == 255 && (i16::from(pixel[0]) - 90).abs() < 3)
        );
    }

    #[test]
    fn fit_centers_without_distorting_and_rejects_invalid_geometry() {
        let bounds = [10., 20., 100., 100.];
        assert_eq!(
            placement([200, 100], bounds, Fit::Contain).unwrap(),
            [10., 45., 100., 50.]
        );
        assert_eq!(
            placement([200, 100], bounds, Fit::Cover).unwrap(),
            [-40., 20., 200., 100.]
        );
        assert_eq!(placement([200, 100], bounds, Fit::Fill).unwrap(), bounds);
        assert_eq!(
            placement([100, 200], bounds, Fit::Cover).unwrap(),
            [10., -30., 100., 200.]
        );
        assert!(placement([0, 1], bounds, Fit::Cover).is_err());
        assert!(placement([1, 1], [0., 0., f32::NAN, 1.], Fit::Cover).is_err());
        assert!(placement([1, 1], [0., 0., -1., 1.], Fit::Cover).is_err());
        assert!(placement([1, u16::MAX], [0., 0., f32::MAX, 1.], Fit::Cover).is_err());
    }
}
