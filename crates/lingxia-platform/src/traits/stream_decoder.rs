use lingxia_error::Error;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
    H265,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoFormat {
    AnnexB,
    Avcc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioCodec {
    Aac,
    PcmS16le,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoStreamConfig {
    pub codec: VideoCodec,
    pub format: VideoFormat,
    pub sps: Vec<u8>,
    pub pps: Vec<u8>,
    pub vps: Vec<u8>,
    pub nal_length_size: Option<u8>,
    pub width: Option<u16>,
    pub height: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioStreamConfig {
    pub codec: AudioCodec,
    pub audio_specific_config: Vec<u8>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub aac_is_adts: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoFrame {
    pub data: Vec<u8>,
    pub dts_ms: u32,
    pub pts_ms: u32,
    pub keyframe: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFrame {
    pub data: Vec<u8>,
    pub dts_ms: u32,
    pub pts_ms: u32,
}

pub trait VideoStreamDecoderHandle: Send + Sync {
    fn supports_soft_reset(&self) -> bool {
        false
    }

    fn supports_in_place_hard_reset(&self) -> bool {
        false
    }

    fn reset_stream(&self, _hard: bool) -> Result<(), Error> {
        Ok(())
    }

    fn configure_video(&self, config: VideoStreamConfig) -> Result<(), Error>;
    fn configure_audio(&self, config: AudioStreamConfig) -> Result<(), Error>;
    fn push_video(&self, frame: VideoFrame) -> Result<(), Error>;
    fn push_audio(&self, frame: AudioFrame) -> Result<(), Error>;
    fn stop(&self) -> Result<(), Error>;
    fn flush(&self) -> Result<(), Error> {
        Ok(())
    }
}

pub trait VideoStreamDecoderManager: Send + Sync + 'static {
    fn create_stream_decoder(
        &self,
        component_id: &str,
    ) -> Result<Box<dyn VideoStreamDecoderHandle>, Error>;
}
