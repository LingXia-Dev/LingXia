//! ExoPlayer frames for the GPU renderer. ExoPlayer decodes into a SurfaceTexture; the render
//! thread latches each frame, draws it into an RGBA AHardwareBuffer with GLES and hands the
//! buffer to Vulkan, which samples the same memory. `glFinish` orders the GL write first.
use jni::objects::{Global, JClass, JFloatArray, JObject};
use jni::{Env, JValue, JavaVM, jni_sig, jni_str};
use lingxia_view::renderer::gpu::video::{Player, import_hardware_buffer, install};
use lingxia_view::renderer::gpu::wgpu;
use std::ffi::{CStr, c_void};
use std::sync::{Mutex, OnceLock};

/// Frames in flight: GL draws one while Vulkan may still sample the previous ones.
const SLOTS: usize = 4;

static CLASS: OnceLock<Global<JClass<'static>>> = OnceLock::new();

/// Resolves `com.lingxia.view.GpuVideo` on a Java thread, where app classes are visible, and
/// installs the players. Missing class: no video.
pub(super) fn register(env: &mut Env) {
    if CLASS.get().is_some() {
        return;
    }
    let Ok(class) = env.find_class(jni_str!("com/lingxia/view/GpuVideo")) else {
        let _ = env.exception_clear();
        return;
    };
    if let Ok(class) = env.new_global_ref(&class)
        && CLASS.set(class).is_ok()
    {
        install(open);
    }
}

fn java<T>(work: impl FnOnce(&mut Env) -> jni::errors::Result<T>) -> Result<T, String> {
    JavaVM::singleton()
        .and_then(|vm| vm.attach_current_thread(|env| work(env)))
        .map_err(|e| format!("GpuVideo: {e}"))
}

fn open(source: &str) -> Result<Box<dyn Player>, String> {
    let class = CLASS.get().ok_or("GpuVideo: not registered")?;
    let player = java(|env| {
        let source = env.new_string(source)?;
        let player = env
            .call_static_method(
                class,
                jni_str!("open"),
                jni_sig!("(Ljava/lang/String;)Lcom/lingxia/view/GpuVideo;"),
                &[JValue::Object(&source)],
            )?
            .l()?;
        env.new_global_ref(&player)
    })?;
    Ok(Box::new(Video {
        java: player,
        gl: None,
        slots: Vec::new(),
        next: 0,
        size: [0, 0],
    }))
}

struct Video {
    java: Global<JObject<'static>>,
    gl: Option<Blit>,
    slots: Vec<Slot>,
    next: usize,
    size: [u32; 2],
}

// The GL names and buffers are used on the render thread only; drop defers their GL half.
unsafe impl Send for Video {}

struct Blit {
    external: u32,
    program: u32,
    transform: i32,
    framebuffer: u32,
}

struct Slot {
    buffer: *mut c_void,
    image: *mut c_void,
    color: u32,
    texture: wgpu::Texture,
}

impl Player for Video {
    fn has_new(&self) -> bool {
        java(|env| {
            env.call_method(&*self.java, jni_str!("available"), jni_sig!("()Z"), &[])?
                .z()
        })
        .unwrap_or(false)
    }

    fn next_frame(
        &mut self,
        device: &wgpu::Device,
    ) -> Result<Option<(wgpu::Texture, [u16; 2])>, String> {
        let size = java(|env| {
            let width = env
                .call_method(&*self.java, jni_str!("width"), jni_sig!("()I"), &[])?
                .i()?;
            let height = env
                .call_method(&*self.java, jni_str!("height"), jni_sig!("()I"), &[])?
                .i()?;
            Ok([width.max(0) as u32, height.max(0) as u32])
        })?;
        if size[0] == 0 || size[1] == 0 {
            return Ok(None);
        }
        let _current = egl()?.enter();
        unsafe { collect() };
        if self.gl.is_none() {
            let blit = unsafe { Blit::new()? };
            let external = blit.external as i32;
            self.gl = Some(blit);
            java(|env| {
                env.call_method(
                    &*self.java,
                    jni_str!("attach"),
                    jni_sig!("(I)V"),
                    &[JValue::Int(external)],
                )
                .map(drop)
            })?;
        }
        if size != self.size {
            self.release_slots();
            for _ in 0..SLOTS {
                self.slots.push(unsafe { Slot::new(device, size)? });
            }
            self.size = size;
        }
        let mut matrix = [0f32; 16];
        let latched = java(|env| {
            let array = env
                .call_method(&*self.java, jni_str!("update"), jni_sig!("()[F"), &[])?
                .l()?;
            if array.is_null() {
                return Ok(false);
            }
            let array = env.cast_local::<JFloatArray>(array)?;
            array.get_region(env, 0, &mut matrix)?;
            Ok(true)
        })?;
        if !latched {
            return Ok(None);
        }
        let slot = &self.slots[self.next];
        self.next = (self.next + 1) % self.slots.len();
        let gl = self.gl.as_ref().ok_or("GpuVideo: no GL")?;
        unsafe { gl.draw(slot.color, size, &matrix)? };
        Ok(Some((
            slot.texture.clone(),
            [size[0].min(65535) as u16, size[1].min(65535) as u16],
        )))
    }
}

