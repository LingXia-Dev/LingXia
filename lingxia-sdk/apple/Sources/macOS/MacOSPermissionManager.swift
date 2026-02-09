#if os(macOS)
import CoreLocation

@MainActor
enum PermissionManager {
    private static var locationRequester: LocationPermissionRequester?

    static func ensureLocationWhenInUseAccess(completion: @escaping (Bool) -> Void) {
        guard CLLocationManager.locationServicesEnabled() else {
            completion(false)
            return
        }

        let status = CLLocationManager().authorizationStatus
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
        switch manager.authorizationStatus {
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
