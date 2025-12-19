use crate::error::PlatformError;
use std::ffi::{CString, c_char};
use std::ptr;
use std::slice;
use std::sync::{Mutex, OnceLock};

// Opaque C types from ohcamera C API
#[repr(C)]
struct Camera_Manager {
    _priv: [u8; 0],
}
#[repr(C)]
struct Camera_CaptureSession {
    _priv: [u8; 0],
}
#[repr(C)]
struct Camera_Input {
    _priv: [u8; 0],
}
#[repr(C)]
struct Camera_PreviewOutput {
    _priv: [u8; 0],
}
#[repr(C)]
struct Camera_PhotoOutput {
    _priv: [u8; 0],
}
#[repr(C)]
struct Camera_VideoOutput {
    _priv: [u8; 0],
}

#[repr(C)]
struct Camera_Device {
    camera_id: *mut c_char,
    camera_position: i32,
    camera_type: i32,
    connection_type: i32,
}
#[repr(C)]
#[derive(Copy, Clone)]
struct Camera_Size {
    width: u32,
    height: u32,
}
#[repr(C)]
#[derive(Copy, Clone)]
struct Camera_Profile {
    format: i32,
    size: Camera_Size,
}
#[repr(C)]
struct Camera_VideoProfile {
    format: i32,
    size: Camera_Size,
    range: Camera_FrameRateRange,
}
#[repr(C)]
#[derive(Copy, Clone)]
struct Camera_FrameRateRange {
    min: u32,
    max: u32,
}
#[repr(C)]
#[derive(Copy, Clone)]
struct Camera_OutputCapability {
    preview_profiles: *mut *mut Camera_Profile,
    preview_profiles_size: u32,
    photo_profiles: *mut *mut Camera_Profile,
    photo_profiles_size: u32,
    video_profiles: *mut *mut Camera_VideoProfile,
    video_profiles_size: u32,
    supported_metadata_object_types: *mut *mut Camera_MetadataObjectType,
    metadata_profiles_size: u32,
}
#[repr(C)]
struct Camera_MetadataObjectType {
    _priv: [u8; 0],
}