impl Video {
    fn release_slots(&mut self) {
        for slot in self.slots.drain(..) {
            defer(Garbage::Texture(slot.color));
            unsafe {
                if let Ok(egl) = egl() {
                    (egl.destroy_image)(egl.display, slot.image);
                }
                AHardwareBuffer_release(slot.buffer);
            }
        }
    }
}

impl Drop for Video {
    fn drop(&mut self) {
        self.release_slots();
        if let Some(gl) = self.gl.take() {
            defer(Garbage::Texture(gl.external));
            defer(Garbage::Program(gl.program));
            defer(Garbage::Framebuffer(gl.framebuffer));
        }
        let _ = java(|env| {
            env.call_method(&*self.java, jni_str!("close"), jni_sig!("()V"), &[])
                .map(drop)
        });
    }
}

/// GL names freed on the render thread, whose context owns them; players close elsewhere.
enum Garbage {
    Texture(u32),
    Program(u32),
    Framebuffer(u32),
}

static GARBAGE: Mutex<Vec<Garbage>> = Mutex::new(Vec::new());

fn defer(garbage: Garbage) {
    GARBAGE
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .push(garbage);
}

unsafe fn collect() {
    let garbage = std::mem::take(&mut *GARBAGE.lock().unwrap_or_else(|p| p.into_inner()));
    for garbage in garbage {
        unsafe {
            match garbage {
                Garbage::Texture(name) => glDeleteTextures(1, &name),
                Garbage::Program(name) => glDeleteProgram(name),
                Garbage::Framebuffer(name) => glDeleteFramebuffers(1, &name),
            }
        }
    }
}

impl Slot {
    unsafe fn new(device: &wgpu::Device, size: [u32; 2]) -> Result<Self, String> {
        unsafe {
            let desc = AHardwareBufferDesc {
                width: size[0],
                height: size[1],
                layers: 1,
                format: AHARDWAREBUFFER_FORMAT_R8G8B8A8_UNORM,
                usage: AHARDWAREBUFFER_USAGE_GPU_SAMPLED_IMAGE
                    | AHARDWAREBUFFER_USAGE_GPU_COLOR_OUTPUT,
                stride: 0,
                rfu0: 0,
                rfu1: 0,
            };
            let mut buffer = std::ptr::null_mut();
            if AHardwareBuffer_allocate(&desc, &mut buffer) != 0 || buffer.is_null() {
                return Err("AHardwareBuffer_allocate failed".into());
            }
            let egl = egl()?;
            let client = (egl.client_buffer)(buffer);
            let attributes = [EGL_IMAGE_PRESERVED_KHR, 1, EGL_NONE];
            let image = (egl.create_image)(
                egl.display,
                std::ptr::null_mut(),
                EGL_NATIVE_BUFFER_ANDROID,
                client,
                attributes.as_ptr(),
            );
            if image.is_null() {
                AHardwareBuffer_release(buffer);
                return Err(format!("eglCreateImageKHR {:#x}", eglGetError()));
            }
            let mut color = 0;
            glGenTextures(1, &mut color);
            glBindTexture(GL_TEXTURE_2D, color);
            (egl.target_texture)(GL_TEXTURE_2D, image);
            glBindTexture(GL_TEXTURE_2D, 0);
            let texture = match import_hardware_buffer(device, buffer, size) {
                Ok(texture) => texture,
                Err(error) => {
                    glDeleteTextures(1, &color);
                    (egl.destroy_image)(egl.display, image);
                    AHardwareBuffer_release(buffer);
                    return Err(error);
                }
            };
            Ok(Self {
                buffer,
                image,
                color,
                texture,
            })
        }
    }
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
            let fragment = shader(GL_FRAGMENT_SHADER, FRAGMENT)?;
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
                return Err("GpuVideo: blit program did not link".into());
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
            if glCheckFramebufferStatus(GL_FRAMEBUFFER) != GL_FRAMEBUFFER_COMPLETE {
                glBindFramebuffer(GL_FRAMEBUFFER, 0);
                return Err("GpuVideo: AHardwareBuffer is not renderable".into());
            }
            glViewport(0, 0, size[0] as i32, size[1] as i32);
            glDisable(GL_BLEND);
            glDisable(GL_SCISSOR_TEST);
            glDisable(GL_DEPTH_TEST);
            glUseProgram(self.program);
            glUniformMatrix4fv(self.transform, 1, 0, matrix.as_ptr());
            glActiveTexture(GL_TEXTURE0);
            glBindTexture(GL_TEXTURE_EXTERNAL_OES, self.external);
            glDrawArrays(GL_TRIANGLE_STRIP, 0, 4);
            glBindTexture(GL_TEXTURE_EXTERNAL_OES, 0);
            glBindFramebuffer(GL_FRAMEBUFFER, 0);
            // Vulkan samples the buffer next, outside any GL fence.
            glFinish();
            Ok(())
        }
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
                "GpuVideo: shader: {}",
                String::from_utf8_lossy(&log[..length])
            ));
        }
        Ok(shader)
    }
}

