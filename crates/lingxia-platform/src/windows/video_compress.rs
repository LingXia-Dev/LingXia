//! Video compression through Media Foundation: the source reader decodes
//! (and scales) frames via its advanced video processing chain, and the
//! sink writer re-encodes them to H.264/AAC in an MP4 container. The whole
//! pipeline prefers hardware transforms when the system exposes them.
//!
//! When the transcode fails, or the re-encode does not actually shrink the
//! file, we fall back to copying the source — mirroring the AVFoundation
//! path on Apple so callers always get a usable output.

use std::fs;
use std::path::{Path, PathBuf};

use windows::Win32::Media::MediaFoundation::{
    IMFMediaType, IMFSample, IMFSinkWriter, IMFSourceReader,
    MF_MT_AAC_AUDIO_PROFILE_LEVEL_INDICATION, MF_MT_AAC_PAYLOAD_TYPE,
    MF_MT_AUDIO_AVG_BYTES_PER_SECOND, MF_MT_AUDIO_BITS_PER_SAMPLE, MF_MT_AUDIO_NUM_CHANNELS,
    MF_MT_AUDIO_SAMPLES_PER_SECOND, MF_MT_AVG_BITRATE, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE,
    MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_MPEG2_PROFILE, MF_MT_PIXEL_ASPECT_RATIO,
    MF_MT_SUBTYPE, MF_PD_DURATION, MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS,
    MF_SOURCE_READER_ALL_STREAMS, MF_SOURCE_READER_ANY_STREAM,
    MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, MF_SOURCE_READER_MEDIASOURCE,
    MF_SOURCE_READERF_ENDOFSTREAM, MFAudioFormat_AAC, MFAudioFormat_PCM, MFCreateAttributes,
    MFCreateMediaType, MFCreateSinkWriterFromURL, MFCreateSourceReaderFromURL, MFMediaType_Audio,
    MFMediaType_Video, MFVideoFormat_H264, MFVideoFormat_NV12, MFVideoInterlace_Progressive,
};
use windows::Win32::System::Variant::{VT_I8, VT_UI8};
use windows::core::PCWSTR;

use crate::error::PlatformError;
use crate::traits::media_runtime::{CompressVideoRequest, CompressedVideo, VideoCompressQuality};

/// Bits per encoded pixel for each preset; multiplied by `width * height * fps`
/// to derive a target average bitrate that scales with the output geometry.
const BPP_LOW: f64 = 0.08;
const BPP_MEDIUM: f64 = 0.10;
const BPP_HIGH: f64 = 0.14;
/// Default bits per pixel when neither a preset nor an explicit bitrate is set.
const BPP_DEFAULT: f64 = 0.10;

/// Longest-edge caps per preset, matching the intent of the Apple export presets
/// (`640x480` / `~540p` / `720p`).
const CAP_LOW: u32 = 640;
const CAP_MEDIUM: u32 = 960;
const CAP_HIGH: u32 = 1280;

/// Floor for any computed bitrate so tiny inputs still encode cleanly.
const MIN_BITRATE_BPS: u32 = 200_000;

fn pack(hi: u32, lo: u32) -> u64 {
    ((hi as u64) << 32) | lo as u64
}

fn unpack(value: u64) -> (u32, u32) {
    ((value >> 32) as u32, value as u32)
}

/// Round up to the nearest even number (H.264 frame dimensions must be even).
fn even(value: u32) -> u32 {
    value.saturating_add(1) & !1
}

