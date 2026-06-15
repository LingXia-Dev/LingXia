//! Video metadata and thumbnails through the Media Foundation source
//! reader: the system codecs decode, the reader's processing chain
//! converts to RGB32, and the `image` crate scales and encodes the
//! thumbnail.

use std::path::Path;

use windows::Win32::Media::MediaFoundation::{
    IMFSourceReader, MF_API_VERSION, MF_MT_AVG_BITRATE, MF_MT_DEFAULT_STRIDE, MF_MT_FRAME_RATE,
    MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MF_MT_VIDEO_ROTATION, MF_PD_DURATION,
    MF_SDK_VERSION, MF_SOURCE_READER_DISABLE_DXVA,
    MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, MF_SOURCE_READER_FIRST_VIDEO_STREAM,
    MF_SOURCE_READER_MEDIASOURCE, MF_SOURCE_READERF_ENDOFSTREAM, MFCreateAttributes,
    MFCreateMediaType, MFCreateSourceReaderFromURL, MFMediaType_Video, MFStartup,
    MFVideoFormat_RGB32,
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
use windows::Win32::System::Variant::{VT_I8, VT_UI8};
use windows::core::{GUID, PCWSTR};

use crate::error::PlatformError;
use crate::traits::media_runtime::{ExtractVideoThumbnailRequest, VideoInfo, VideoThumbnail};

/// `MFSTARTUP_NOSOCKET` — the lite startup without the RTSP stack.
const MFSTARTUP_LITE: u32 = 0x1;

pub(super) fn ensure_media_foundation() {
    use std::sync::OnceLock;
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| unsafe {
        // The calling thread may or may not have COM yet; both outcomes
        // are fine for the source reader.
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        if let Err(err) = MFStartup((MF_SDK_VERSION << 16) | MF_API_VERSION, MFSTARTUP_LITE) {
            log::warn!("MFStartup failed: {err}");
        }
    });
}

/// Opens a source reader; `with_processing` inserts the video processor
/// so the output can be converted to RGB32 (thumbnails).
fn open_reader(path: &Path, with_processing: bool) -> Result<IMFSourceReader, PlatformError> {
    ensure_media_foundation();
    let wide: Vec<u16> = path
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let attributes = if with_processing {
        let mut attributes = None;
        unsafe {
            MFCreateAttributes(&mut attributes, 2)
                .map_err(|err| PlatformError::Platform(format!("attributes: {err}")))?;
        }
        if let Some(attributes) = attributes.as_ref() {
            unsafe {
                let _ = attributes.SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1);
                // Force software decode so frames land in system memory. With
                // DXVA the decoder hands back D3D surfaces, and a plain
                // `IMFMediaBuffer::Lock` of those yields an all-zero (black)
                // buffer — which is exactly what the thumbnail grabber reads.
                let _ = attributes.SetUINT32(&MF_SOURCE_READER_DISABLE_DXVA, 1);
            }
        }
        attributes
    } else {
        None
    };
    unsafe { MFCreateSourceReaderFromURL(PCWSTR(wide.as_ptr()), attributes.as_ref()) }
        .map_err(|err| PlatformError::Platform(format!("failed to open {}: {err}", path.display())))
}

fn hundred_ns(variant: &PROPVARIANT) -> u64 {
    unsafe {
        let inner = &*variant.Anonymous.Anonymous;
        if inner.vt == VT_UI8 || inner.vt == VT_I8 {
            inner.Anonymous.uhVal as u64
        } else {
            0
        }
    }
}

fn mime_for(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    let mime = match ext.as_str() {
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "wmv" => "video/x-ms-wmv",
        _ => return None,
    };
    Some(mime.to_string())
}

pub(super) fn read_video_info(path: &Path) -> Result<VideoInfo, PlatformError> {
    let reader = open_reader(path, false)?;
    let duration_ms = unsafe {
        reader.GetPresentationAttribute(MF_SOURCE_READER_MEDIASOURCE.0 as u32, &MF_PD_DURATION)
    }
    .map(|value| hundred_ns(&value) / 10_000)
    .unwrap_or(0);

    let native =
        unsafe { reader.GetNativeMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, 0) }
            .map_err(|err| PlatformError::Platform(format!("no video stream: {err}")))?;

    let frame = unsafe { native.GetUINT64(&MF_MT_FRAME_SIZE) }.unwrap_or(0);
    let (width, height) = ((frame >> 32) as u32, frame as u32);
    let fps = unsafe { native.GetUINT64(&MF_MT_FRAME_RATE) }
        .ok()
        .and_then(|rate| {
            let (numerator, denominator) = ((rate >> 32) as u32, rate as u32);
            (denominator > 0).then(|| numerator as f32 / denominator as f32)
        });
    let bitrate = unsafe { native.GetUINT32(&MF_MT_AVG_BITRATE) }
        .ok()
        .map(u64::from);
    let rotation = unsafe { native.GetUINT32(&MF_MT_VIDEO_ROTATION) }
        .ok()
        .map(|rotation| rotation as u16);

    Ok(VideoInfo {
        width,
        height,
        duration_ms,
        rotation,
        bitrate,
        fps,
        mime_type: mime_for(path),
    })
}

