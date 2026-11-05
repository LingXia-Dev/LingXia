#if os(iOS)
import AVFoundation
import CoreLocation
import Photos
import UIKit

enum PermissionManager {
    @MainActor
    private static var locationRequester: LocationPermissionRequester?

    @MainActor
    static func ensureCameraAccess(
        for feature: PermissionFeature,
        completion: @escaping (Bool) -> Void
    ) {
        let status = AVCaptureDevice.authorizationStatus(for: .video)
        if status == .authorized {
            completion(true)
            return
        }
        PermissionGates.run(
            feature,
            permission: "camera",
            blocked: status != .notDetermined,
            declined: { completion(false) }
        ) { finish in
            guard status == .notDetermined else {
                finish(.deniedPermanently)
                completion(false)
                return
            }
            AVCaptureDevice.requestAccess(for: .video) { granted in
                Task { @MainActor in
                    finish(granted ? .granted : .deniedPermanently)
                    completion(granted)
                }
            }
        }
    }

    @MainActor
    static func ensureMicrophoneAccess(completion: @escaping (Bool) -> Void) {
        let audioSession = AVAudioSession.sharedInstance()
        let status = audioSession.recordPermission
        if status == .granted {
            completion(true)
            return
        }
        PermissionGates.run(
            .camera,
            permission: "microphone",
            blocked: status != .undetermined,
            declined: { completion(false) }
        ) { finish in
            guard status == .undetermined else {
                finish(.deniedPermanently)
                completion(false)
                return
            }
            audioSession.requestRecordPermission { granted in
                Task { @MainActor in
                    finish(granted ? .granted : .deniedPermanently)
                    completion(granted)
                }
            }
        }
    }

    @MainActor
    static func ensurePhotoLibraryAccess(completion: @escaping (Bool) -> Void) {
        ensurePhotoAccess(.readWrite, feature: .mediaLibrary, completion: completion)
    }

    @MainActor
    static func ensurePhotoAccess(
        _ level: PHAccessLevel,
        feature: PermissionFeature,
        completion: @escaping (Bool) -> Void
    ) {
        let status = PHPhotoLibrary.authorizationStatus(for: level)
        if status == .authorized || status == .limited {
            completion(true)
            return
        }
        PermissionGates.run(
            feature,
            permission: "photos",
            blocked: status != .notDetermined,
            declined: { completion(false) }
        ) { finish in
            guard status == .notDetermined else {
                finish(.deniedPermanently)
                completion(false)
                return
            }
            PHPhotoLibrary.requestAuthorization(for: level) { newStatus in
                Task { @MainActor in
                    let granted = newStatus == .authorized || newStatus == .limited
                    finish(granted ? .granted : .deniedPermanently)
                    completion(granted)
                }
            }
        }
    }

    @MainActor
    static func ensureLocationWhenInUseAccess(
        for feature: PermissionFeature,
        completion: @escaping (Bool) -> Void
    ) {
        guard CLLocationManager.locationServicesEnabled() else {
            completion(false)
            return
        }

        let status = CLLocationManager.authorizationStatus()
        if status == .authorizedAlways || status == .authorizedWhenInUse {
            completion(true)
            return
        }
        PermissionGates.run(
            feature,
            permission: "location",
            blocked: status != .notDetermined,
            declined: { completion(false) }
        ) { finish in
            guard status == .notDetermined else {
                finish(.deniedPermanently)
                completion(false)
                return
            }
            let requester = LocationPermissionRequester { granted in
                Self.locationRequester = nil
                finish(granted ? .granted : .deniedPermanently)
                completion(granted)
            }
            Self.locationRequester = requester
            requester.requestWhenInUseAuthorization()
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