fn wide_path(path: &Path) -> Vec<u16> {
    path.to_string_lossy()
        .trim_start_matches(r"\\?\")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect()
}

fn paths_refer_to_same_file(left: &Path, right: &Path) -> bool {
    let left = comparable_path(left);
    let right = comparable_path(right);
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}

fn comparable_path(path: &Path) -> PathBuf {
    if let Ok(path) = fs::canonicalize(path) {
        return path;
    }
    if let (Some(parent), Some(file_name)) = (path.parent(), path.file_name())
        && let Ok(parent) = fs::canonicalize(parent)
    {
        return parent.join(file_name);
    }
    path.to_path_buf()
}

/// The output geometry, bitrate, and (optional) frame-rate cap for a transcode.
struct EncodePlan {
    width: u32,
    height: u32,
    bitrate_bps: u32,
    /// `Some(fps)` when the source should be decimated to this rate.
    drop_to_fps: Option<u32>,
}

/// Scale `(w, h)` so its longest edge fits `cap`, preserving aspect; never upscales.
fn scale_to_cap(w: u32, h: u32, cap: u32) -> (u32, u32) {
    let longest = w.max(h);
    if longest == 0 || longest <= cap {
        return (even(w), even(h));
    }
    let scale = cap as f64 / longest as f64;
    (
        even((w as f64 * scale).round().max(2.0) as u32),
        even((h as f64 * scale).round().max(2.0) as u32),
    )
}

fn plan_encode(
    request: &CompressVideoRequest,
    src_w: u32,
    src_h: u32,
    src_fps: f64,
    src_bitrate: Option<u64>,
) -> EncodePlan {
    let src_fps = if src_fps > 0.0 { src_fps } else { 30.0 };

    let (width, height, bpp, explicit_bitrate) = match request.quality {
        Some(VideoCompressQuality::Low) => {
            let (w, h) = scale_to_cap(src_w, src_h, CAP_LOW);
            (w, h, BPP_LOW, None)
        }
        Some(VideoCompressQuality::Medium) => {
            let (w, h) = scale_to_cap(src_w, src_h, CAP_MEDIUM);
            (w, h, BPP_MEDIUM, None)
        }
        Some(VideoCompressQuality::High) => {
            let (w, h) = scale_to_cap(src_w, src_h, CAP_HIGH);
            (w, h, BPP_HIGH, None)
        }
        None => {
            // Explicit-parameter path: scale by ratio, honor an explicit bitrate.
            let ratio = request
                .resolution_ratio
                .filter(|r| *r > 0.0 && *r < 1.0)
                .map(|r| r as f64)
                .unwrap_or(1.0);
            let w = even((src_w as f64 * ratio).round().max(2.0) as u32);
            let h = even((src_h as f64 * ratio).round().max(2.0) as u32);
            let explicit = request
                .bitrate_kbps
                .filter(|b| *b > 0)
                .map(|b| b.saturating_mul(1000));
            (w, h, BPP_DEFAULT, explicit)
        }
    };

    // Frame-rate decimation only applies to the explicit fps parameter; presets
    // keep the source rate, like the Apple presets do.
    let drop_to_fps = request
        .fps
        .filter(|fps| *fps > 0 && (*fps as f64) < src_fps - 0.01);
    let effective_fps = drop_to_fps.map(|f| f as f64).unwrap_or(src_fps);

    let bitrate_bps = match explicit_bitrate {
        Some(bitrate) => bitrate.max(MIN_BITRATE_BPS),
        None => {
            let modeled = (width as f64 * height as f64 * effective_fps * bpp) as u32;
            let mut target = modeled.max(MIN_BITRATE_BPS);
            // Never exceed the source bitrate in derived modes — guarantees a shrink.
            if let Some(src) = src_bitrate.filter(|b| *b > 0) {
                target = target.min(src as u32);
            }
            target.max(MIN_BITRATE_BPS)
        }
    };

    EncodePlan {
        width,
        height,
        bitrate_bps,
        drop_to_fps,
    }
}

fn create_reader(path: &Path) -> Result<IMFSourceReader, PlatformError> {
    super::video_info::ensure_media_foundation();
    let wide = wide_path(path);
    let mut attributes: Option<windows::Win32::Media::MediaFoundation::IMFAttributes> = None;
    unsafe {
        MFCreateAttributes(&mut attributes, 2)
            .map_err(|err| PlatformError::Platform(format!("reader attributes: {err}")))?;
    }
    if let Some(attributes) = attributes.as_ref() {
        unsafe {
            let _ = attributes.SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1);
            let _ = attributes.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1);
        }
    }
    unsafe { MFCreateSourceReaderFromURL(PCWSTR(wide.as_ptr()), attributes.as_ref()) }
        .map_err(|err| PlatformError::Platform(format!("open source {}: {err}", path.display())))
}