#[link(name = "ohcamera")]
unsafe extern "C" {
    fn OH_Camera_GetCameraManager(out: *mut *mut Camera_Manager) -> i32;
    fn OH_Camera_DeleteCameraManager(manager: *mut Camera_Manager) -> i32;
    fn OH_CameraManager_CreateCaptureSession(
        manager: *mut Camera_Manager,
        out: *mut *mut Camera_CaptureSession,
    ) -> i32;
    fn OH_CaptureSession_Release(session: *mut Camera_CaptureSession) -> i32;

    fn OH_CameraManager_GetSupportedCameras(
        manager: *mut Camera_Manager,
        cameras: *mut *mut Camera_Device,
        size: *mut u32,
    ) -> i32;
    fn OH_CameraManager_DeleteSupportedCameras(
        manager: *mut Camera_Manager,
        cameras: *mut Camera_Device,
        size: u32,
    ) -> i32;
    fn OH_CameraManager_GetSupportedCameraOutputCapability(
        manager: *mut Camera_Manager,
        camera: *const Camera_Device,
        cap: *mut *mut Camera_OutputCapability,
    ) -> i32;
    fn OH_CameraManager_DeleteSupportedCameraOutputCapability(
        manager: *mut Camera_Manager,
        cap: *mut Camera_OutputCapability,
    ) -> i32;

    fn OH_CameraManager_CreateCameraInput(
        manager: *mut Camera_Manager,
        device: *const Camera_Device,
        input: *mut *mut Camera_Input,
    ) -> i32;
    fn OH_CameraInput_Open(input: *mut Camera_Input) -> i32;
    fn OH_CameraInput_Close(input: *mut Camera_Input) -> i32;
    fn OH_CameraInput_Release(input: *mut Camera_Input) -> i32;

    fn OH_CameraManager_CreatePreviewOutput(
        manager: *mut Camera_Manager,
        profile: *const Camera_Profile,
        surface_id: *const c_char,
        output: *mut *mut Camera_PreviewOutput,
    ) -> i32;
    fn OH_CameraManager_CreatePreviewOutputUsedInPreconfig(
        manager: *mut Camera_Manager,
        surface_id: *const c_char,
        output: *mut *mut Camera_PreviewOutput,
    ) -> i32;
    fn OH_PreviewOutput_Stop(output: *mut Camera_PreviewOutput) -> i32;
    fn OH_PreviewOutput_Release(output: *mut Camera_PreviewOutput) -> i32;

    fn OH_CameraManager_CreatePhotoOutput(
        manager: *mut Camera_Manager,
        profile: *const Camera_Profile,
        surface_id: *const c_char,
        output: *mut *mut Camera_PhotoOutput,
    ) -> i32;
    fn OH_CameraManager_CreatePhotoOutputUsedInPreconfig(
        manager: *mut Camera_Manager,
        surface_id: *const c_char,
        output: *mut *mut Camera_PhotoOutput,
    ) -> i32;
    fn OH_PhotoOutput_Release(output: *mut Camera_PhotoOutput) -> i32;
    fn OH_PhotoOutput_Capture(output: *mut Camera_PhotoOutput) -> i32;

    fn OH_CameraManager_CreateVideoOutput(
        manager: *mut Camera_Manager,
        profile: *const Camera_VideoProfile,
        surface_id: *const c_char,
        output: *mut *mut Camera_VideoOutput,
    ) -> i32;
    fn OH_VideoOutput_Start(output: *mut Camera_VideoOutput) -> i32;
    fn OH_VideoOutput_Stop(output: *mut Camera_VideoOutput) -> i32;
    fn OH_VideoOutput_Release(output: *mut Camera_VideoOutput) -> i32;

    fn OH_CaptureSession_BeginConfig(session: *mut Camera_CaptureSession) -> i32;
    fn OH_CaptureSession_CommitConfig(session: *mut Camera_CaptureSession) -> i32;
    fn OH_CaptureSession_Start(session: *mut Camera_CaptureSession) -> i32;
    fn OH_CaptureSession_Stop(session: *mut Camera_CaptureSession) -> i32;
    fn OH_CaptureSession_AddInput(
        session: *mut Camera_CaptureSession,
        input: *mut Camera_Input,
    ) -> i32;
    fn OH_CaptureSession_AddPreviewOutput(
        session: *mut Camera_CaptureSession,
        output: *mut Camera_PreviewOutput,
    ) -> i32;
    fn OH_CaptureSession_RemovePreviewOutput(
        session: *mut Camera_CaptureSession,
        output: *mut Camera_PreviewOutput,
    ) -> i32;
    fn OH_CaptureSession_AddPhotoOutput(
        session: *mut Camera_CaptureSession,
        output: *mut Camera_PhotoOutput,
    ) -> i32;
    fn OH_CaptureSession_RemovePhotoOutput(
        session: *mut Camera_CaptureSession,
        output: *mut Camera_PhotoOutput,
    ) -> i32;
    fn OH_CaptureSession_AddVideoOutput(
        session: *mut Camera_CaptureSession,
        output: *mut Camera_VideoOutput,
    ) -> i32;
    fn OH_CaptureSession_RemoveVideoOutput(
        session: *mut Camera_CaptureSession,
        output: *mut Camera_VideoOutput,
    ) -> i32;

    // Flash mode control
    fn OH_CaptureSession_HasFlash(session: *mut Camera_CaptureSession, has_flash: *mut bool)
    -> i32;
    fn OH_CaptureSession_IsFlashModeSupported(
        session: *mut Camera_CaptureSession,
        flash_mode: u32,
        is_supported: *mut bool,
    ) -> i32;
    fn OH_CaptureSession_SetFlashMode(session: *mut Camera_CaptureSession, flash_mode: u32) -> i32;
}

// Flash mode constants
const FLASH_MODE_CLOSE: u32 = 0;
const FLASH_MODE_OPEN: u32 = 1;

