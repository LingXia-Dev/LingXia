//! `PlatformAdapter` over a Java `com.lingxia.ui.GpuPlatform`. Calls made on
//! the main thread run Android mechanics; only `post` is called from the worker.
use crate::DisplayList;
use crate::interaction::view::{AccessibilityEffect, Dispatch, Effect};
use crate::platform::*;
use crate::semantics::projection::Event;
use crate::session::host::{Failure, FrameSnapshot, FrameTiming, RecoveryAction, RecoveryMessage};
use crate::session::lifecycle::{CommitTicket, ConnectionTicket, SurfaceTicket};
use jni::objects::{Global, JObject, JString};
use jni::{Env, JValue, JavaVM, jni_sig, jni_str};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub(super) struct AndroidAdapter {
    platform: Global<JObject<'static>>,
    /// The attached GPU presenter; 0 renders on the CPU.
    pub(super) gpu: AtomicU64,
    started: Instant,
}

pub(super) enum Frame {
    #[cfg_attr(not(all(feature = "gpu", target_os = "android")), allow(dead_code))]
    Gpu {
        surface: u64,
        revision: u64,
        metrics: serde_json::Value,
    },
    Cpu {
        pixels: Vec<i32>,
        width: i32,
        height: i32,
    },
}

pub(super) fn log(message: &str) {
    #[cfg(target_os = "android")]
    {
        unsafe extern "C" {
            fn __android_log_write(priority: i32, tag: *const u8, text: *const u8) -> i32;
        }
        if let Ok(text) = std::ffi::CString::new(message) {
            // ANDROID_LOG_WARN
            unsafe { __android_log_write(5, c"LingXia.UI".as_ptr().cast(), text.as_ptr().cast()) };
        }
    }
    #[cfg(not(target_os = "android"))]
    eprintln!("LingXia.UI: {message}");
}

impl AndroidAdapter {
    pub(super) fn new(platform: Global<JObject<'static>>) -> Self {
        Self {
            platform,
            gpu: AtomicU64::new(0),
            started: Instant::now(),
        }
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
        #[cfg(all(feature = "gpu", target_os = "android"))]
        {
            let surface = self.gpu.load(Ordering::Acquire);
            if surface != 0 {
                return Ok(
                    super::gpu_surface::prepare(surface, &display, generation.get())?.map(
                        |metrics| Frame::Gpu {
                            surface,
                            revision: generation.get(),
                            metrics,
                        },
                    ),
                );
            }
        }
        let _ = generation;
        let frame = display.rasterize()?;
        let (width, height) = (frame.pixels.width(), frame.pixels.height());
        // The opaque root background makes these premultiplied channels also valid ARGB colors.
        let pixels = frame
            .pixels
            .data_as_u8_slice()
            .chunks_exact(4)
            .map(|p| u32::from_be_bytes([p[3], p[0], p[1], p[2]]) as i32)
            .collect();
        Ok(Some(Frame::Cpu {
            pixels,
            width: i32::from(width),
            height: i32::from(height),
        }))
    }

    fn metrics(&self, frame: &Frame) -> Option<serde_json::Value> {
        Some(match frame {
            Frame::Gpu { metrics, .. } => metrics.clone(),
            Frame::Cpu { .. } => serde_json::json!({"backend": "vello-cpu"}),
        })
    }

    fn present(&self, frame: Frame) -> Result<bool, String> {
        match frame {
            #[cfg(all(feature = "gpu", target_os = "android"))]
            Frame::Gpu {
                surface, revision, ..
            } => {
                let started = Instant::now();
                let Some([acquire, encode, submit, present]) =
                    super::gpu_surface::present(surface, revision)?
                else {
                    return Ok(false);
                };
                // Diagnose host-thread stalls separately from the worker's render time.
                info(&format!(
                    "GPU_PRESENT surface={surface} generation={revision} lockUs={} acquireUs={acquire} encodeUs={encode} submitUs={submit} presentUs={present}",
                    started.elapsed().as_micros()
                ));
                Ok(true)
            }
            #[cfg(not(all(feature = "gpu", target_os = "android")))]
            Frame::Gpu { .. } => Ok(false),
            Frame::Cpu {
                pixels,
                width,
                height,
            } => Ok(self.java(|env, platform| {
                let array = env.new_int_array(pixels.len())?;
                array.set_region(env, 0, &pixels)?;
                env.call_method(
                    platform,
                    jni_str!("presentPixels"),
                    jni_sig!("([III)Z"),
                    &[
                        JValue::Object(&array),
                        JValue::Int(width),
                        JValue::Int(height),
                    ],
                )?
                .z()
            })),
        }
    }
}