fn create_sink(path: &Path) -> Result<IMFSinkWriter, PlatformError> {
    let wide = wide_path(path);
    let mut attributes: Option<windows::Win32::Media::MediaFoundation::IMFAttributes> = None;
    unsafe {
        MFCreateAttributes(&mut attributes, 1)
            .map_err(|err| PlatformError::Platform(format!("sink attributes: {err}")))?;
    }
    if let Some(attributes) = attributes.as_ref() {
        unsafe {
            let _ = attributes.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1);
        }
    }
    unsafe { MFCreateSinkWriterFromURL(PCWSTR(wide.as_ptr()), None, attributes.as_ref()) }
        .map_err(|err| PlatformError::Platform(format!("create sink {}: {err}", path.display())))
}

fn duration_ms(reader: &IMFSourceReader) -> u64 {
    let variant = unsafe {
        reader.GetPresentationAttribute(MF_SOURCE_READER_MEDIASOURCE.0 as u32, &MF_PD_DURATION)
    };
    match variant {
        Ok(variant) => unsafe {
            let inner = &*variant.Anonymous.Anonymous;
            if inner.vt == VT_UI8 || inner.vt == VT_I8 {
                (inner.Anonymous.uhVal as u64) / 10_000
            } else {
                0
            }
        },
        Err(_) => 0,
    }
}

/// First selected video and audio stream indices (numeric, 0-based), if present.
struct Streams {
    video: u32,
    audio: Option<u32>,
}

fn classify_streams(reader: &IMFSourceReader) -> Result<Streams, PlatformError> {
    unsafe {
        // Deselect everything, then re-enable just the streams we transcode.
        let _ = reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false);
    }
    let mut video: Option<u32> = None;
    let mut audio: Option<u32> = None;
    let mut index = 0u32;
    loop {
        let native = unsafe { reader.GetNativeMediaType(index, 0) };
        let Ok(native) = native else { break };
        let major = unsafe { native.GetGUID(&MF_MT_MAJOR_TYPE) };
        if let Ok(major) = major {
            if major == MFMediaType_Video && video.is_none() {
                video = Some(index);
                unsafe {
                    let _ = reader.SetStreamSelection(index, true);
                }
            } else if major == MFMediaType_Audio && audio.is_none() {
                audio = Some(index);
                unsafe {
                    let _ = reader.SetStreamSelection(index, true);
                }
            }
        }
        index += 1;
    }
    let video = video.ok_or_else(|| PlatformError::Platform("no video stream".to_string()))?;
    Ok(Streams { video, audio })
}

/// Read source geometry / rate / bitrate from the native video media type.
fn native_video_params(
    reader: &IMFSourceReader,
    video_index: u32,
) -> Result<(u32, u32, f64, Option<u64>, (u32, u32)), PlatformError> {
    let native = unsafe { reader.GetNativeMediaType(video_index, 0) }
        .map_err(|err| PlatformError::Platform(format!("native video type: {err}")))?;
    let frame = unsafe { native.GetUINT64(&MF_MT_FRAME_SIZE) }.unwrap_or(0);
    let (w, h) = unpack(frame);
    let rate = unsafe { native.GetUINT64(&MF_MT_FRAME_RATE) }.unwrap_or(pack(30, 1));
    let (num, den) = unpack(rate);
    let fps = if den > 0 {
        num as f64 / den as f64
    } else {
        30.0
    };
    let bitrate = unsafe { native.GetUINT32(&MF_MT_AVG_BITRATE) }
        .ok()
        .map(u64::from)
        .filter(|b| *b > 0);
    Ok((w, h, fps, bitrate, (num.max(1), den.max(1))))
}

