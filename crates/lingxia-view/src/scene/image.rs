//! Worker-side bitmap decoding and shared CPU/GPU image geometry. No resource IO.
use ::image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};
use std::{io::Cursor, sync::Arc};
use vello_cpu::Pixmap;

pub const MAX_DIMENSION: u32 = 4096;
pub const MAX_PIXELS: u64 = 4 * 1024 * 1024;
const MAX_SOURCE_PIXELS: u64 = 12 * 1024 * 1024;

/// A decode size: the painted box in device pixels, each side rounded up to a
/// half-octave step from 64 to [`MAX_DIMENSION`], so small layout changes reuse a decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize)]
pub struct Bucket {
    pub width: u32,
    pub height: u32,
}

impl Bucket {
    /// The source at its own size, within [`MAX_PIXELS`].
    pub const FULL: Self = Self {
        width: u32::MAX,
        height: u32::MAX,
    };

    pub fn of(width: f32, height: f32) -> Self {
        Self {
            width: step(width),
            height: step(height),
        }
    }
    pub fn covers(self, other: Self) -> bool {
        self.width >= other.width && self.height >= other.height
    }
    pub fn union(self, other: Self) -> Self {
        Self {
            width: self.width.max(other.width),
            height: self.height.max(other.height),
        }
    }
}

fn step(extent: f32) -> u32 {
    let mut step = 64.0_f64;
    while step < f64::from(extent) && step < f64::from(MAX_DIMENSION) {
        step *= std::f64::consts::SQRT_2;
    }
    (step.round() as u32).min(MAX_DIMENSION)
}

/// Decoded pixels are immutable and premultiplied, shared by reference frames.
#[derive(Clone, Debug)]
pub struct Bitmap {
    pixels: Arc<Pixmap>,
    /// Decoded size over source size.
    scale: f64,
    /// Oriented source size.
    source: [u32; 2],
    /// No larger decode exists: the source or the pixel budget bounded this one.
    best: bool,
}

impl PartialEq for Bitmap {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.pixels, &other.pixels)
    }
}
impl Eq for Bitmap {}

impl Bitmap {
    pub(crate) fn from_pixels(pixels: Pixmap) -> Self {
        let source = [u32::from(pixels.width()), u32::from(pixels.height())];
        Self {
            pixels: Arc::new(pixels),
            scale: 1.0,
            source,
            best: true,
        }
    }
    /// The host supplies authorized bytes; call off the UI thread.
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        Self::decode_to(bytes, Bucket::FULL)
    }
    /// Decode no larger than a `bucket` box needs to draw without upsampling.
    pub fn decode_to(bytes: &[u8], bucket: Bucket) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > crate::protocol::limits::IMAGE_BYTES {
            return Err("UI_IMAGE: encoded size exceeds limit or is empty".into());
        }
        let format = ::image::guess_format(bytes).map_err(image_error)?;
        // Feature unification must not silently broaden the accepted formats.
        // GIF decodes its first frame; animation is not supported.
        if !matches!(
            format,
            ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP | ImageFormat::Gif
        ) {
            return Err("UI_IMAGE: only PNG, JPEG, WebP and GIF are supported".into());
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
        let (width, height) = (f64::from(decoded.width()), f64::from(decoded.height()));
        // Keep UHD sources useful without increasing the retained texture budget.
        let budget = (MAX_PIXELS as f64 / (width * height)).sqrt();
        let fit = need(bucket, [width, height]);
        let scale = budget.min(fit).min(1.0);
        let source = [decoded.width(), decoded.height()];
        if scale < 1.0 {
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
            scale,
            source,
            // Pixels bounded by the source or the budget serve every larger box too.
            best: fit >= budget.min(1.0),
        })
    }

    pub fn pixels(&self) -> &Arc<Pixmap> {
        &self.pixels
    }
    /// Whether these pixels draw any box in `bucket` without upsampling.
    pub fn serves(&self, bucket: Bucket) -> bool {
        self.best || self.scale >= need(bucket, self.source.map(f64::from))
    }
    /// Whether `other` serves every box these pixels serve.
    pub(crate) fn superseded_by(&self, other: &Self) -> bool {
        other.best || (!self.best && other.scale >= self.scale)
    }
    pub fn byte_len(&self) -> usize {
        usize::from(self.pixels.width()) * usize::from(self.pixels.height()) * 4
    }
}

