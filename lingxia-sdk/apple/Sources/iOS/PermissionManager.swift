#if os(iOS)
import AVFoundation
import CoreLocation
import Photos
import UIKit

enum PermissionManager {
    @MainActor
    private static var locationRequester: LocationPermissionRequester?

    @MainActor
    static func ensureCameraAccess(completion: @escaping (Bool) -> Void) {
        let status = AVCaptureDevice.authorizationStatus(for: .video)
        switch status {
        case .authorized:
            completion(true)
        case .notDetermined:
            AVCaptureDevice.requestAccess(for: .video) { granted in
                Task { @MainActor in
                    completion(granted)
                }
            }
        case .denied, .restricted:
            completion(false)
        @unknown default:
            completion(false)
        }
    }

    @MainActor
    static func ensureMicrophoneAccess(completion: @escaping (Bool) -> Void) {
        let audioSession = AVAudioSession.sharedInstance()
        switch audioSession.recordPermission {
        case .granted:
            completion(true)
        case .denied:
            completion(false)
        case .undetermined:
            audioSession.requestRecordPermission { granted in
                Task { @MainActor in
                    completion(granted)
                }
            }
        @unknown default:
            completion(false)
        }
    }

    @MainActor
    static func ensurePhotoLibraryAccess(completion: @escaping (Bool) -> Void) {
        let status = PHPhotoLibrary.authorizationStatus(for: .readWrite)
        switch status {
        case .authorized, .limited:
            completion(true)
        case .denied, .restricted:
            completion(false)
        case .notDetermined:
            PHPhotoLibrary.requestAuthorization(for: .readWrite) { newStatus in
                Task { @MainActor in
                    completion(newStatus == .authorized || newStatus == .limited)
                }
            }
        @unknown default:
            completion(false)
        }
    }

    @MainActor
    static func ensureLocationWhenInUseAccess(completion: @escaping (Bool) -> Void) {
        guard CLLocationManager.locationServicesEnabled() else {
            completion(false)
            return
        }

        let status = CLLocationManager.authorizationStatus()
        switch status {
        case .authorizedAlways, .authorizedWhenInUse:
            completion(true)
        case .denied, .restricted:
            completion(false)
        case .notDetermined:
            let requester = LocationPermissionRequester { granted in
                Self.locationRequester = nil
                completion(granted)
            }
            Self.locationRequester = requester
            requester.requestWhenInUseAuthorization()
        @unknown default:
            completion(false)
        }
    }
}

@MainActor
private final class LocationPermissionRequester: NSObject {
    private let manager = CLLocationManager()
    private let completion: (Bool) -> Void

    init(completion: @escaping (Bool) -> Void) {
        self.completion = completion
        super.init()
        manager.delegate = self
    }

    func requestWhenInUseAuthorization() {
        manager.requestWhenInUseAuthorization()
    }

    func locationManagerDidChangeAuthorization(_ manager: CLLocationManager) {
        let status = manager.authorizationStatus
        switch status {
        case .authorizedAlways, .authorizedWhenInUse:
            completion(true)
        case .denied, .restricted:
            completion(false)
        case .notDetermined:
            break
        @unknown default:
            completion(false)
        }
    }
}

extension LocationPermissionRequester: @preconcurrency CLLocationManagerDelegate {}
#endif