/// Set the reader's video output to NV12 at the planned size; fall back to the
/// native size when the processing chain refuses to scale. Returns the actual
/// output geometry.
fn configure_video_output(
    reader: &IMFSourceReader,
    video_index: u32,
    plan: &EncodePlan,
) -> Result<(u32, u32), PlatformError> {
    let scaled = unsafe {
        let media = MFCreateMediaType()
            .map_err(|err| PlatformError::Platform(format!("nv12 type: {err}")))?;
        media.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).ok();
        media.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12).ok();
        media
            .SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
            .ok();
        media
            .SetUINT64(&MF_MT_FRAME_SIZE, pack(plan.width, plan.height))
            .ok();
        reader
            .SetCurrentMediaType(video_index, None, &media)
            .is_ok()
    };

    if !scaled {
        // Scaling unavailable — decode at native size and only reduce bitrate.
        let media = unsafe {
            let media = MFCreateMediaType()
                .map_err(|err| PlatformError::Platform(format!("nv12 type: {err}")))?;
            media.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).ok();
            media.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12).ok();
            media
                .SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
                .ok();
            media
        };
        unsafe {
            reader
                .SetCurrentMediaType(video_index, None, &media)
                .map_err(|err| PlatformError::Platform(format!("set nv12 output: {err}")))?;
        }
    }

    let current = unsafe { reader.GetCurrentMediaType(video_index) }
        .map_err(|err| PlatformError::Platform(format!("current video type: {err}")))?;
    let frame =
        unsafe { current.GetUINT64(&MF_MT_FRAME_SIZE) }.unwrap_or(pack(plan.width, plan.height));
    let (w, h) = unpack(frame);
    Ok((even(w.max(2)), even(h.max(2))))
}

/// Configure the reader's audio output to PCM and return the resulting type,
/// so the sink writer can take it as input. `None` means audio is unavailable.
fn configure_audio_output(reader: &IMFSourceReader, audio_index: u32) -> Option<IMFMediaType> {
    let media = unsafe { MFCreateMediaType().ok()? };
    unsafe {
        media.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).ok()?;
        media.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM).ok()?;
        media.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16).ok()?;
        reader.SetCurrentMediaType(audio_index, None, &media).ok()?;
        reader.GetCurrentMediaType(audio_index).ok()
    }
}

fn build_h264_type(
    width: u32,
    height: u32,
    bitrate_bps: u32,
    frame_rate: (u32, u32),
) -> Result<IMFMediaType, PlatformError> {
    unsafe {
        let media = MFCreateMediaType()
            .map_err(|err| PlatformError::Platform(format!("h264 type: {err}")))?;
        media.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).ok();
        media.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264).ok();
        media.SetUINT32(&MF_MT_AVG_BITRATE, bitrate_bps).ok();
        media.SetUINT64(&MF_MT_FRAME_SIZE, pack(width, height)).ok();
        media
            .SetUINT64(&MF_MT_FRAME_RATE, pack(frame_rate.0, frame_rate.1))
            .ok();
        media
            .SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
            .ok();
        media.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1)).ok();
        // eAVEncH264VProfile_Main = 77 — broad playback compatibility.
        media.SetUINT32(&MF_MT_MPEG2_PROFILE, 77).ok();
        Ok(media)
    }
}

fn build_aac_type(pcm: &IMFMediaType) -> Result<IMFMediaType, PlatformError> {
    unsafe {
        let sample_rate = pcm
            .GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND)
            .unwrap_or(44100);
        let channels = pcm.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS).unwrap_or(2).max(1);
        // 128 kbps stereo, 96 kbps mono — both valid MS AAC encoder rates.
        let bytes_per_sec = if channels <= 1 { 12000 } else { 16000 };
        let media = MFCreateMediaType()
            .map_err(|err| PlatformError::Platform(format!("aac type: {err}")))?;
        media.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).ok();
        media.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC).ok();
        media.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16).ok();
        media
            .SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, sample_rate)
            .ok();
        media.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, channels).ok();
        media
            .SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, bytes_per_sec)
            .ok();
        media.SetUINT32(&MF_MT_AAC_PAYLOAD_TYPE, 0).ok();
        // 0x29 = AAC-LC profile/level indication.
        media
            .SetUINT32(&MF_MT_AAC_AUDIO_PROFILE_LEVEL_INDICATION, 0x29)
            .ok();
        Ok(media)
    }
}