/// The source scale a `bucket` box needs under any fit: cover's, the larger ratio.
fn need(bucket: Bucket, [width, height]: [f64; 2]) -> f64 {
    if bucket == Bucket::FULL {
        return f64::INFINITY;
    }
    (f64::from(bucket.width) / width).max(f64::from(bucket.height) / height)
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

/// Image rectangle aligned at `position` box fractions; the caller clips to the node bounds.
pub fn placement(
    size: [u16; 2],
    bounds: [f32; 4],
    fit: Fit,
    position: [f32; 2],
) -> Result<[f32; 4], String> {
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
        (x + (width - w) * f64::from(position[0])) as f32,
        (y + (height - h) * f64::from(position[1])) as f32,
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

    fn center(bitmap: &Bitmap) -> [u8; 4] {
        let pixmap = &bitmap.pixels;
        let (w, h) = (pixmap.width() as usize, pixmap.height() as usize);
        let at = ((h / 2) * w + w / 2) * 4;
        pixmap.data_as_u8_slice()[at..at + 4].try_into().unwrap()
    }

    #[test]
    fn webp_lossy_and_lossless_and_the_first_gif_frame_decode() {
        let lossy = Bitmap::decode(include_bytes!("tests/images/red-lossy.webp")).unwrap();
        assert_eq!((lossy.pixels.width(), lossy.pixels.height()), (16, 12));
        let [r, g, b, a] = center(&lossy);
        assert!(
            r > 230 && g < 30 && b < 30 && a == 255,
            "lossy red: {r},{g},{b},{a}"
        );

        let mut lossless = Vec::new();
        ::image::codecs::webp::WebPEncoder::new_lossless(&mut lossless)
            .write_image(
                &[0, 0, 255, 128].repeat(6),
                3,
                2,
                ::image::ExtendedColorType::Rgba8,
            )
            .unwrap();
        let decoded = Bitmap::decode(&lossless).unwrap();
        assert_eq!(center(&decoded), [0, 0, 128, 128], "premultiplied like PNG");

        let gif = Bitmap::decode(include_bytes!("tests/images/red-blue.gif")).unwrap();
        let [r, _, b, _] = center(&gif);
        assert!(r > 230 && b < 30, "the first frame is red: {r},{b}");
    }

    #[test]
    fn webp_and_gif_decode_to_the_painted_size() {
        let mut lossless = Vec::new();
        ::image::codecs::webp::WebPEncoder::new_lossless(&mut lossless)
            .write_image(
                &[200; 512 * 512 * 4],
                512,
                512,
                ::image::ExtendedColorType::Rgba8,
            )
            .unwrap();
        let small = Bitmap::decode_to(&lossless, Bucket::of(64.0, 64.0)).unwrap();
        assert!(small.pixels.width() < 512 && small.pixels.width() >= 64);
        let mut gif = Vec::new();
        {
            let mut encoder = ::image::codecs::gif::GifEncoder::new(&mut gif);
            encoder
                .encode(
                    &[90; 512 * 512 * 4],
                    512,
                    512,
                    ::image::ExtendedColorType::Rgba8,
                )
                .unwrap();
        }
        let small = Bitmap::decode_to(&gif, Bucket::of(64.0, 64.0)).unwrap();
        assert!(small.pixels.width() < 512 && small.pixels.width() >= 64);
    }

    #[test]
    fn malformed_unsupported_and_oversized_images_fail() {
        for bytes in [&[][..], b"not an image", b"GIF89a"] {
            assert!(Bitmap::decode(bytes).is_err());
        }
        assert!(Bitmap::decode(&vec![0; crate::protocol::limits::IMAGE_BYTES + 1]).is_err());
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

    const C: [f32; 2] = [0.5, 0.5];

    #[test]
    fn fit_centers_without_distorting_and_rejects_invalid_geometry() {
        let bounds = [10., 20., 100., 100.];
        assert_eq!(
            placement([200, 100], bounds, Fit::Contain, C).unwrap(),
            [10., 45., 100., 50.]
        );
        assert_eq!(
            placement([200, 100], bounds, Fit::Cover, C).unwrap(),
            [-40., 20., 200., 100.]
        );
        assert_eq!(placement([200, 100], bounds, Fit::Fill, C).unwrap(), bounds);
        assert_eq!(
            placement([100, 200], bounds, Fit::Cover, C).unwrap(),
            [10., -30., 100., 200.]
        );
        assert!(placement([0, 1], bounds, Fit::Cover, C).is_err());
        assert!(placement([1, 1], [0., 0., f32::NAN, 1.], Fit::Cover, C).is_err());
        assert!(placement([1, 1], [0., 0., -1., 1.], Fit::Cover, C).is_err());
        assert!(placement([1, u16::MAX], [0., 0., f32::MAX, 1.], Fit::Cover, C).is_err());
    }

    #[test]
    fn position_aligns_the_overflow_or_the_slack() {
        let bounds = [10., 20., 100., 100.];
        assert_eq!(
            placement([200, 100], bounds, Fit::Cover, [0., 0.]).unwrap(),
            [10., 20., 200., 100.]
        );
        assert_eq!(
            placement([200, 100], bounds, Fit::Cover, [1., 0.]).unwrap(),
            [-90., 20., 200., 100.]
        );
        assert_eq!(
            placement([200, 100], bounds, Fit::Contain, [0.5, 1.]).unwrap(),
            [10., 70., 100., 50.]
        );
        assert_eq!(
            placement([200, 100], bounds, Fit::Fill, [0., 1.]).unwrap(),
            bounds
        );
    }
}
