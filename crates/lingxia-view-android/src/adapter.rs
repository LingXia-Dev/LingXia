//! `PlatformAdapter` over a Java `com.lingxia.view.GpuPlatform`. Calls made on
//! the main thread run Android mechanics; only `post` is called from the worker.
use jni::objects::{Global, JObject, JString};
use jni::{Env, JValue, JavaVM, jni_sig, jni_str};
use lingxia_view::DisplayList;
use lingxia_view::interaction::view::{AccessibilityEffect, Dispatch, Effect};
use lingxia_view::platform::*;
use lingxia_view::semantics::projection::Event;
use lingxia_view::session::host::{
    Failure, FrameSnapshot, FrameTiming, RecoveryAction, RecoveryMessage, WeakViewHost,
};
use lingxia_view::session::lifecycle::{CommitTicket, ConnectionTicket, SurfaceTicket};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Loads image bytes for the page an owner token names. The embedding host
/// installs it; image bytes never cross JNI.
pub trait ImageLoader: Send + Sync + 'static {
    /// `done` runs once with the bytes, unless the returned handle is dropped first.
    fn load(
        &self,
        owner: i64,
        source: String,
        done: Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>,
    ) -> Box<dyn Send>;
}

static IMAGE_LOADER: OnceLock<Arc<dyn ImageLoader>> = OnceLock::new();

/// Install the process image loader; the first call wins.
pub fn set_image_loader(loader: Arc<dyn ImageLoader>) {
    let _ = IMAGE_LOADER.set(loader);
}

/// Where this page's GPU target stands; see [`route`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum Target {
    /// No presenter yet, or an attach is in flight.
    Waiting = 0,
    Ready = 1,
    /// Attach failed; frames use the CPU reference renderer.
    Unavailable = 2,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Route {
    Gpu(u64),
    Cpu,
    /// Not ready to take a frame; the host prepares again when the target is.
    Wait,
}

/// A page whose GPU attach is pending must not rasterize on the CPU: that work
/// shares the View worker and would hold up its own attach and other pages.
pub(super) fn route(gpu_built: bool, surface: u64, target: Target) -> Route {
    match (gpu_built, surface, target) {
        (false, _, _) | (true, 0, Target::Unavailable) => Route::Cpu,
        (true, 0, _) => Route::Wait,
        (true, surface, _) => Route::Gpu(surface),
    }
}

pub(super) struct AndroidAdapter {
    platform: Global<JObject<'static>>,
    /// The attached GPU presenter, or 0.
    pub(super) gpu: AtomicU64,
    target: AtomicU8,
    started: Instant,
    /// Names this page to the image loader; 0 has no image source.
    owner: i64,
    pub(super) host: OnceLock<WeakViewHost<AndroidAdapter>>,
    /// In-flight loads; dropping a handle cancels its transport.
    images: Mutex<HashMap<u64, Box<dyn Send>>>,
    /// CPU frames waiting for the raster pool.
    raster: OnceLock<Arc<lingxia_view::renderer::pool::Mailbox<(DisplayList, CommitTicket)>>>,
    /// UI-thread time of the last frame handoff to Java, for the present log.
    reconcile_us: AtomicU64,
    /// A dialog of the top layer is open: the page sits above the host's bars.
    modal: std::sync::atomic::AtomicBool,
}

pub(super) enum Frame {
    /// Drawn on the render thread while the worker goes on; presented there too.
    #[cfg(all(feature = "gpu", target_os = "android"))]
    Gpu {
        surface: u64,
        revision: u64,
        drawn: Arc<super::render::Drawn>,
    },
    /// Rasterized off the View worker when presented.
    Cpu(DisplayList, CommitTicket),
}

/// Android log priorities: 3 debug, 4 info, 5 warn.
fn write(priority: i32, message: &str) {
    #[cfg(target_os = "android")]
    {
        unsafe extern "C" {
            fn __android_log_write(priority: i32, tag: *const u8, text: *const u8) -> i32;
        }
        if let Ok(text) = std::ffi::CString::new(message) {
            unsafe {
                __android_log_write(
                    priority,
                    c"LingXia.UI".as_ptr().cast(),
                    text.as_ptr().cast(),
                )
            };
        }
    }
    #[cfg(not(target_os = "android"))]
    if priority >= 5 {
        eprintln!("LingXia.UI: {message}");
    }
}

