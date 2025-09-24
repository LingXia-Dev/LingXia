//! Apple platform location services implementation

use super::Platform;
use crate::error::PlatformError;
use crate::traits::Location;

#[cfg(target_os = "ios")]
mod ios {
    use super::*;
    use dispatch2::DispatchQueue;
    use objc2::define_class;
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
    use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly};
    use objc2_core_location::{
        CLAuthorizationStatus, CLLocation, CLLocationManager, CLLocationManagerDelegate,
        kCLDistanceFilterNone, kCLLocationAccuracyBest, kCLLocationAccuracyHundredMeters,
    };
    use objc2_foundation::{NSArray, NSError};
    use serde_json::json;
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, OnceLock};
    use std::time::{Duration, Instant};

    // Global storage for location callbacks that can be accessed from any thread.
    static LOCATION_CALLBACKS: OnceLock<Arc<Mutex<HashMap<u64, LocationCallbackInfo>>>> =
        OnceLock::new();

    #[derive(Clone)]
    struct LocationCallbackInfo {
        callback_id: u64,
        config: crate::LocationRequestConfig,
        start_time: Instant,
    }

    struct ActiveLocationRequest {
        manager: Retained<CLLocationManager>,
        delegate: Retained<LocationDelegate>,
    }

    std::thread_local! {
        static ACTIVE_REQUESTS: RefCell<HashMap<u64, ActiveLocationRequest>> = RefCell::new(HashMap::new());
    }

    #[derive(Default)]
    struct LocationDelegateIvars {
        callback_id: Cell<u64>,
    }

    define_class!(
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "LingXiaLocationDelegate"]
        #[ivars = LocationDelegateIvars]
        struct LocationDelegate;

        impl LocationDelegate {
            #[unsafe(method(locationManager:didUpdateLocations:))]
            fn location_manager_did_update_locations(
                &self,
                _manager: &CLLocationManager,
                locations: &NSArray<CLLocation>,
            ) {
                let callback_id = self.callback_id();
                let maybe_location = locations.lastObject();
                let Some(location) = maybe_location else {
                    log::warn!("iOS Location: Received update without locations for callback {}", callback_id);
                    deliver_failure(callback_id, "No location available");
                    return;
                };
                deliver_success(callback_id, &location);
            }

            #[unsafe(method(locationManager:didFailWithError:))]
            fn location_manager_did_fail_with_error(
                &self,
                _manager: &CLLocationManager,
                error: &NSError,
            ) {
                let callback_id = self.callback_id();
                let message = format!("Location update failed: {}", ns_error_to_string(error));
                log::error!("iOS Location: {} (callback {})", message, callback_id);
                deliver_failure(callback_id, &message);
            }

            #[unsafe(method(locationManagerDidChangeAuthorization:))]
            fn location_manager_did_change_authorization(&self, manager: &CLLocationManager) {
                let status = unsafe { manager.authorizationStatus() };
                let callback_id = self.callback_id();
                match status {
                    CLAuthorizationStatus::AuthorizedWhenInUse | CLAuthorizationStatus::AuthorizedAlways => {
                        unsafe {
                            manager.requestLocation();
                        }
                    }
                    CLAuthorizationStatus::Denied | CLAuthorizationStatus::Restricted => {
                        log::warn!("iOS Location: Authorization changed to denied/restricted for callback {}", callback_id);
                        deliver_failure(callback_id, "Location permission denied");
                    }
                    CLAuthorizationStatus::NotDetermined => {
                        log::debug!("iOS Location: Authorization still not determined for callback {}", callback_id);
                    }
                    _ => {}
                }
            }
        }

        unsafe impl NSObjectProtocol for LocationDelegate {}
        unsafe impl CLLocationManagerDelegate for LocationDelegate {}
    );

    impl LocationDelegate {
        fn new(callback_id: u64) -> Retained<Self> {
            let mtm =
                MainThreadMarker::new().expect("LocationDelegate must be created on main thread");
            let this = Self::alloc(mtm);
            let this = this.set_ivars(LocationDelegateIvars {
                callback_id: Cell::new(callback_id),
            });
            unsafe { msg_send![super(this), init] }
        }

        fn callback_id(&self) -> u64 {
            self.ivars().callback_id.get()
        }
    }

    fn ns_error_to_string(error: &NSError) -> String {
        error.localizedDescription().to_string()
    }

    fn active_requests_mut<F, R>(f: F) -> R
    where
        F: FnOnce(&mut HashMap<u64, ActiveLocationRequest>) -> R,
    {
        ACTIVE_REQUESTS.with(|cell| {
            let mut borrow = cell.borrow_mut();
            f(&mut borrow)
        })
    }

    fn insert_active_request(callback_id: u64, request: ActiveLocationRequest) {
        active_requests_mut(|active| {
            active.insert(callback_id, request);
        });
    }

    fn take_active_request(callback_id: u64) -> Option<ActiveLocationRequest> {
        active_requests_mut(|active| active.remove(&callback_id))
    }

    fn callbacks() -> &'static Arc<Mutex<HashMap<u64, LocationCallbackInfo>>> {
        LOCATION_CALLBACKS.get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
    }

    fn remove_callback_info(callback_id: u64) -> Option<LocationCallbackInfo> {
        let callbacks = callbacks();
        let mut guard = callbacks.lock().unwrap();
        guard.remove(&callback_id)
    }

    fn cleanup_request(callback_id: u64) {
        if let Some(active) = take_active_request(callback_id) {
            unsafe {
                active.manager.stopUpdatingLocation();
                active.manager.setDelegate(None);
            }
        }
    }

    fn deliver_success(callback_id: u64, location: &CLLocation) {
        let info = remove_callback_info(callback_id);
        cleanup_request(callback_id);
        let Some(info) = info else {
            log::debug!(
                "iOS Location: Callback {} already handled before success",
                callback_id
            );
            return;
        };

        let payload = build_location_payload(location, info.config.include_altitude);
        log::info!(
            "iOS Location: Delivering success result for callback {}",
            callback_id
        );
        lingxia_messaging::invoke_callback(callback_id, true, payload);
    }

    fn deliver_failure(callback_id: u64, message: &str) {
        let info_present = remove_callback_info(callback_id).is_some();
        cleanup_request(callback_id);
        if !info_present {
            log::debug!(
                "iOS Location: Callback {} already handled before failure",
                callback_id
            );
            return;
        }

        let payload = json!({ "error": message }).to_string();
        lingxia_messaging::invoke_callback(callback_id, false, payload);
    }

    fn build_location_payload(location: &CLLocation, include_altitude: bool) -> String {
        unsafe {
            let coordinate = location.coordinate();
            let horizontal_accuracy = sanitize_measurement(location.horizontalAccuracy());
            let vertical_accuracy = sanitize_measurement(location.verticalAccuracy());
            let speed = sanitize_measurement(location.speed());
            let altitude = if include_altitude {
                sanitize_measurement(location.altitude())
            } else {
                0.0
            };

            json!({
                "latitude": coordinate.latitude,
                "longitude": coordinate.longitude,
                "speed": speed,
                "accuracy": horizontal_accuracy,
                "altitude": altitude,
                "vertical_accuracy": vertical_accuracy,
                "horizontal_accuracy": horizontal_accuracy,
            })
            .to_string()
        }
    }

    fn sanitize_measurement(value: f64) -> f64 {
        if value.is_finite() && value >= 0.0 {
            value
        } else {
            0.0
        }
    }

    pub(super) fn is_location_enabled() -> Result<bool, PlatformError> {
        let enabled = unsafe { CLLocationManager::locationServicesEnabled_class() };
        Ok(enabled)
    }

    pub(super) fn request_location(callback_id: u64) -> Result<(), PlatformError> {
        request_location_with_config(callback_id, crate::LocationRequestConfig::default())
    }

    pub(super) fn request_location_with_config(
        callback_id: u64,
        config: crate::LocationRequestConfig,
    ) -> Result<(), PlatformError> {
        let services_enabled = unsafe { CLLocationManager::locationServicesEnabled_class() };
        if !services_enabled {
            log::error!("iOS Location: Services disabled");
            lingxia_messaging::invoke_callback(
                callback_id,
                false,
                json!({ "error": "Location services are disabled" }).to_string(),
            );
            return Err(PlatformError::Platform(
                "Location services are disabled".into(),
            ));
        }

        // Record callback info so delegate can access configuration.
        callbacks().lock().unwrap().insert(
            callback_id,
            LocationCallbackInfo {
                callback_id,
                config: config.clone(),
                start_time: Instant::now(),
            },
        );

        #[allow(deprecated)]
        let authorization_status = unsafe { CLLocationManager::authorizationStatus_class() };
        if matches!(
            authorization_status,
            CLAuthorizationStatus::Denied | CLAuthorizationStatus::Restricted
        ) {
            log::warn!(
                "iOS Location: Authorization denied/restricted before request (status: {:?})",
                authorization_status
            );
            deliver_failure(callback_id, "Location permission denied");
            return Err(PlatformError::Platform("Location permission denied".into()));
        }

        DispatchQueue::main().exec_async(move || {
            if let Err(err_msg) = start_location_request(callback_id) {
                log::error!("iOS Location: Failed to start request {}", err_msg);
                deliver_failure(callback_id, &err_msg);
            }
        });

        // Set up timeout monitoring if requested.
        if let Some(timeout_ms) = config.high_accuracy_expire_time {
            let callbacks = callbacks().clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(timeout_ms as u64));
                let should_timeout = {
                    let mut guard = callbacks.lock().unwrap();
                    if guard.contains_key(&callback_id) {
                        guard.remove(&callback_id);
                        true
                    } else {
                        false
                    }
                };

                if should_timeout {
                    log::warn!(
                        "iOS Location: Request {} timed out after {}ms",
                        callback_id,
                        timeout_ms
                    );
                    DispatchQueue::main().exec_async(move || {
                        cleanup_request(callback_id);
                        let payload = json!({ "error": "Location request timed out" }).to_string();
                        lingxia_messaging::invoke_callback(callback_id, false, payload);
                    });
                }
            });
        }

        Ok(())
    }

    fn start_location_request(callback_id: u64) -> Result<(), String> {
        let callback_info = {
            let callbacks = callbacks();
            let guard = callbacks.lock().unwrap();
            guard.get(&callback_id).cloned()
        }
        .ok_or_else(|| "Location request not registered".to_string())?;

        let manager = unsafe { CLLocationManager::new() };
        let delegate = LocationDelegate::new(callback_id);
        let high_accuracy = callback_info.config.is_high_accuracy;

        unsafe {
            let desired_accuracy = if high_accuracy {
                kCLLocationAccuracyBest
            } else {
                kCLLocationAccuracyHundredMeters
            };
            manager.setDesiredAccuracy(desired_accuracy);

            if high_accuracy {
                manager.setDistanceFilter(kCLDistanceFilterNone);
            }

            manager.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

            let status = manager.authorizationStatus();
            if status == CLAuthorizationStatus::NotDetermined {
                manager.requestWhenInUseAuthorization();
            }
        }

        insert_active_request(
            callback_id,
            ActiveLocationRequest {
                manager: manager.clone(),
                delegate: delegate.clone(),
            },
        );

        unsafe {
            manager.requestLocation();
        }

        Ok(())
    }
}

impl Location for Platform {
    fn is_location_enabled(&self) -> Result<bool, PlatformError> {
        #[cfg(target_os = "ios")]
        {
            ios::is_location_enabled()
        }

        #[cfg(not(target_os = "ios"))]
        {
            Ok(false)
        }
    }

    fn request_location(
        &self,
        callback_id: u64,
        config: crate::LocationRequestConfig,
    ) -> Result<(), PlatformError> {
        #[cfg(target_os = "ios")]
        {
            ios::request_location_with_config(callback_id, config)
        }

        #[cfg(not(target_os = "ios"))]
        {
            let _ = config;
            let _ = lingxia_messaging::invoke_callback(
                callback_id,
                false,
                "Location services are not supported on this platform".to_string(),
            );
            Err(PlatformError::Platform(
                "Location not available on this platform".into(),
            ))
        }
    }
}
