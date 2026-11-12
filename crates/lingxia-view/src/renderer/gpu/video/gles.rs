//! Video frames on a GLES device, inside wgpu's own context: the decoder's image is latched into
//! an external texture and drawn into RGBA textures wgpu owns. Commands in one context run in
//! order, so no fence sits between the draw and wgpu's sampling.
use std::ffi::CStr;
use std::num::NonZeroU32;

/// The platform image a decoder writes, driven in the current GL context.
pub trait Image {
    /// Attaches the image to external texture `external`.
    fn attach(&mut self, external: u32) -> Result<(), String>;
    /// Detaches the image; the context may then delete the external texture.
    fn detach(&mut self);
    /// Latches the newest decoded frame: its texture transform, or `None` with none new.
    fn latch(&mut self) -> Result<Option<[f32; 16]>, String>;
}

/// What [`Frames::next`] drew.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drawn {
    /// The decoder's newest frame.
    Frame,
    /// The still rows given in its place.
    Still,
}

/// Frames in flight: the one wgpu samples and the one being drawn.
const SLOTS: usize = 2;

/// One player's frame textures on one GLES device.
#[derive(Default)]
pub struct Frames {
    blit: Option<Blit>,
    /// The device whose context owns `blit`; names are per context, so teardown happens there.
    device: Option<wgpu::Device>,
    slots: Vec<wgpu::Texture>,
    next: usize,
    size: [u32; 2],
    /// A GL error left pending in the context was logged once.
    drained: bool,
}

/// Whether `device` runs on GLES, where frames take this path.
pub fn applies(device: &wgpu::Device) -> bool {
    unsafe { device.as_hal::<wgpu::hal::api::Gles>() }.is_some()
}

impl Frames {
    /// Draws `image`'s newest frame of `size`, or `still` (tight RGBA rows of `size`) while it
    /// has none, into the next slot. Render thread only.
    pub fn next(
        &mut self,
        device: &wgpu::Device,
        image: &mut dyn Image,
        size: [u32; 2],
        still: Option<&[u8]>,
    ) -> Result<Option<(Drawn, wgpu::Texture, [u16; 2])>, String> {
        if size[0] == 0 || size[1] == 0 {
            return Ok(None);
        }
        if self.device.as_ref().is_some_and(|owner| owner != device) {
            self.release(image);
        }
        if size != self.size {
            // wgpu frees the old frames on its own schedule, outside the context lock below.
            self.slots.clear();
            self.next = 0;
            self.size = [0, 0];
        }
        let hal = unsafe { device.as_hal::<wgpu::hal::api::Gles>() }
            .ok_or("VIDEO_DECODE: not a GLES device")?;
        let mut created = Vec::new();
        let latched = {
            let _context = hal.context().lock();
            if self.blit.is_none() {
                let blit = unsafe { Blit::new()? };
                if let Err(error) = image.attach(blit.external) {
                    unsafe { blit.delete() };
                    return Err(error);
                }
                self.blit = Some(blit);
                self.device = Some(device.clone());
            }
            if self.slots.is_empty() {
                for _ in 0..SLOTS {
                    created.push(unsafe { color_texture(size) });
                }
            }
            // A latch reports and clears any error pending in the context (Android logs it per
            // frame); what wgpu left behind is drained first and logged once.
            let pending = unsafe { drain_errors() };
            if pending != 0 && !std::mem::replace(&mut self.drained, true) {
                log::debug!(target: "lingxia_view", "video frame path: GL error {pending:#x} pending before the latch");
            }
            image.latch()?
        };
        if !created.is_empty() {
            for name in created {
                let name = NonZeroU32::new(name).ok_or("VIDEO_DECODE: no GL texture")?;
                self.slots.push(unsafe { wrap(&hal, device, name, size) });
            }
            self.size = size;
        }
        let drawn = match (latched, still) {
            (Some(_), _) => Drawn::Frame,
            (None, Some(_)) => Drawn::Still,
            (None, None) => return Ok(None),
        };
        let index = self.next;
        let slot = self.slots[index].clone();
        let name = unsafe { slot.as_hal::<wgpu::hal::api::Gles>() }
            .and_then(|texture| match &texture.inner {
                wgpu::hal::gles::TextureInner::Texture { raw, .. } => Some(raw.0.get()),
                _ => None,
            })
            .ok_or("VIDEO_DECODE: no GL name for the frame")?;
        let result = {
            let _context = hal.context().lock();
            match (latched, still) {
                (Some(matrix), _) => {
                    let blit = self.blit.as_ref().ok_or("VIDEO_DECODE: no GL")?;
                    unsafe { blit.draw(name, size, &matrix) }
                }
                (None, Some(rows)) => unsafe { upload(name, size, rows) },
                (None, None) => unreachable!(),
            }
        };
        if let Err(error) = result {
            // A target that lost its storage is made again at the next frame.
            self.slots.clear();
            self.size = [0, 0];
            self.next = 0;
            return Err(error);
        }
        self.next = (index + 1) % self.slots.len();
        Ok(Some((
            drawn,
            slot,
            [size[0].min(65535) as u16, size[1].min(65535) as u16],
        )))
    }