pub(super) fn log(message: &str) {
    write(5, message);
}

pub(super) fn info(message: &str) {
    write(4, message);
}

impl AndroidAdapter {
    pub(super) fn new(platform: Global<JObject<'static>>, owner: i64) -> Self {
        Self {
            platform,
            gpu: AtomicU64::new(0),
            target: AtomicU8::new(Target::Waiting as u8),
            started: Instant::now(),
            owner,
            host: OnceLock::new(),
            images: Mutex::new(HashMap::new()),
            raster: OnceLock::new(),
            reconcile_us: AtomicU64::new(0),
            modal: Default::default(),
        }
    }

    #[cfg_attr(not(all(feature = "gpu", target_os = "android")), allow(dead_code))]
    pub(super) fn set_target(&self, target: Target) {
        self.target.store(target as u8, Ordering::Release);
    }

    fn target(&self) -> Target {
        match self.target.load(Ordering::Acquire) {
            1 => Target::Ready,
            2 => Target::Unavailable,
            _ => Target::Waiting,
        }
    }

    fn images(&self) -> std::sync::MutexGuard<'_, HashMap<u64, Box<dyn Send>>> {
        self.images.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Run `work` with this thread's JNI environment; a thrown Java exception
    /// is logged and cleared.
    pub(super) fn java<T: Default>(
        &self,
        work: impl FnOnce(&mut Env, &JObject) -> jni::errors::Result<T>,
    ) -> T {
        let result = JavaVM::singleton().and_then(|vm| {
            vm.attach_current_thread(|env| -> jni::errors::Result<T> {
                let platform = env.new_local_ref(&self.platform)?;
                work(env, &platform)
            })
        });
        result.unwrap_or_else(|error| {
            log(&format!("GpuPlatform call failed: {error}"));
            T::default()
        })
    }

    fn void(&self, name: &'static jni::strings::JNIStr) {
        self.java(|env, platform| {
            env.call_method(platform, name, jni_sig!("()V"), &[])?;
            Ok(())
        })
    }

    fn int(&self, name: &'static jni::strings::JNIStr, value: i32) {
        self.java(|env, platform| {
            env.call_method(platform, name, jni_sig!("(I)V"), &[JValue::Int(value)])?;
            Ok(())
        })
    }

    fn flag(&self, name: &'static jni::strings::JNIStr, value: bool) {
        self.java(|env, platform| {
            env.call_method(platform, name, jni_sig!("(Z)V"), &[JValue::Bool(value)])?;
            Ok(())
        })
    }
}

fn node(index: usize) -> i32 {
    i32::try_from(index).unwrap_or(-1)
}

impl Presenter for AndroidAdapter {
    type Frame = Frame;

    fn prepare(
        &self,
        display: DisplayList,
        _: SurfaceTicket,
        generation: CommitTicket,
    ) -> Result<Option<Frame>, String> {
        let gpu_built = cfg!(all(feature = "gpu", target_os = "android"));
        match route(gpu_built, self.gpu.load(Ordering::Acquire), self.target()) {
            Route::Wait => return Ok(None),
            #[cfg(all(feature = "gpu", target_os = "android"))]
            Route::Gpu(surface) => {
                return Ok(Some(Frame::Gpu {
                    surface,
                    revision: generation.get(),
                    drawn: super::render::draw(surface, generation.get(), display),
                }));
            }
            #[cfg(not(all(feature = "gpu", target_os = "android")))]
            Route::Gpu(_) => return Ok(None),
            Route::Cpu => {}
        }
        Ok(Some(Frame::Cpu(display, generation)))
    }

    fn metrics(&self, frame: &Frame) -> Option<serde_json::Value> {
        Some(match frame {
            #[cfg(all(feature = "gpu", target_os = "android"))]
            Frame::Gpu { .. } => {
                serde_json::json!({"renderer": "sparse-strips", "renderThread": true})
            }
            Frame::Cpu(..) => serde_json::json!({"backend": "vello-cpu"}),
        })
    }