pub(super) fn extract_thumbnail(
    request: &ExtractVideoThumbnailRequest,
    source: &Path,
) -> Result<VideoThumbnail, PlatformError> {
    let reader = open_reader(source, true)?;
    let stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;

    // Have the reader's chain decode and convert to RGB32.
    unsafe {
        let rgb = MFCreateMediaType()
            .map_err(|err| PlatformError::Platform(format!("media type: {err}")))?;
        rgb.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .and_then(|_| rgb.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32))
            .and_then(|_| reader.SetCurrentMediaType(stream, None, &rgb))
            .map_err(|err| {
                PlatformError::Platform(format!("RGB32 conversion unavailable: {err}"))
            })?;
    }

    let requested_100ns = request
        .time_ms
        .filter(|time| *time > 0)
        .map(|time_ms| time_ms as i64 * 10_000);
    if let Some(position_100ns) = requested_100ns {
        let mut position = PROPVARIANT::default();
        unsafe {
            let inner = &mut *position.Anonymous.Anonymous;
            inner.vt = VT_I8;
            inner.Anonymous.hVal = position_100ns;
            let _ = reader.SetCurrentPosition(&GUID::zeroed(), &position);
        }
    }

    // The seek lands on the keyframe before the position, so decode
    // forward until the sample covering the requested time (keeping the
    // last decoded frame as the fallback when the request is past EOS).
    let sample = unsafe {
        let mut sample = None;
        // Bounds the keyframe-to-target decode walk (~30s at 60fps).
        for _ in 0..2048 {
            let mut flags = 0u32;
            let mut current = None;
            reader
                .ReadSample(stream, 0, None, Some(&mut flags), None, Some(&mut current))
                .map_err(|err| PlatformError::Platform(format!("decode failed: {err}")))?;
            if let Some(current) = current {
                let reached = match requested_100ns {
                    Some(target) => {
                        let time = current.GetSampleTime().unwrap_or(i64::MAX);
                        let duration = current.GetSampleDuration().unwrap_or(0).max(0);
                        time + duration >= target
                    }
                    None => true,
                };
                sample = Some(current);
                if reached {
                    break;
                }
            }
            if flags & (MF_SOURCE_READERF_ENDOFSTREAM.0 as u32) != 0 {
                break;
            }
        }
        sample.ok_or_else(|| PlatformError::Platform("no decodable video frame".to_string()))?
    };

    // Actual (decoder-aligned) frame geometry.
    let current_type = unsafe { reader.GetCurrentMediaType(stream) }
        .map_err(|err| PlatformError::Platform(format!("media type: {err}")))?;
    let frame = unsafe { current_type.GetUINT64(&MF_MT_FRAME_SIZE) }
        .map_err(|err| PlatformError::Platform(format!("frame size: {err}")))?;
    let (width, height) = ((frame >> 32) as u32, frame as u32);
    if width == 0 || height == 0 {
        return Err(PlatformError::Platform("empty video frame".to_string()));
    }
    let stride = unsafe { current_type.GetUINT32(&MF_MT_DEFAULT_STRIDE) }
        .map(|stride| stride as i32)
        .unwrap_or((width * 4) as i32);

    // Copy out the BGRA pixels (bottom-up when the stride is negative).
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    unsafe {
        let buffer = sample
            .ConvertToContiguousBuffer()
            .map_err(|err| PlatformError::Platform(format!("frame buffer: {err}")))?;
        let mut data: *mut u8 = std::ptr::null_mut();
        let mut length = 0u32;
        buffer
            .Lock(&mut data, None, Some(&mut length))
            .map_err(|err| PlatformError::Platform(format!("frame lock: {err}")))?;
        let row_bytes = (width * 4) as usize;
        let abs_stride = stride.unsigned_abs() as usize;
        for row in 0..height as usize {
            let source_row = if stride < 0 {
                height as usize - 1 - row
            } else {
                row
            };
            let offset = source_row * abs_stride;
            if offset + row_bytes <= length as usize {
                std::ptr::copy_nonoverlapping(
                    data.add(offset),
                    pixels.as_mut_ptr().add(row * row_bytes),
                    row_bytes,
                );
            }
        }
        let _ = buffer.Unlock();
    }
    // BGRA -> RGBA.
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
        pixel[3] = 0xff;
    }

    let image = image::RgbaImage::from_raw(width, height, pixels)
        .ok_or_else(|| PlatformError::Platform("frame conversion failed".to_string()))?;
    let mut dynamic = image::DynamicImage::ImageRgba8(image);

    // Fit within the requested bounds, keeping the aspect ratio.
    let max_width = request.max_width.unwrap_or(width).max(1);
    let max_height = request.max_height.unwrap_or(height).max(1);
    if width > max_width || height > max_height {
        dynamic = dynamic.resize(max_width, max_height, image::imageops::FilterType::Triangle);
    }

    if let Some(parent) = request.output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            PlatformError::Platform(format!("failed to create {}: {err}", parent.display()))
        })?;
    }
    let is_png = request
        .output_path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("png"));
    if is_png {
        dynamic
            .to_rgba8()
            .save_with_format(&request.output_path, image::ImageFormat::Png)
            .map_err(|err| PlatformError::Platform(format!("thumbnail encode: {err}")))?;
    } else {
        let file = std::fs::File::create(&request.output_path).map_err(|err| {
            PlatformError::Platform(format!(
                "failed to create {}: {err}",
                request.output_path.display()
            ))
        })?;
        let mut writer = std::io::BufWriter::new(file);
        let quality = request.quality.clamp(1, 100);
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, quality);
        dynamic
            .to_rgb8()
            .write_with_encoder(encoder)
            .map_err(|err| PlatformError::Platform(format!("thumbnail encode: {err}")))?;
    }

    Ok(VideoThumbnail {
        path: request.output_path.clone(),
        width: dynamic.width(),
        height: dynamic.height(),
        mime_type: Some(if is_png { "image/png" } else { "image/jpeg" }.to_string()),
    })
}