struct CamState {
    manager: *mut Camera_Manager,
    session: *mut Camera_CaptureSession,
    camera_input: *mut Camera_Input,
    preview_output: *mut Camera_PreviewOutput,
    photo_output: *mut Camera_PhotoOutput,
    video_output: *mut Camera_VideoOutput,
    cameras: *mut Camera_Device,
    cameras_len: u32,
    capability: *mut Camera_OutputCapability,
    camera_index: u32,
    current_facing: String,
    preview_surface_id: String,
}

impl Default for CamState {
    fn default() -> Self {
        Self {
            manager: ptr::null_mut(),
            session: ptr::null_mut(),
            camera_input: ptr::null_mut(),
            preview_output: ptr::null_mut(),
            photo_output: ptr::null_mut(),
            video_output: ptr::null_mut(),
            cameras: ptr::null_mut(),
            cameras_len: 0,
            capability: ptr::null_mut(),
            camera_index: 0,
            current_facing: String::from("back"),
            preview_surface_id: String::new(),
        }
    }
}

// Manual implementation to avoid threading issues
impl CamState {
    fn new() -> Self {
        Default::default()
    }
}

// Make CamState thread-safe by containing only raw pointers
unsafe impl Send for CamState {}
unsafe impl Sync for CamState {}

static STATE: OnceLock<Mutex<CamState>> = OnceLock::new();

fn state() -> &'static Mutex<CamState> {
    STATE.get_or_init(|| Mutex::new(CamState::new()))
}

pub fn camera_init(_surface_id: &str, _facing: &str) -> Result<bool, PlatformError> {
    // Initialize manager and capture session and attach preview to provided surface
    let mut st = state().lock().unwrap();
    unsafe {
        cleanup_state(&mut st);
        let mut mgr: *mut Camera_Manager = ptr::null_mut();
        let rc1 = OH_Camera_GetCameraManager(&mut mgr as *mut *mut Camera_Manager);

        if rc1 != 0 || mgr.is_null() {
            log::error!(
                "[Harmony.Camera] OH_Camera_GetCameraManager failed: rc={}, mgr.is_null={}",
                rc1,
                mgr.is_null()
            );
            return Err(PlatformError::Platform(
                "OH_Camera_GetCameraManager failed".into(),
            ));
        }
        st.manager = mgr;

        // Setup camera devices/capability
        match setup_camera_base(&mut st) {
            Ok(_) => (),
            Err(e) => {
                log::error!("[Harmony.Camera] setup_camera_base failed: {:?}", e);
                return Err(e);
            }
        }

        // Choose camera index by facing hint (simple: 0=back, 1=front if available)
        st.current_facing = _facing.to_string();
        if _facing.eq_ignore_ascii_case("front") && st.cameras_len > 1 {
            st.camera_index = 1;
        } else {
            st.camera_index = 0;
        }

        // Create input
        let device = st.cameras.offset(st.camera_index as isize);
        let mut input: *mut Camera_Input = ptr::null_mut();
        let rci = OH_CameraManager_CreateCameraInput(st.manager, device, &mut input);
        if rci != 0 || input.is_null() {
            log::error!(
                "[Harmony.Camera] CreateCameraInput failed: rc={}, input.is_null={}",
                rci,
                input.is_null()
            );
            return Err(PlatformError::Platform("CreateCameraInput failed".into()));
        }
        st.camera_input = input;
        let rco = OH_CameraInput_Open(input);
        if rco != 0 {
            log::error!("[Harmony.Camera] CameraInput_Open failed: rc={}", rco);
            return Err(PlatformError::Platform("CameraInput_Open failed".into()));
        }

        // Create session
        let mut sess: *mut Camera_CaptureSession = ptr::null_mut();
        let rc2 = OH_CameraManager_CreateCaptureSession(
            mgr,
            &mut sess as *mut *mut Camera_CaptureSession,
        );
        // session created
        if rc2 != 0 || sess.is_null() {
            OH_CameraInput_Close(input);
            OH_CameraInput_Release(input);
            OH_Camera_DeleteCameraManager(mgr);
            return Err(PlatformError::Platform(
                "CreateCaptureSession failed".into(),
            ));
        }
        st.session = sess;

        // Begin config and add preview
        OH_CaptureSession_BeginConfig(sess);
        OH_CaptureSession_AddInput(sess, input);
        // Remember preview surface for future reconfigure
        st.preview_surface_id = _surface_id.to_string();

        let c_surface = CString::new(_surface_id)
            .map_err(|_| PlatformError::InvalidParameter("surfaceId null".into()))?;

        // Try preconfig path first
        let mut bound = false;
        let mut prev: *mut Camera_PreviewOutput = ptr::null_mut();
        let rc_pre = OH_CameraManager_CreatePreviewOutputUsedInPreconfig(
            st.manager,
            c_surface.as_ptr(),
            &mut prev,
        );
        if rc_pre == 0 && !prev.is_null() {
            let rc_add_prev = OH_CaptureSession_AddPreviewOutput(sess, prev);
            if rc_add_prev == 0 {
                bound = true;
            } else {
                OH_PreviewOutput_Release(prev);
                prev = ptr::null_mut();
            }
        }

        // Fallback: iterate profiles to find a compatible one
        if !bound {
            let caps = &*st.capability;
            if caps.preview_profiles_size > 0 && !caps.preview_profiles.is_null() {
                let profiles = slice::from_raw_parts(
                    caps.preview_profiles,
                    caps.preview_profiles_size as usize,
                );
                for &p in profiles.iter() {
                    if p.is_null() {
                        continue;
                    }
                    let mut po: *mut Camera_PreviewOutput = ptr::null_mut();
                    let rc = OH_CameraManager_CreatePreviewOutput(
                        st.manager,
                        p,
                        c_surface.as_ptr(),
                        &mut po,
                    );
                    if rc != 0 || po.is_null() {
                        continue;
                    }
                    let rc_add = OH_CaptureSession_AddPreviewOutput(sess, po);
                    if rc_add == 0 {
                        prev = po;
                        bound = true;
                        break;
                    } else {
                        OH_PreviewOutput_Release(po);
                    }
                }
            }
        }

        if !bound {
            return Err(PlatformError::Platform(
                "Failed to bind preview output to surface".into(),
            ));
        }

        st.preview_output = prev;

        // Commit and start session for preview
        let rc_commit = OH_CaptureSession_CommitConfig(sess);
        if rc_commit != 0 {
            log::error!("[Harmony.Camera] CommitConfig failed: {}", rc_commit);
            return Err(PlatformError::Platform("CommitConfig failed".into()));
        }

        let rc_start = OH_CaptureSession_Start(sess);
        if rc_start != 0 {
            log::error!("[Harmony.Camera] Start session failed: {}", rc_start);
            return Err(PlatformError::Platform("Start session failed".into()));
        }
    }
    Ok(true)
}