    fn present(&self, frame: Frame) -> Result<bool, String> {
        match frame {
            #[cfg(all(feature = "gpu", target_os = "android"))]
            Frame::Gpu {
                surface,
                revision,
                drawn,
            } => {
                let Some(host) = self.host.get().cloned() else {
                    return Ok(false);
                };
                super::render::present(surface, revision, drawn, move |shown, metrics| {
                    let Some(host) = host.upgrade() else { return };
                    let us = |path: &[&str]| {
                        path.iter()
                            .fold(metrics.as_ref(), |value, key| value?.get(key))
                            .and_then(serde_json::Value::as_u64)
                            .unwrap_or(0)
                    };
                    let gpu = [
                        us(&["prepare", "sceneUs"]),
                        us(&["prepare", "encodeUs"]),
                        us(&["prepare", "submitUs"]),
                        us(&["presentUs"]),
                    ];
                    match shown {
                        Ok(shown) => host.deferred_drawn(CommitTicket::new(revision), shown, gpu),
                        Err(error) => {
                            host.deferred_presented(CommitTicket::new(revision), false);
                            host.fail(error);
                        }
                    }
                });
                Ok(true)
            }
            Frame::Cpu(display, generation) => {
                let Some(host) = self.host.get().cloned() else {
                    return Ok(false);
                };
                self.raster
                    .get_or_init(|| {
                        lingxia_view::renderer::pool::Mailbox::new(move |(display, generation)| {
                            rasterize(&host, display, generation)
                        })
                    })
                    .submit((display, generation));
                Ok(true)
            }
        }
    }

    fn deferred(&self, _: &Frame) -> bool {
        true
    }

    fn release_transient(&self) {
        #[cfg(all(feature = "gpu", target_os = "android"))]
        {
            let surface = self.gpu.load(Ordering::Acquire);
            super::render::run(move || super::gpu_surface::release_transient(surface));
        }
    }
}

/// Raster thread. Every frame is reported once, shown or dropped; the host re-prepares
/// a dropped one.
fn rasterize(host: &WeakViewHost<AndroidAdapter>, display: DisplayList, generation: CommitTicket) {
    let Some(host) = host.upgrade() else {
        return;
    };
    let started = Instant::now();
    let frame = match display.rasterize() {
        Ok(frame) => frame,
        Err(error) => {
            host.deferred_presented(generation, false);
            return host.fail(error);
        }
    };
    let raster_us = started.elapsed().as_micros();
    let started = Instant::now();
    let (width, height) = (frame.pixels.width(), frame.pixels.height());
    let mut pixels = frame.pixels;
    let bytes = pixels.data_as_u8_slice_mut();
    // Java copies the premultiplied RGBA bytes into its kept bitmap during the call.
    let presented = host.adapter().java(|env, platform| {
        // SAFETY: `bytes` outlives the call, and Java reads the buffer only inside it.
        let buffer = unsafe { env.new_direct_byte_buffer(bytes.as_mut_ptr(), bytes.len())? };
        env.call_method(
            platform,
            jni_str!("presentPixels"),
            jni_sig!("(Ljava/nio/ByteBuffer;II)Z"),
            &[
                JValue::Object(&buffer),
                JValue::Int(i32::from(width)),
                JValue::Int(i32::from(height)),
            ],
        )?
        .z()
    });
    info(&format!(
        "CPU_PRESENT generation={} rasterUs={raster_us} presentUs={} shown={presented}",
        generation.get(),
        started.elapsed().as_micros()
    ));
    host.deferred_presented(generation, presented);
}

impl FrameScheduler for AndroidAdapter {
    fn request_vsync(&self) {
        self.void(jni_str!("requestVsync"));
    }
}

impl MainThread for AndroidAdapter {
    fn post(&self, task: Task) {
        self.post_after(Duration::ZERO, task);
    }
    fn post_after(&self, delay: Duration, task: Task) {
        let task = Box::into_raw(Box::new(task)) as i64;
        let delay = i64::try_from(delay.as_millis()).unwrap_or(i64::MAX);
        let posted = self.java(|env, platform| {
            env.call_method(
                platform,
                jni_str!("post"),
                jni_sig!("(JJ)V"),
                &[JValue::Long(task), JValue::Long(delay)],
            )?;
            Ok(true)
        });
        if !posted {
            // SAFETY: Java never received the pointer.
            drop(unsafe { Box::from_raw(task as *mut Task) });
        }
    }
}