/// Pump samples from reader to sink until every selected stream hits EOS.
fn pump_samples(
    reader: &IMFSourceReader,
    sink: &IMFSinkWriter,
    video_index: u32,
    video_sink: u32,
    audio_index: Option<u32>,
    audio_sink: Option<u32>,
    drop_interval_100ns: Option<i64>,
) -> Result<(), PlatformError> {
    let total_streams = 1 + audio_index.is_some() as u32;
    let mut ended = 0u32;
    let mut last_kept_video: Option<i64> = None;

    loop {
        let mut actual_index = 0u32;
        let mut flags = 0u32;
        let mut timestamp = 0i64;
        let mut sample: Option<IMFSample> = None;
        unsafe {
            reader
                .ReadSample(
                    MF_SOURCE_READER_ANY_STREAM.0 as u32,
                    0,
                    Some(&mut actual_index),
                    Some(&mut flags),
                    Some(&mut timestamp),
                    Some(&mut sample),
                )
                .map_err(|err| PlatformError::Platform(format!("read sample: {err}")))?;
        }

        if let Some(sample) = sample {
            if actual_index == video_index {
                let keep = match drop_interval_100ns {
                    Some(interval) => match last_kept_video {
                        Some(last) => timestamp - last >= interval,
                        None => true,
                    },
                    None => true,
                };
                if keep {
                    last_kept_video = Some(timestamp);
                    unsafe {
                        sink.WriteSample(video_sink, &sample).map_err(|err| {
                            PlatformError::Platform(format!("write video: {err}"))
                        })?;
                    }
                }
            } else if Some(actual_index) == audio_index {
                if let Some(audio_sink) = audio_sink {
                    unsafe {
                        // Audio drops are non-fatal: keep video flowing.
                        if let Err(err) = sink.WriteSample(audio_sink, &sample) {
                            log::warn!("write audio sample failed: {err}");
                        }
                    }
                }
            }
        }

        if flags & (MF_SOURCE_READERF_ENDOFSTREAM.0 as u32) != 0 {
            ended += 1;
            if ended >= total_streams {
                break;
            }
        }
    }
    Ok(())
}