    /// Whether GL names exist that [`Frames::release`] must free in their context.
    pub fn bound(&self) -> bool {
        self.blit.is_some()
    }

    /// Detaches `image` and frees the GL names in the context that made them. Render thread
    /// only: the lock makes that context current on the calling thread.
    pub fn release(&mut self, image: &mut dyn Image) {
        // wgpu frees these frames itself, under its own lock.
        self.slots.clear();
        self.size = [0, 0];
        self.next = 0;
        let (Some(blit), Some(device)) = (self.blit.take(), self.device.take()) else {
            return;
        };
        let Some(hal) = (unsafe { device.as_hal::<wgpu::hal::api::Gles>() }) else {
            return;
        };
        let _context = hal.context().lock();
        image.detach();
        unsafe {
            // A detach may delete the texture itself; checked under the same lock, so the name
            // cannot have gone to another player in between.
            if glIsTexture(blit.external) != 0 {
                glDeleteTextures(1, &blit.external);
            }
            blit.delete_pipeline();
        }
    }
}

struct Blit {
    external: u32,
    program: u32,
    transform: i32,
    framebuffer: u32,
}

const VERTEX: &CStr = c"#version 300 es
uniform mat4 transform;
out vec2 uv;
void main() {
    vec2 p = vec2(float(gl_VertexID & 1), float(gl_VertexID >> 1));
    uv = (transform * vec4(p.x, 1.0 - p.y, 0.0, 1.0)).xy;
    gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}";

const FRAGMENT: &CStr = c"#version 300 es
#extension GL_OES_EGL_image_external_essl3 : require
precision mediump float;
uniform samplerExternalOES frame;
in vec2 uv;
out vec4 color;
void main() { color = texture(frame, uv); }";