impl EnvironmentSource for AndroidAdapter {
    fn environment(&self) -> Option<FrameEnvironment> {
        let encoded = self.java(|env, platform| {
            let value = env
                .call_method(
                    platform,
                    jni_str!("environment"),
                    jni_sig!("()Ljava/lang/String;"),
                    &[],
                )?
                .l()?;
            if value.is_null() {
                return Ok(None);
            }
            Ok(Some(env.cast_local::<JString>(value)?.try_to_string(env)?))
        })?;
        let facts: serde_json::Value = serde_json::from_str(&encoded)
            .map_err(|error| log(&format!("UI_ENVIRONMENT: {error}")))
            .ok()?;
        let number = |key: &str| facts[key].as_f64().unwrap_or(0.0) as f32;
        let edges = |key: &str| {
            let mut edges = [0.0; 4];
            for (edge, value) in edges.iter_mut().zip(facts[key].as_array()?) {
                *edge = value.as_f64()? as f32;
            }
            Some(edges)
        };
        let text_scale = facts["text_scale"].as_array().map(|rows| {
            rows.iter()
                .filter_map(|row| Some([row[0].as_f64()? as f32, row[1].as_f64()? as f32]))
                .collect()
        });
        DeviceFacts {
            width: u32::try_from(facts["width"].as_u64()?).ok()?,
            height: u32::try_from(facts["height"].as_u64()?).ok()?,
            scale: number("scale"),
            os: "android".into(),
            dark: facts["dark"].as_bool().unwrap_or(false),
            system_insets: edges("system_insets")?,
            chrome_insets: edges("chrome_insets")?,
            keyboard: number("keyboard"),
            smallest_width: number("smallest_width"),
            locale: facts["locale"].as_str().unwrap_or_default().to_owned(),
            fine_pointer: facts["fine_pointer"].as_bool().unwrap_or(false),
            animation_scale: facts["animation_scale"].as_f64().map_or(1.0, |v| v as f32),
            reduce_motion: facts["reduce_motion"].as_bool().unwrap_or(false),
            high_contrast: facts["high_contrast"].as_bool().unwrap_or(false),
            text_scale,
            hour_cycle: facts["hour_cycle"].as_str().map(str::to_owned),
            first_weekday: facts["first_weekday"]
                .as_u64()
                .and_then(|day| u8::try_from(day).ok()),
        }
        .environment()
    }
}

impl ImageProvider for AndroidAdapter {
    fn fetch(&self, id: u64, source: &str) {
        let Some(host) = self.host.get().cloned() else {
            return;
        };
        let Some(loader) = IMAGE_LOADER.get().filter(|_| self.owner != 0) else {
            if let Some(host) = host.upgrade() {
                host.image_loaded(id, Err("UI_IMAGE: no image source for this page".into()));
            }
            return;
        };
        let finished = Arc::new(AtomicBool::new(false));
        let done = {
            let finished = finished.clone();
            Box::new(move |result: Result<Vec<u8>, String>| {
                finished.store(true, Ordering::Release);
                if let Some(host) = host.upgrade() {
                    host.adapter().images().remove(&id);
                    host.image_loaded(id, result);
                }
            })
        };
        let handle = loader.load(self.owner, source.into(), done);
        // A loader may finish synchronously; then there is nothing left to cancel.
        if !finished.load(Ordering::Acquire) {
            self.images().insert(id, handle);
        }
    }
    fn cancel(&self, id: u64) {
        let handle = self.images().remove(&id);
        drop(handle);
    }
}

/// `GpuPlatform.announce` event codes.
fn event_code(event: &AccessibilityEffect) -> Option<(i32, u32)> {
    Some(match event {
        AccessibilityEffect::Focused { id } => (0, *id),
        AccessibilityEffect::FocusCleared { id } => (1, *id),
        AccessibilityEffect::InputFocused { id } => (2, *id),
        AccessibilityEffect::HoverEnter { id } => (3, *id),
        AccessibilityEffect::HoverExit { id } => (4, *id),
        AccessibilityEffect::Change(Event::ValueChanged { id }) => (5, *id),
        AccessibilityEffect::Change(Event::ContentChanged) => (6, u32::MAX),
        AccessibilityEffect::Change(_) => return None,
    })
}