pub fn camera_release() {
    let mut st = state().lock().unwrap();
    unsafe { cleanup_state(&mut st) }
}

pub fn camera_switch_facing(is_back: bool) -> Result<bool, PlatformError> {
    // Re-init session with stored preview surface and new facing
    let (preview_surface, new_facing);
    {
        let mut st = state().lock().unwrap();
        new_facing = if is_back {
            "back".to_string()
        } else {
            "front".to_string()
        };
        st.current_facing = new_facing.clone();
        preview_surface = st.preview_surface_id.clone();
    }
    if preview_surface.is_empty() {
        return Ok(true);
    }
    camera_release();
    camera_init(&preview_surface, &new_facing).map(|_| true)
}

/// Set camera flash mode
/// flash_on: true to enable flash (FLASH_MODE_OPEN), false to disable (FLASH_MODE_CLOSE)
pub fn camera_set_flash_mode(flash_on: bool) -> Result<bool, PlatformError> {
    let st = state().lock().unwrap();
    unsafe {
        if st.session.is_null() {
            log::warn!("[Harmony.Camera] session is null, cannot set flash mode");
            return Ok(false);
        }

        // Check if flash is available
        let mut has_flash = false;
        let rc = OH_CaptureSession_HasFlash(st.session, &mut has_flash);
        if rc != 0 || !has_flash {
            log::info!(
                "[Harmony.Camera] Flash not available: rc={}, has_flash={}",
                rc,
                has_flash
            );
            return Ok(false);
        }

        // Determine target flash mode
        let target_mode = if flash_on {
            FLASH_MODE_OPEN
        } else {
            FLASH_MODE_CLOSE
        };

        // Check if mode is supported
        let mut is_supported = false;
        let rc = OH_CaptureSession_IsFlashModeSupported(st.session, target_mode, &mut is_supported);
        if rc != 0 || !is_supported {
            log::warn!(
                "[Harmony.Camera] Flash mode {} not supported: rc={}, is_supported={}",
                target_mode,
                rc,
                is_supported
            );
            return Ok(false);
        }

        // Set flash mode
        let rc = OH_CaptureSession_SetFlashMode(st.session, target_mode);
        if rc != 0 {
            log::error!("[Harmony.Camera] SetFlashMode failed: rc={}", rc);
            return Err(PlatformError::Platform(format!(
                "SetFlashMode failed: {}",
                rc
            )));
        }

        log::info!(
            "[Harmony.Camera] Flash mode set to {}",
            if flash_on { "ON" } else { "OFF" }
        );
    }
    Ok(true)
}