fn transcode(
    request: &CompressVideoRequest,
    source: &Path,
) -> Result<CompressedVideo, PlatformError> {
    let reader = create_reader(source)?;
    let total_duration_ms = duration_ms(&reader);
    let streams = classify_streams(&reader)?;
    let (src_w, src_h, src_fps, src_bitrate, src_rate) =
        native_video_params(&reader, streams.video)?;

    let plan = plan_encode(request, src_w, src_h, src_fps, src_bitrate);

    let (out_w, out_h) = configure_video_output(&reader, streams.video, &plan)?;
    let video_input = unsafe { reader.GetCurrentMediaType(streams.video) }
        .map_err(|err| PlatformError::Platform(format!("video input type: {err}")))?;

    // Output frame rate: the decimated rate when dropping, else the source rate.
    let out_rate = match plan.drop_to_fps {
        Some(fps) => (fps, 1),
        None => src_rate,
    };
    let drop_interval = plan
        .drop_to_fps
        .map(|fps| 10_000_000i64 / fps.max(1) as i64);

    // Audio is best-effort: a configuration failure yields a video-only output
    // rather than failing the whole compression.
    let audio_input = streams
        .audio
        .and_then(|index| configure_audio_output(&reader, index).map(|t| (index, t)));

    let sink = create_sink(&request.output_path)?;

    let h264 = build_h264_type(out_w, out_h, plan.bitrate_bps, out_rate)?;
    let video_sink = unsafe { sink.AddStream(&h264) }
        .map_err(|err| PlatformError::Platform(format!("add video stream: {err}")))?;
    unsafe {
        sink.SetInputMediaType(video_sink, &video_input, None)
            .map_err(|err| PlatformError::Platform(format!("video input: {err}")))?;
    }

    let (audio_index, audio_sink) = match &audio_input {
        Some((index, pcm)) => match build_aac_type(pcm) {
            Ok(aac) => unsafe {
                match sink.AddStream(&aac) {
                    Ok(stream) => match sink.SetInputMediaType(stream, pcm, None) {
                        Ok(()) => (Some(*index), Some(stream)),
                        Err(err) => {
                            log::warn!("audio input type rejected: {err}; encoding video only");
                            (None, None)
                        }
                    },
                    Err(err) => {
                        log::warn!("audio stream rejected: {err}; encoding video only");
                        (None, None)
                    }
                }
            },
            Err(err) => {
                log::warn!("aac setup failed: {err}; encoding video only");
                (None, None)
            }
        },
        None => (None, None),
    };

    // Drop the audio stream selection if we could not wire its encoder.
    if audio_sink.is_none() {
        if let Some(index) = streams.audio {
            unsafe {
                let _ = reader.SetStreamSelection(index, false);
            }
        }
    }

    unsafe {
        sink.BeginWriting()
            .map_err(|err| PlatformError::Platform(format!("begin writing: {err}")))?;
    }

    pump_samples(
        &reader,
        &sink,
        streams.video,
        video_sink,
        audio_index,
        audio_sink,
        drop_interval,
    )?;

    unsafe {
        sink.Finalize()
            .map_err(|err| PlatformError::Platform(format!("finalize: {err}")))?;
    }

    let size = fs::metadata(&request.output_path)
        .map(|m| m.len())
        .unwrap_or(0);
    Ok(CompressedVideo {
        path: request.output_path.clone(),
        width: out_w,
        height: out_h,
        duration_ms: total_duration_ms,
        size,
        mime_type: Some("video/mp4".to_string()),
    })
}

/// Copy the source verbatim into the output slot and report it as the result.
fn copy_source(
    request: &CompressVideoRequest,
    source: &Path,
) -> Result<CompressedVideo, PlatformError> {
    fs::copy(source, &request.output_path).map_err(|err| {
        PlatformError::Platform(format!(
            "fallback copy {} -> {}: {err}",
            source.display(),
            request.output_path.display()
        ))
    })?;
    let info = super::video_info::read_video_info(&request.output_path)?;
    let size = fs::metadata(&request.output_path)
        .map(|m| m.len())
        .unwrap_or(0);
    Ok(CompressedVideo {
        path: request.output_path.clone(),
        width: info.width,
        height: info.height,
        duration_ms: info.duration_ms,
        size,
        mime_type: info.mime_type.or_else(|| Some("video/mp4".to_string())),
    })
}

pub(super) fn compress_video(
    request: &CompressVideoRequest,
    source: &Path,
) -> Result<CompressedVideo, PlatformError> {
    if paths_refer_to_same_file(source, &request.output_path) {
        return Err(PlatformError::InvalidParameter(
            "compress_video output path must be different from source path".to_string(),
        ));
    }

    if let Some(parent) = request.output_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            PlatformError::Platform(format!("failed to create {}: {err}", parent.display()))
        })?;
    }

    match transcode(request, source) {
        Ok(result) => {
            let src_size = fs::metadata(source).map(|m| m.len()).unwrap_or(0);
            // A re-encode that did not shrink the file is worse than the source —
            // hand back the original instead.
            if src_size > 0 && result.size >= src_size {
                log::info!(
                    "compress_video: output ({}) >= source ({}); using source copy",
                    result.size,
                    src_size
                );
                copy_source(request, source)
            } else {
                Ok(result)
            }
        }
        Err(err) => {
            log::warn!("compress_video transcode failed: {err}; falling back to source copy");
            copy_source(request, source)
        }
    }
}