impl AccessibilitySink for AndroidAdapter {
    fn announce(&self, events: &[AccessibilityEffect]) {
        for event in events {
            self.java(|env, platform| {
                if let Some((code, id)) = event_code(event) {
                    env.call_method(
                        platform,
                        jni_str!("announce"),
                        jni_sig!("(II)V"),
                        &[JValue::Int(code), JValue::Int(id as i32)],
                    )?;
                    return Ok(());
                }
                match event {
                    AccessibilityEffect::Change(Event::TextChanged {
                        id,
                        before,
                        after,
                        secure,
                    }) => {
                        let (before, after) = (env.new_string(before)?, env.new_string(after)?);
                        env.call_method(
                            platform,
                            jni_str!("announceText"),
                            jni_sig!("(IZLjava/lang/String;Ljava/lang/String;)V"),
                            &[
                                JValue::Int(*id as i32),
                                JValue::Bool(*secure),
                                JValue::Object(&before),
                                JValue::Object(&after),
                            ],
                        )?;
                    }
                    AccessibilityEffect::Change(Event::SelectionChanged {
                        id,
                        text,
                        start,
                        end,
                        secure,
                    }) => {
                        let text = env.new_string(text)?;
                        env.call_method(
                            platform,
                            jni_str!("announceSelection"),
                            jni_sig!("(IZLjava/lang/String;II)V"),
                            &[
                                JValue::Int(*id as i32),
                                JValue::Bool(*secure),
                                JValue::Object(&text),
                                JValue::Int(*start as i32),
                                JValue::Int(*end as i32),
                            ],
                        )?;
                    }
                    AccessibilityEffect::Change(Event::Announce {
                        id,
                        text,
                        assertive,
                    }) => {
                        let text = env.new_string(text)?;
                        env.call_method(
                            platform,
                            jni_str!("announceLive"),
                            jni_sig!("(ILjava/lang/String;Z)V"),
                            &[
                                JValue::Int(*id as i32),
                                JValue::Object(&text),
                                JValue::Bool(*assertive),
                            ],
                        )?;
                    }
                    _ => {}
                }
                Ok(())
            });
        }
    }
    fn clicked(&self) {
        self.void(jni_str!("announceClicked"));
    }
}

impl InputMethod for AndroidAdapter {
    fn reconcile(&self, frame: &FrameSnapshot) {
        let started = Instant::now();
        self.java(|env, platform| {
            let snapshot = env.new_string(&frame.json)?;
            env.call_method(
                platform,
                jni_str!("frame"),
                jni_sig!("(Ljava/lang/String;F)V"),
                &[JValue::Object(&snapshot), JValue::Float(frame.scale)],
            )?;
            Ok(())
        });
        self.reconcile_us
            .store(started.elapsed().as_micros() as u64, Ordering::Relaxed);
    }
    fn focus(&self, editor: &Dispatch, _: &FrameSnapshot) {
        let Dispatch::Editor {
            node: index,
            commit,
            point,
            word,
        } = editor
        else {
            return;
        };
        let [x, y] = point.unwrap_or([f32::NAN; 2]);
        self.java(|env, platform| {
            env.call_method(
                platform,
                jni_str!("imeFocus"),
                jni_sig!("(IJZFFZ)V"),
                &[
                    JValue::Int(node(*index)),
                    JValue::Long(commit.get() as i64),
                    JValue::Bool(point.is_some()),
                    JValue::Float(x),
                    JValue::Float(y),
                    JValue::Bool(*word),
                ],
            )?;
            Ok(())
        })
    }
    fn activation(&self) {
        self.void(jni_str!("imeActivation"));
    }
    fn reset(&self) {
        self.void(jni_str!("imeReset"));
    }
    fn edit_text(&self, index: usize, text: Option<&str>, start: i32, end: i32) -> EditOutcome {
        let code = self.java(|env, platform| {
            let text = match text {
                Some(text) => JObject::from(env.new_string(text)?),
                None => JObject::null(),
            };
            env.call_method(
                platform,
                jni_str!("editText"),
                jni_sig!("(ILjava/lang/String;II)I"),
                &[
                    JValue::Int(node(index)),
                    JValue::Object(&text),
                    JValue::Int(start),
                    JValue::Int(end),
                ],
            )?
            .i()
            .map(Some)
        });
        // `GpuPlatform.editText`: 0 applied, 1 not ready, anything else failed.
        match code {
            Some(0) => EditOutcome::Applied,
            Some(1) => EditOutcome::NotReady,
            _ => EditOutcome::Failed,
        }
    }
}