pub fn camera_take_photo() -> Result<(), PlatformError> {
    let st = state().lock().unwrap();

    unsafe {
        if st.photo_output.is_null() {
            log::error!("[Harmony.Camera] photo_output is null");
            return Err(PlatformError::Platform(
                "photo output not configured".into(),
            ));
        }

        if st.session.is_null() {
            log::error!("[Harmony.Camera] session is null");
            return Err(PlatformError::Platform(
                "capture session not initialized".into(),
            ));
        }

        let rc = OH_PhotoOutput_Capture(st.photo_output);
        if rc != 0 {
            // Camera error codes from HarmonyOS:
            // 0 = CAMERA_OK
            // 7400101 = CAMERA_INVALID_ARGUMENT
            // 7400102 = CAMERA_OPERATION_NOT_ALLOWED
            // 7400103 = CAMERA_SESSION_NOT_CONFIG
            // 7400104 = CAMERA_SESSION_NOT_RUNNING
            let error_msg = match rc {
                7400101 => "Invalid argument",
                7400102 => "Operation not allowed",
                7400103 => "Session not config",
                7400104 => "Session not running",
                _ => "Unknown error",
            };
            log::error!(
                "[Harmony.Camera] OH_PhotoOutput_Capture failed: {} ({})",
                rc,
                error_msg
            );
            return Err(PlatformError::Platform(format!(
                "OH_PhotoOutput_Capture failed: {} ({})",
                rc, error_msg
            )));
        }
    }
    Ok(())
}