/// The video GL context, current on the render thread only while a frame is drawn.
struct Egl {
    display: *mut c_void,
    context: *mut c_void,
    surface: *mut c_void,
    create_image:
        unsafe extern "C" fn(*mut c_void, *mut c_void, u32, *mut c_void, *const i32) -> *mut c_void,
    destroy_image: unsafe extern "C" fn(*mut c_void, *mut c_void) -> u32,
    client_buffer: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    target_texture: unsafe extern "C" fn(u32, *mut c_void),
}

unsafe impl Send for Egl {}
unsafe impl Sync for Egl {}

static EGL: OnceLock<Result<Egl, String>> = OnceLock::new();

fn egl() -> Result<&'static Egl, String> {
    EGL.get_or_init(|| unsafe { Egl::new() })
        .as_ref()
        .map_err(Clone::clone)
}

/// Restores the context that was current before [`Egl::enter`]; with none, ours stays.
struct Current {
    previous: Option<[*mut c_void; 4]>,
}

impl Drop for Current {
    fn drop(&mut self) {
        if let Some([display, draw, read, context]) = self.previous {
            unsafe { eglMakeCurrent(display, draw, read, context) };
        }
    }
}

impl Egl {
    unsafe fn new() -> Result<Self, String> {
        unsafe {
            let display = eglGetDisplay(std::ptr::null_mut());
            if display.is_null()
                || eglInitialize(display, std::ptr::null_mut(), std::ptr::null_mut()) == 0
            {
                return Err("GpuVideo: no EGL display".into());
            }
            let attributes = [
                EGL_RENDERABLE_TYPE,
                EGL_OPENGL_ES3_BIT,
                EGL_SURFACE_TYPE,
                EGL_PBUFFER_BIT,
                EGL_RED_SIZE,
                8,
                EGL_GREEN_SIZE,
                8,
                EGL_BLUE_SIZE,
                8,
                EGL_ALPHA_SIZE,
                8,
                EGL_NONE,
            ];
            let mut config = std::ptr::null_mut();
            let mut count = 0;
            if eglChooseConfig(display, attributes.as_ptr(), &mut config, 1, &mut count) == 0
                || count == 0
            {
                return Err("GpuVideo: no EGL config".into());
            }
            let context_attributes = [EGL_CONTEXT_CLIENT_VERSION, 3, EGL_NONE];
            let context = eglCreateContext(
                display,
                config,
                std::ptr::null_mut(),
                context_attributes.as_ptr(),
            );
            if context.is_null() {
                return Err(format!("GpuVideo: eglCreateContext {:#x}", eglGetError()));
            }
            let surface_attributes = [EGL_WIDTH, 1, EGL_HEIGHT, 1, EGL_NONE];
            let surface = eglCreatePbufferSurface(display, config, surface_attributes.as_ptr());
            if surface.is_null() {
                return Err(format!(
                    "GpuVideo: eglCreatePbufferSurface {:#x}",
                    eglGetError()
                ));
            }
            macro_rules! proc {
                ($name:literal) => {{
                    let address = eglGetProcAddress($name.as_ptr());
                    if address.is_null() {
                        return Err(format!("GpuVideo: no {:?}", $name));
                    }
                    std::mem::transmute::<*mut c_void, _>(address)
                }};
            }
            Ok(Self {
                display,
                context,
                surface,
                create_image: proc!(c"eglCreateImageKHR"),
                destroy_image: proc!(c"eglDestroyImageKHR"),
                client_buffer: proc!(c"eglGetNativeClientBufferANDROID"),
                target_texture: proc!(c"glEGLImageTargetTexture2DOES"),
            })
        }
    }