impl Feedback for AndroidAdapter {
    fn apply(&self, effects: &[Effect]) {
        for effect in effects {
            match effect {
                Effect::Press { node: index, .. } => self.int(jni_str!("press"), node(*index)),
                Effect::ClearPress => self.void(jni_str!("clearPress")),
                Effect::FocusRing { node: index } => self.int(jni_str!("focusRing"), node(*index)),
                Effect::ClearFocusRing => self.void(jni_str!("clearFocusRing")),
                Effect::Claim { exclusive } => self.flag(jni_str!("claim"), *exclusive),
                Effect::ArmLongPress => self.void(jni_str!("armLongPress")),
                Effect::CancelLongPress => self.void(jni_str!("cancelLongPress")),
                Effect::Fling {
                    region,
                    horizontal,
                    velocity,
                } => self.java(|env, platform| {
                    let region = env.new_string(region)?;
                    env.call_method(
                        platform,
                        jni_str!("fling"),
                        jni_sig!("(Ljava/lang/String;ZF)V"),
                        &[
                            JValue::Object(&region),
                            JValue::Bool(*horizontal),
                            JValue::Float(*velocity),
                        ],
                    )?;
                    Ok(())
                }),
                Effect::StopFling => self.void(jni_str!("stopFling")),
                Effect::Clicked => self.void(jni_str!("clicked")),
                Effect::Haptic => self.void(jni_str!("haptic")),

                // The host runs these itself.
                Effect::Redraw
                | Effect::Dispatch
                | Effect::Number
                | Effect::Accessibility(_)
                | Effect::EditText { .. } => {}
            }
        }
    }
}

/// `GpuPlatform.recovery` codes; -1 clears.
fn recovery_codes(failure: &Failure) -> (i32, i32) {
    let message = match failure.message {
        RecoveryMessage::UpdateRequired => 0,
        RecoveryMessage::SessionFailed => 1,
        RecoveryMessage::UpdateFailed => 2,
        RecoveryMessage::DisplayFailed => 3,
    };
    let action = match failure.action {
        None => -1,
        Some(RecoveryAction::Retry) => 0,
        Some(RecoveryAction::Reload) => 1,
    };
    (message, action)
}