pub fn camera_start_photo_with_surface(
    photo_surface_id: &str,
    _callback_id: &str,
    _cache_dir: &str,
) -> Result<bool, PlatformError> {
    let mut st = state().lock().unwrap();
    // start photo with provided surface

    unsafe {
        if st.manager.is_null() {
            log::error!("[Harmony.Camera] manager is null");
            return Err(PlatformError::Platform(
                "camera manager not initialized".into(),
            ));
        }

        // Setup cameras/capability/session/input/preview once lazily
        if st.cameras.is_null() {
            setup_camera_base(&mut st)?;
        }

        if st.session.is_null() {
            // create session
            let mut sess: *mut Camera_CaptureSession = ptr::null_mut();
            let rc = OH_CameraManager_CreateCaptureSession(st.manager, &mut sess);
            if rc != 0 || sess.is_null() {
                log::error!("[Harmony.Camera] CreateCaptureSession failed: rc={}", rc);
                return Err(PlatformError::Platform(
                    "CreateCaptureSession failed".into(),
                ));
            }
            st.session = sess;
        }

        // check camera input
        if st.camera_input.is_null() {
            // create camera input
            let device = st.cameras.offset(st.camera_index as isize);
            let mut input: *mut Camera_Input = ptr::null_mut();
            let rc = OH_CameraManager_CreateCameraInput(st.manager, device, &mut input);
            if rc != 0 || input.is_null() {
                log::error!("[Harmony.Camera] CreateCameraInput failed: rc={}", rc);
                return Err(PlatformError::Platform("CreateCameraInput failed".into()));
            }
            st.camera_input = input;
            let rc = OH_CameraInput_Open(input);
            if rc != 0 {
                log::error!("[Harmony.Camera] CameraInput_Open failed: rc={}", rc);
                return Err(PlatformError::Platform("CameraInput_Open failed".into()));
            }
        }

        // stop session for reconfig
        let rc_stop = OH_CaptureSession_Stop(st.session);
        if rc_stop != 0 {
            log::warn!(
                "[Harmony.Camera] Stop session failed: {}, continuing anyway",
                rc_stop
            );
        }

        let rc_begin = OH_CaptureSession_BeginConfig(st.session);
        if rc_begin != 0 {
            log::error!("[Harmony.Camera] BeginConfig failed: {}", rc_begin);
            return Err(PlatformError::Platform(format!(
                "BeginConfig failed: {}",
                rc_begin
            )));
        }

        // Remove old photo_output if exists
        if !st.photo_output.is_null() {
            // release old photo output
            OH_PhotoOutput_Release(st.photo_output);
            st.photo_output = ptr::null_mut();
        }

        // Create photo output with ImageReceiver surface id
        let cstr = CString::new(photo_surface_id)
            .map_err(|_| PlatformError::InvalidParameter("surfaceId null".into()))?;
        let mut photo_out: *mut Camera_PhotoOutput = ptr::null_mut();
        let profile = choose_photo_profile(st.capability)?;
        let rc =
            OH_CameraManager_CreatePhotoOutput(st.manager, profile, cstr.as_ptr(), &mut photo_out);
        if rc != 0 || photo_out.is_null() {
            let fallback = OH_CameraManager_CreatePhotoOutputUsedInPreconfig(
                st.manager,
                cstr.as_ptr(),
                &mut photo_out,
            );
            if fallback != 0 || photo_out.is_null() {
                return Err(PlatformError::Platform("CreatePhotoOutput failed".into()));
            }
        }

        // Add PhotoOutput to session FIRST (before registering callbacks)
        let rc_add_photo = OH_CaptureSession_AddPhotoOutput(st.session, photo_out);

        if rc_add_photo != 0 {
            log::error!("[Harmony.Camera] AddPhotoOutput failed: {}", rc_add_photo);
            OH_PhotoOutput_Release(photo_out);
            st.photo_output = ptr::null_mut();
            return Err(PlatformError::Platform(format!(
                "AddPhotoOutput failed: {}",
                rc_add_photo
            )));
        }
        st.photo_output = photo_out;

        // Now commit and start the session
        let rc_commit = OH_CaptureSession_CommitConfig(st.session);
        if rc_commit != 0 {
            log::error!("[Harmony.Camera] CommitConfig failed: {}", rc_commit);
            return Err(PlatformError::Platform(format!(
                "CommitConfig failed: {}",
                rc_commit
            )));
        }

        let rc_start = OH_CaptureSession_Start(st.session);
        if rc_start != 0 {
            log::error!("[Harmony.Camera] Start session failed: {}", rc_start);
            return Err(PlatformError::Platform(format!(
                "Start session failed: {}",
                rc_start
            )));
        }
    }
    Ok(true)
}