pub(super) fn info(message: &str) {
    #[cfg(target_os = "android")]
    {
        unsafe extern "C" {
            fn __android_log_write(priority: i32, tag: *const u8, text: *const u8) -> i32;
        }
        if let Ok(text) = std::ffi::CString::new(message) {
            // ANDROID_LOG_INFO
            unsafe { __android_log_write(4, c"LingXia.UI".as_ptr().cast(), text.as_ptr().cast()) };
        }
    }
    #[cfg(not(target_os = "android"))]
    let _ = message;
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
        let value: serde_json::Value = serde_json::from_str(&encoded)
            .map_err(|error| log(&format!("UI_ENVIRONMENT: {error}")))
            .ok()?;
        let mut env = value["env"].clone();
        env["renderer"] = serde_json::json!({
            "fallback": crate::renderer::sentinel::fallback().map(|_| "driver"),
        });
        Some(FrameEnvironment {
            width: u16::try_from(value["width"].as_u64()?).ok()?,
            height: u16::try_from(value["height"].as_u64()?).ok()?,
            scale: value["scale"].as_f64()? as f32,
            value: env,
        })
    }
}

impl ImageProvider for AndroidAdapter {
    fn fetch(&self, id: u64, source: &str) {
        self.java(|env, platform| {
            let source = env.new_string(source)?;
            env.call_method(
                platform,
                jni_str!("fetchImage"),
                jni_sig!("(JLjava/lang/String;)V"),
                &[JValue::Long(id as i64), JValue::Object(&source)],
            )?;
            Ok(())
        })
    }
    fn cancel(&self, id: u64) {
        self.java(|env, platform| {
            env.call_method(
                platform,
                jni_str!("cancelImage"),
                jni_sig!("(J)V"),
                &[JValue::Long(id as i64)],
            )?;
            Ok(())
        })
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
        self.java(|env, platform| {
            let snapshot = env.new_string(&frame.json)?;
            env.call_method(
                platform,
                jni_str!("frame"),
                jni_sig!("(Ljava/lang/String;F)V"),
                &[JValue::Object(&snapshot), JValue::Float(frame.scale)],
            )?;
            Ok(())
        })
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
                Effect::EditText {
                    node: index,
                    text,
                    start,
                    end,
                } => self.java(|env, platform| {
                    let text = match text {
                        Some(text) => JObject::from(env.new_string(text)?),
                        None => JObject::null(),
                    };
                    env.call_method(
                        platform,
                        jni_str!("editText"),
                        jni_sig!("(ILjava/lang/String;II)V"),
                        &[
                            JValue::Int(node(*index)),
                            JValue::Object(&text),
                            JValue::Int(*start),
                            JValue::Int(*end),
                        ],
                    )?;
                    Ok(())
                }),
                // The host runs these itself.
                Effect::Redraw | Effect::Dispatch | Effect::Number | Effect::Accessibility(_) => {}
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
        let metrics = serde_json::from_str::<serde_json::Value>(&frame.json)
            .map(|snapshot| snapshot["metrics"].to_string())
            .unwrap_or_default();
        info(&format!(
            "present generation={} surface={} size={}x{} workerMs={} workerQueueUs={} uiWaitUs={} uiUs={} rebuild={} caretVisible={} nodes={} images={} metrics={metrics}",
            frame.generation.get(),
            self.gpu.load(Ordering::Acquire),
            frame.width,
            frame.height,
            timing.worker_us / 1000,
            timing.queue_us,
            timing.wait_us,
            timing.main_us,
            timing.rebuild,
            timing.caret_visible,
            frame.frame.nodes.len(),
            frame.loaded_images,
        ));
        self.void(jni_str!("presented"));
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
        let retained = failure.is_some_and(|f| f.retained);
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
    fn warn(&self, message: &str) {
        log(message);
    }
}

impl Clock for AndroidAdapter {
    fn now(&self) -> Duration {
        self.started.elapsed()
    }
}