impl HostEvents for AndroidAdapter {
    fn loaded(&self) {
        self.void(jni_str!("loaded"));
    }
    fn presented(&self, frame: &FrameSnapshot, timing: &FrameTiming) {
        let started = Instant::now();
        // Borrow the metrics text instead of building the whole frame as a value.
        let metrics =
            serde_json::from_str::<HashMap<&str, &serde_json::value::RawValue>>(&frame.json)
                .ok()
                .and_then(|snapshot| snapshot.get("metrics").map(|raw| raw.get().to_owned()))
                .unwrap_or_default();
        let metrics_us = started.elapsed().as_micros();
        // An exiting dialog still covers the bars until its exit ends.
        let open = frame.frame.nodes.iter().any(|node| {
            node.role.as_deref() == Some("dialog")
                && lingxia_view::interaction::frame::visible(node)
        });
        let modal = open || (self.modal.load(Ordering::Relaxed) && frame.metrics.ghosts > 0);
        if self.modal.swap(modal, Ordering::Relaxed) != modal {
            self.flag(jni_str!("topLayer"), modal);
        }
        let started = Instant::now();
        self.void(jni_str!("presented"));
        info(&format!(
            "present generation={} surface={} size={}x{} workerMs={} workerQueueUs={} uiWaitUs={} uiUs={} reconcileUs={} metricsUs={metrics_us} presentedUs={} rebuild={} caretVisible={} nodes={} images={} metrics={metrics}",
            frame.generation.get(),
            self.gpu.load(Ordering::Acquire),
            frame.width,
            frame.height,
            timing.worker_us / 1000,
            timing.queue_us,
            timing.wait_us,
            timing.main_us,
            self.reconcile_us.load(Ordering::Relaxed),
            started.elapsed().as_micros(),
            timing.rebuild,
            timing.caret_visible,
            frame.frame.nodes.len(),
            frame.loaded_images,
        ));
    }
    fn message(&self, connection: ConnectionTicket, message: &str) {
        self.java(|env, platform| {
            let message = env.new_string(message)?;
            env.call_method(
                platform,
                jni_str!("message"),
                jni_sig!("(JLjava/lang/String;)V"),
                &[
                    JValue::Long(connection.get() as i64),
                    JValue::Object(&message),
                ],
            )?;
            Ok(())
        })
    }
    fn restart(&self, connection: ConnectionTicket) -> bool {
        self.java(|env, platform| {
            env.call_method(
                platform,
                jni_str!("restart"),
                jni_sig!("(J)Z"),
                &[JValue::Long(connection.get() as i64)],
            )?
            .z()
        })
    }
    fn recovery(&self, failure: Option<&Failure>) {
        let (message, action) = failure.map_or((-1, -1), recovery_codes);
        let retained = failure.is_some_and(|f| !f.covers_page());
        let error = failure.map(|f| f.error.as_str()).unwrap_or_default();
        self.java(|env, platform| {
            let error = env.new_string(error)?;
            env.call_method(
                platform,
                jni_str!("recovery"),
                jni_sig!("(IIZLjava/lang/String;)V"),
                &[
                    JValue::Int(message),
                    JValue::Int(action),
                    JValue::Bool(retained),
                    JValue::Object(&error),
                ],
            )?;
            Ok(())
        })
    }
    fn bootstrap_overdue(&self, overdue: bool) {
        self.flag(jni_str!("bootstrapOverdue"), overdue);
    }
    fn text_menu(&self, menu: Option<&lingxia_view::session::host::TextMenu>) {
        let encoded = menu.and_then(|menu| serde_json::to_string(menu).ok());
        self.java(|env, platform| {
            let menu = match &encoded {
                Some(menu) => JObject::from(env.new_string(menu)?),
                None => JObject::null(),
            };
            env.call_method(
                platform,
                jni_str!("textMenu"),
                jni_sig!("(Ljava/lang/String;)V"),
                &[JValue::Object(&menu)],
            )?;
            Ok(())
        })
    }
    fn revealed(&self, hold_surface: bool) {
        self.flag(jni_str!("revealed"), hold_surface);
    }
    fn host_chrome(&self, chrome: &lingxia_view::platform::HostChrome) {
        let argb = |[r, g, b, a]: [u8; 4]| JValue::Int(i32::from_be_bytes([a, r, g, b]));
        self.java(|env, platform| {
            env.call_method(
                platform,
                jni_str!("hostChrome"),
                jni_sig!("(IIIIIIZ)V"),
                &[
                    argb(chrome.canvas),
                    argb(chrome.bar),
                    argb(chrome.bar_line),
                    argb(chrome.content),
                    argb(chrome.content_muted),
                    argb(chrome.accent),
                    JValue::Bool(chrome.dark),
                ],
            )?;
            Ok(())
        })
    }
    fn copy_text(&self, text: &str) {
        self.java(|env, platform| {
            let text = env.new_string(text)?;
            env.call_method(
                platform,
                jni_str!("copyText"),
                jni_sig!("(Ljava/lang/String;)V"),
                &[JValue::Object(&text)],
            )?;
            Ok(())
        })
    }
    fn warn(&self, message: &str) {
        log(message);
    }

    fn debug(&self, message: &str) {
        write(3, message);
    }
}

impl Clock for AndroidAdapter {
    fn now(&self) -> Duration {
        self.started.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_gpu_targets_wait_instead_of_rasterizing() {
        assert_eq!(route(true, 0, Target::Waiting), Route::Wait);
        assert_eq!(route(true, 0, Target::Ready), Route::Wait);
        assert_eq!(
            route(true, 7, Target::Waiting),
            Route::Gpu(7),
            "re-attach keeps its presenter"
        );
        assert_eq!(route(true, 7, Target::Ready), Route::Gpu(7));
        assert_eq!(route(true, 0, Target::Unavailable), Route::Cpu);
        assert_eq!(
            route(false, 0, Target::Waiting),
            Route::Cpu,
            "CPU-only builds never wait"
        );
    }
}