pub fn camera_start_video_with_surface(video_surface_id: &str) -> Result<bool, PlatformError> {
    let mut st = state().lock().unwrap();
    unsafe {
        if st.manager.is_null() {
            return Err(PlatformError::Platform(
                "camera manager not initialized".into(),
            ));
        }
        if st.cameras.is_null() {
            setup_camera_base(&mut st)?;
        }
        if st.session.is_null() {
            let mut sess: *mut Camera_CaptureSession = ptr::null_mut();
            let rc = OH_CameraManager_CreateCaptureSession(st.manager, &mut sess);
            if rc != 0 || sess.is_null() {
                return Err(PlatformError::Platform(
                    "CreateCaptureSession failed".into(),
                ));
            }
            st.session = sess;
        }
        if st.camera_input.is_null() {
            let device = st.cameras.offset(st.camera_index as isize);
            let mut input: *mut Camera_Input = ptr::null_mut();
            let rc = OH_CameraManager_CreateCameraInput(st.manager, device, &mut input);
            if rc != 0 || input.is_null() {
                return Err(PlatformError::Platform("CreateCameraInput failed".into()));
            }
            st.camera_input = input;
            let rc = OH_CameraInput_Open(input);
            if rc != 0 {
                return Err(PlatformError::Platform("CameraInput_Open failed".into()));
            }
        }

        OH_CaptureSession_BeginConfig(st.session);
        OH_CaptureSession_AddInput(st.session, st.camera_input);

        if !st.video_output.is_null() {
            OH_CaptureSession_RemoveVideoOutput(st.session, st.video_output);
            OH_VideoOutput_Release(st.video_output);
            st.video_output = ptr::null_mut();
        }
        let vprof = choose_video_profile(st.capability)?;
        let cstr = CString::new(video_surface_id)
            .map_err(|_| PlatformError::InvalidParameter("surfaceId null".into()))?;
        let mut video_out: *mut Camera_VideoOutput = ptr::null_mut();
        let rc =
            OH_CameraManager_CreateVideoOutput(st.manager, vprof, cstr.as_ptr(), &mut video_out);
        if rc != 0 || video_out.is_null() {
            return Err(PlatformError::Platform("CreateVideoOutput failed".into()));
        }
        st.video_output = video_out;
        OH_CaptureSession_AddVideoOutput(st.session, video_out);
        OH_CaptureSession_CommitConfig(st.session);
        OH_CaptureSession_Start(st.session);
    }
    Ok(true)
}

pub fn camera_video_output_start() -> Result<bool, PlatformError> {
    let st = state().lock().unwrap();
    unsafe {
        if st.video_output.is_null() {
            return Err(PlatformError::Platform(
                "video output not configured".into(),
            ));
        }
        let rc = OH_VideoOutput_Start(st.video_output);
        if rc != 0 {
            return Err(PlatformError::Platform(format!(
                "OH_VideoOutput_Start failed: {}",
                rc
            )));
        }
    }
    Ok(true)
}

pub fn camera_video_output_stop_and_release() -> Result<bool, PlatformError> {
    let mut st = state().lock().unwrap();
    unsafe {
        if !st.video_output.is_null() {
            OH_VideoOutput_Stop(st.video_output);
            if !st.session.is_null() {
                OH_CaptureSession_RemoveVideoOutput(st.session, st.video_output);
            }
            OH_VideoOutput_Release(st.video_output);
            st.video_output = ptr::null_mut();
        }
    }
    Ok(true)
}

unsafe fn setup_camera_base(st: &mut CamState) -> Result<(), PlatformError> {
    // Enumerate cameras
    let mut cameras_ptr: *mut Camera_Device = ptr::null_mut();
    let mut size: u32 = 0;
    let rc =
        unsafe { OH_CameraManager_GetSupportedCameras(st.manager, &mut cameras_ptr, &mut size) };
    if rc != 0 || cameras_ptr.is_null() || size == 0 {
        return Err(PlatformError::Platform("GetSupportedCameras failed".into()));
    }
    st.cameras = cameras_ptr;
    st.cameras_len = size;
    st.camera_index = 0; // pick first (simple)

    // Capability
    let mut cap: *mut Camera_OutputCapability = ptr::null_mut();
    let device = unsafe { st.cameras.offset(st.camera_index as isize) };
    let rc = unsafe {
        OH_CameraManager_GetSupportedCameraOutputCapability(st.manager, device, &mut cap)
    };
    if rc != 0 || cap.is_null() {
        return Err(PlatformError::Platform(
            "GetSupportedCameraOutputCapability failed".into(),
        ));
    }
    st.capability = cap;
    Ok(())
}