    fn enter(&self) -> Current {
        unsafe {
            let context = eglGetCurrentContext();
            if context == self.context {
                return Current { previous: None };
            }
            let previous = (!context.is_null()).then(|| {
                [
                    eglGetCurrentDisplay(),
                    eglGetCurrentSurface(EGL_DRAW),
                    eglGetCurrentSurface(EGL_READ),
                    context,
                ]
            });
            eglMakeCurrent(self.display, self.surface, self.surface, self.context);
            Current { previous }
        }
    }
}

const EGL_NONE: i32 = 0x3038;
const EGL_RENDERABLE_TYPE: i32 = 0x3040;
const EGL_OPENGL_ES3_BIT: i32 = 0x40;
const EGL_SURFACE_TYPE: i32 = 0x3033;
const EGL_PBUFFER_BIT: i32 = 0x1;
const EGL_RED_SIZE: i32 = 0x3024;
const EGL_GREEN_SIZE: i32 = 0x3023;
const EGL_BLUE_SIZE: i32 = 0x3022;
const EGL_ALPHA_SIZE: i32 = 0x3021;
const EGL_WIDTH: i32 = 0x3057;
const EGL_HEIGHT: i32 = 0x3056;
const EGL_CONTEXT_CLIENT_VERSION: i32 = 0x3098;
const EGL_DRAW: i32 = 0x3059;
const EGL_READ: i32 = 0x305A;
const EGL_NATIVE_BUFFER_ANDROID: u32 = 0x3140;
const EGL_IMAGE_PRESERVED_KHR: i32 = 0x30D2;

const GL_TEXTURE_EXTERNAL_OES: u32 = 0x8D65;
const GL_TEXTURE_2D: u32 = 0x0DE1;
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

const AHARDWAREBUFFER_FORMAT_R8G8B8A8_UNORM: u32 = 1;
const AHARDWAREBUFFER_USAGE_GPU_SAMPLED_IMAGE: u64 = 1 << 8;
const AHARDWAREBUFFER_USAGE_GPU_COLOR_OUTPUT: u64 = 1 << 9;

#[repr(C)]
struct AHardwareBufferDesc {
    width: u32,
    height: u32,
    layers: u32,
    format: u32,
    usage: u64,
    stride: u32,
    rfu0: u32,
    rfu1: u64,
}

#[link(name = "android")]
unsafe extern "C" {
    fn AHardwareBuffer_allocate(desc: *const AHardwareBufferDesc, out: *mut *mut c_void) -> i32;
    fn AHardwareBuffer_release(buffer: *mut c_void);
}

#[link(name = "EGL")]
unsafe extern "C" {
    fn eglGetDisplay(id: *mut c_void) -> *mut c_void;
    fn eglInitialize(display: *mut c_void, major: *mut i32, minor: *mut i32) -> u32;
    fn eglChooseConfig(
        display: *mut c_void,
        attributes: *const i32,
        configs: *mut *mut c_void,
        size: i32,
        count: *mut i32,
    ) -> u32;
    fn eglCreateContext(
        display: *mut c_void,
        config: *mut c_void,
        share: *mut c_void,
        attributes: *const i32,
    ) -> *mut c_void;
    fn eglCreatePbufferSurface(
        display: *mut c_void,
        config: *mut c_void,
        attributes: *const i32,
    ) -> *mut c_void;
    fn eglMakeCurrent(
        display: *mut c_void,
        draw: *mut c_void,
        read: *mut c_void,
        context: *mut c_void,
    ) -> u32;
    fn eglGetCurrentContext() -> *mut c_void;
    fn eglGetCurrentDisplay() -> *mut c_void;
    fn eglGetCurrentSurface(which: i32) -> *mut c_void;
    fn eglGetProcAddress(name: *const std::ffi::c_char) -> *mut c_void;
    fn eglGetError() -> i32;
}

#[link(name = "GLESv3")]
unsafe extern "C" {
    fn glGenTextures(count: i32, names: *mut u32);
    fn glDeleteTextures(count: i32, names: *const u32);
    fn glBindTexture(target: u32, name: u32);
    fn glTexParameteri(target: u32, name: u32, value: i32);
    fn glGenFramebuffers(count: i32, names: *mut u32);
    fn glDeleteFramebuffers(count: i32, names: *const u32);
    fn glBindFramebuffer(target: u32, name: u32);
    fn glFramebufferTexture2D(target: u32, attachment: u32, kind: u32, texture: u32, level: i32);
    fn glCheckFramebufferStatus(target: u32) -> u32;
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
    fn glDrawArrays(mode: u32, first: i32, count: i32);
    fn glFinish();
}