impl Blit {
    unsafe fn new() -> Result<Self, String> {
        unsafe {
            let vertex = shader(GL_VERTEX_SHADER, VERTEX)?;
            let fragment = match shader(GL_FRAGMENT_SHADER, FRAGMENT) {
                Ok(fragment) => fragment,
                Err(error) => {
                    glDeleteShader(vertex);
                    return Err(error);
                }
            };
            let program = glCreateProgram();
            glAttachShader(program, vertex);
            glAttachShader(program, fragment);
            glLinkProgram(program);
            glDeleteShader(vertex);
            glDeleteShader(fragment);
            let mut linked = 0;
            glGetProgramiv(program, GL_LINK_STATUS, &mut linked);
            if linked == 0 {
                glDeleteProgram(program);
                return Err("VIDEO_DECODE: blit program did not link".into());
            }
            let transform = glGetUniformLocation(program, c"transform".as_ptr());
            let mut external = 0;
            glGenTextures(1, &mut external);
            glBindTexture(GL_TEXTURE_EXTERNAL_OES, external);
            glTexParameteri(GL_TEXTURE_EXTERNAL_OES, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
            glTexParameteri(GL_TEXTURE_EXTERNAL_OES, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
            glTexParameteri(GL_TEXTURE_EXTERNAL_OES, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
            glTexParameteri(GL_TEXTURE_EXTERNAL_OES, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
            glBindTexture(GL_TEXTURE_EXTERNAL_OES, 0);
            let mut framebuffer = 0;
            glGenFramebuffers(1, &mut framebuffer);
            Ok(Self {
                external,
                program,
                transform,
                framebuffer,
            })
        }
    }

    /// Deletes the program and framebuffer; the external texture goes with its image.
    unsafe fn delete_pipeline(&self) {
        unsafe {
            glDeleteProgram(self.program);
            glDeleteFramebuffers(1, &self.framebuffer);
        }
    }

    /// Deletes everything, for a blit no image was attached to.
    unsafe fn delete(&self) {
        unsafe {
            glDeleteTextures(1, &self.external);
            self.delete_pipeline();
        }
    }

    /// Draws the latched frame into `color`, leaving the state wgpu resets per submission.
    unsafe fn draw(&self, color: u32, size: [u32; 2], matrix: &[f32; 16]) -> Result<(), String> {
        unsafe {
            glBindFramebuffer(GL_FRAMEBUFFER, self.framebuffer);
            glFramebufferTexture2D(
                GL_FRAMEBUFFER,
                GL_COLOR_ATTACHMENT0,
                GL_TEXTURE_2D,
                color,
                0,
            );
            let complete = glCheckFramebufferStatus(GL_FRAMEBUFFER) == GL_FRAMEBUFFER_COMPLETE;
            if complete {
                // wgpu leaves its sampler objects bound; one on unit 0 would override the
                // external texture's own filtering.
                glBindSampler(0, 0);
                glViewport(0, 0, size[0] as i32, size[1] as i32);
                glDisable(GL_BLEND);
                glDisable(GL_SCISSOR_TEST);
                glDisable(GL_DEPTH_TEST);
                glDisable(GL_STENCIL_TEST);
                glDisable(GL_CULL_FACE);
                glColorMask(1, 1, 1, 1);
                glUseProgram(self.program);
                glUniformMatrix4fv(self.transform, 1, 0, matrix.as_ptr());
                glActiveTexture(GL_TEXTURE0);
                glBindTexture(GL_TEXTURE_EXTERNAL_OES, self.external);
                glDrawArrays(GL_TRIANGLE_STRIP, 0, 4);
                glBindTexture(GL_TEXTURE_EXTERNAL_OES, 0);
                glUseProgram(0);
            }
            glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, 0, 0);
            glBindFramebuffer(GL_FRAMEBUFFER, 0);
            glFlush();
            if complete {
                Ok(())
            } else {
                Err("VIDEO_DECODE: the frame texture is not renderable".into())
            }
        }
    }
}

/// Clears the context's error flags: the first one, or 0.
unsafe fn drain_errors() -> u32 {
    unsafe {
        let first = glGetError();
        // Bounded: each call clears one flag, and a lost context keeps reporting.
        for _ in 0..8 {
            if glGetError() == 0 {
                break;
            }
        }
        first
    }
}

/// Copies tight RGBA `rows` of `size` into texture `name`.
unsafe fn upload(name: u32, size: [u32; 2], rows: &[u8]) -> Result<(), String> {
    if rows.len() != size[0] as usize * size[1] as usize * 4 {
        return Err("VIDEO_DECODE: still rows do not match the frame size".into());
    }
    unsafe {
        // wgpu's buffer copies leave unpack state behind.
        glBindBuffer(GL_PIXEL_UNPACK_BUFFER, 0);
        glPixelStorei(GL_UNPACK_ALIGNMENT, 4);
        glPixelStorei(GL_UNPACK_ROW_LENGTH, 0);
        glPixelStorei(GL_UNPACK_SKIP_ROWS, 0);
        glPixelStorei(GL_UNPACK_SKIP_PIXELS, 0);
        glBindTexture(GL_TEXTURE_2D, name);
        glTexSubImage2D(
            GL_TEXTURE_2D,
            0,
            0,
            0,
            size[0] as i32,
            size[1] as i32,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            rows.as_ptr().cast(),
        );
        glBindTexture(GL_TEXTURE_2D, 0);
        glFlush();
    }
    Ok(())
}

/// An RGBA texture in the current context, for wgpu to own once wrapped.
unsafe fn color_texture(size: [u32; 2]) -> u32 {
    unsafe {
        let mut name = 0;
        glGenTextures(1, &mut name);
        glBindTexture(GL_TEXTURE_2D, name);
        glTexStorage2D(GL_TEXTURE_2D, 1, GL_RGBA8, size[0] as i32, size[1] as i32);
        glBindTexture(GL_TEXTURE_2D, 0);
        name
    }
}

/// Hands texture `name` to wgpu, which deletes it with the texture.
unsafe fn wrap(
    hal: &wgpu::hal::gles::Device,
    device: &wgpu::Device,
    name: NonZeroU32,
    size: [u32; 2],
) -> wgpu::Texture {
    let extent = wgpu::Extent3d {
        width: size[0],
        height: size[1],
        depth_or_array_layers: 1,
    };
    unsafe {
        let texture = hal.texture_from_raw(
            name,
            &wgpu::hal::TextureDescriptor {
                label: Some("LingXia video frame"),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUses::RESOURCE,
                memory_flags: wgpu::hal::MemoryFlags::empty(),
                view_formats: Vec::new(),
            },
            None,
        );
        device.create_texture_from_hal::<wgpu::hal::api::Gles>(
            texture,
            &wgpu::TextureDescriptor {
                label: Some("LingXia video frame"),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::TextureUses::RESOURCE,
        )
    }
}

unsafe fn shader(kind: u32, source: &CStr) -> Result<u32, String> {
    unsafe {
        let shader = glCreateShader(kind);
        let text = source.as_ptr();
        glShaderSource(shader, 1, &text, std::ptr::null());
        glCompileShader(shader);
        let mut compiled = 0;
        glGetShaderiv(shader, GL_COMPILE_STATUS, &mut compiled);
        if compiled == 0 {
            let mut log = [0u8; 512];
            let mut length = 0;
            glGetShaderInfoLog(
                shader,
                log.len() as i32,
                &mut length,
                log.as_mut_ptr().cast(),
            );
            glDeleteShader(shader);
            let length = (length.max(0) as usize).min(log.len());
            return Err(format!(
                "VIDEO_DECODE: shader: {}",
                String::from_utf8_lossy(&log[..length])
            ));
        }
        Ok(shader)
    }
}

const GL_TEXTURE_EXTERNAL_OES: u32 = 0x8D65;
const GL_TEXTURE_2D: u32 = 0x0DE1;
const GL_RGBA: u32 = 0x1908;
const GL_RGBA8: u32 = 0x8058;
const GL_UNSIGNED_BYTE: u32 = 0x1401;
const GL_TEXTURE_MIN_FILTER: u32 = 0x2801;
const GL_TEXTURE_MAG_FILTER: u32 = 0x2800;
const GL_TEXTURE_WRAP_S: u32 = 0x2802;
const GL_TEXTURE_WRAP_T: u32 = 0x2803;
const GL_LINEAR: i32 = 0x2601;
const GL_CLAMP_TO_EDGE: i32 = 0x812F;
const GL_FRAMEBUFFER: u32 = 0x8D40;
const GL_COLOR_ATTACHMENT0: u32 = 0x8CE0;
const GL_FRAMEBUFFER_COMPLETE: u32 = 0x8CD5;
const GL_VERTEX_SHADER: u32 = 0x8B31;
const GL_FRAGMENT_SHADER: u32 = 0x8B30;
const GL_COMPILE_STATUS: u32 = 0x8B81;
const GL_LINK_STATUS: u32 = 0x8B82;
const GL_TRIANGLE_STRIP: u32 = 0x0005;
const GL_TEXTURE0: u32 = 0x84C0;
const GL_BLEND: u32 = 0x0BE2;
const GL_SCISSOR_TEST: u32 = 0x0C11;
const GL_DEPTH_TEST: u32 = 0x0B71;
const GL_STENCIL_TEST: u32 = 0x0B90;
const GL_CULL_FACE: u32 = 0x0B44;
const GL_PIXEL_UNPACK_BUFFER: u32 = 0x88EC;
const GL_UNPACK_ALIGNMENT: u32 = 0x0CF5;
const GL_UNPACK_ROW_LENGTH: u32 = 0x0CF2;
const GL_UNPACK_SKIP_ROWS: u32 = 0x0CF3;
const GL_UNPACK_SKIP_PIXELS: u32 = 0x0CF4;

#[link(name = "GLESv3")]
unsafe extern "C" {
    fn glGenTextures(count: i32, names: *mut u32);
    fn glDeleteTextures(count: i32, names: *const u32);
    fn glIsTexture(name: u32) -> u8;
    fn glBindTexture(target: u32, name: u32);
    fn glTexParameteri(target: u32, name: u32, value: i32);
    fn glTexStorage2D(target: u32, levels: i32, format: u32, width: i32, height: i32);
    fn glTexSubImage2D(
        target: u32,
        level: i32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        format: u32,
        kind: u32,
        pixels: *const std::ffi::c_void,
    );
    fn glPixelStorei(name: u32, value: i32);
    fn glBindBuffer(target: u32, name: u32);
    fn glGenFramebuffers(count: i32, names: *mut u32);
    fn glDeleteFramebuffers(count: i32, names: *const u32);
    fn glBindFramebuffer(target: u32, name: u32);
    fn glFramebufferTexture2D(target: u32, attachment: u32, kind: u32, texture: u32, level: i32);
    fn glCheckFramebufferStatus(target: u32) -> u32;
    fn glBindSampler(unit: u32, sampler: u32);
    fn glCreateShader(kind: u32) -> u32;
    fn glShaderSource(
        shader: u32,
        count: i32,
        sources: *const *const std::ffi::c_char,
        lengths: *const i32,
    );
    fn glCompileShader(shader: u32);
    fn glGetShaderiv(shader: u32, name: u32, value: *mut i32);
    fn glGetShaderInfoLog(shader: u32, size: i32, length: *mut i32, log: *mut std::ffi::c_char);
    fn glDeleteShader(shader: u32);
    fn glCreateProgram() -> u32;
    fn glAttachShader(program: u32, shader: u32);
    fn glLinkProgram(program: u32);
    fn glGetProgramiv(program: u32, name: u32, value: *mut i32);
    fn glDeleteProgram(program: u32);
    fn glGetUniformLocation(program: u32, name: *const std::ffi::c_char) -> i32;
    fn glUseProgram(program: u32);
    fn glUniformMatrix4fv(location: i32, count: i32, transpose: u8, value: *const f32);
    fn glActiveTexture(unit: u32);
    fn glViewport(x: i32, y: i32, width: i32, height: i32);
    fn glDisable(capability: u32);
    fn glColorMask(red: u8, green: u8, blue: u8, alpha: u8);
    fn glDrawArrays(mode: u32, first: i32, count: i32);
    fn glFlush();
    fn glGetError() -> u32;
}