fn choose_preview_profile(
    cap: *mut Camera_OutputCapability,
) -> Result<*mut Camera_Profile, PlatformError> {
    unsafe {
        let caps = &*cap;
        if caps.preview_profiles_size == 0 || caps.preview_profiles.is_null() {
            return Err(PlatformError::Platform("no preview profiles".into()));
        }
        let arr = slice::from_raw_parts(caps.preview_profiles, caps.preview_profiles_size as usize);
        let first = *arr
            .first()
            .ok_or_else(|| PlatformError::Platform("empty preview profiles".into()))?;
        if first.is_null() {
            return Err(PlatformError::Platform("invalid preview profile".into()));
        }
        Ok(first)
    }
}
fn choose_photo_profile(
    cap: *mut Camera_OutputCapability,
) -> Result<*mut Camera_Profile, PlatformError> {
    unsafe {
        let caps = &*cap;
        if caps.photo_profiles_size == 0 || caps.photo_profiles.is_null() {
            return choose_preview_profile(cap);
        }
        let arr = slice::from_raw_parts(caps.photo_profiles, caps.photo_profiles_size as usize);
        let first = *arr
            .first()
            .ok_or_else(|| PlatformError::Platform("empty photo profiles".into()))?;
        if first.is_null() {
            return Err(PlatformError::Platform("invalid photo profile".into()));
        }
        Ok(first)
    }
}
fn choose_video_profile(
    cap: *mut Camera_OutputCapability,
) -> Result<*mut Camera_VideoProfile, PlatformError> {
    unsafe {
        let caps = &*cap;
        if caps.video_profiles_size == 0 || caps.video_profiles.is_null() {
            return Err(PlatformError::Platform("no video profiles".into()));
        }
        let arr = slice::from_raw_parts(caps.video_profiles, caps.video_profiles_size as usize);
        let first = *arr
            .first()
            .ok_or_else(|| PlatformError::Platform("empty video profiles".into()))?;
        if first.is_null() {
            return Err(PlatformError::Platform("invalid video profile".into()));
        }
        Ok(first)
    }
}

unsafe fn cleanup_state(st: &mut CamState) {
    if !st.photo_output.is_null() {
        if !st.session.is_null() {
            unsafe {
                OH_CaptureSession_RemovePhotoOutput(st.session, st.photo_output);
            }
        }
        unsafe {
            OH_PhotoOutput_Release(st.photo_output);
        }
        st.photo_output = ptr::null_mut();
    }
    if !st.video_output.is_null() {
        unsafe {
            OH_VideoOutput_Stop(st.video_output);
        }
        if !st.session.is_null() {
            unsafe {
                OH_CaptureSession_RemoveVideoOutput(st.session, st.video_output);
            }
        }
        unsafe {
            OH_VideoOutput_Release(st.video_output);
        }
        st.video_output = ptr::null_mut();
    }
    if !st.preview_output.is_null() {
        if !st.session.is_null() {
            unsafe {
                OH_CaptureSession_RemovePreviewOutput(st.session, st.preview_output);
            }
        }
        unsafe {
            OH_PreviewOutput_Stop(st.preview_output);
            OH_PreviewOutput_Release(st.preview_output);
        }
        st.preview_output = ptr::null_mut();
    }
    if !st.session.is_null() {
        unsafe {
            OH_CaptureSession_Stop(st.session);
            OH_CaptureSession_Release(st.session);
        }
        st.session = ptr::null_mut();
    }
    if !st.camera_input.is_null() {
        unsafe {
            OH_CameraInput_Close(st.camera_input);
            OH_CameraInput_Release(st.camera_input);
        }
        st.camera_input = ptr::null_mut();
    }
    if !st.capability.is_null() && !st.manager.is_null() {
        unsafe {
            OH_CameraManager_DeleteSupportedCameraOutputCapability(st.manager, st.capability);
        }
        st.capability = ptr::null_mut();
    }
    if !st.cameras.is_null() && !st.manager.is_null() && st.cameras_len > 0 {
        unsafe {
            OH_CameraManager_DeleteSupportedCameras(st.manager, st.cameras, st.cameras_len);
        }
        st.cameras = ptr::null_mut();
        st.cameras_len = 0;
    }
    if !st.manager.is_null() {
        unsafe {
            OH_Camera_DeleteCameraManager(st.manager);
        }
        st.manager = ptr::null_mut();
    }
}
