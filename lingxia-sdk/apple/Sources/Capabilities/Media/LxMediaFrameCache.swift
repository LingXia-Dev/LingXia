#if os(iOS)
import Foundation
import AVFoundation
import UIKit

/// Process-level cache for first-frame images of local video assets, used by
/// `LxMediaPlayer`'s playlist mode to bridge the visual gap during item
/// transitions (manual jumps and error recovery).
///
/// Extraction is independent of any `AVPlayer` instance — uses
/// `AVAssetImageGenerator`, which decodes a single frame without holding the
/// codec/buffer resources of a full playback session. Typical local mp4 cost
/// is ~10-50 ms; remote URLs short-circuit to nil.
///
/// Memory:
/// - Backed by `NSCache<NSString, UIImage>`, which UIKit auto-evicts on
///   memory pressure (its native behavior; no explicit observer needed).
/// - Total bytes capped via `totalCostLimit` (heap-relative, conservative).
///
/// Threading:
/// - Public API is callable from any thread; callbacks dispatch on the main
///   queue. Decode runs on a dedicated serial queue so AVAssetImageGenerator
///   isn't created/destroyed concurrently.
///
/// Same-URL concurrent requests are coalesced: single decode, callbacks fan out.
///
/// Not `@MainActor`-isolated by design: the rest of the media stack uses plain
/// classes with `DispatchQueue.main.async` for main-thread work. Adding actor
/// isolation here would force callers (e.g. KVO observers, `Player.Listener`)
/// into `Task @MainActor` shims unnecessarily. Mutable fields are protected
/// via the queues described below — see method docs for thread-safety rules.
final class LxMediaFrameCache: @unchecked Sendable {

    static let shared = LxMediaFrameCache()

    /// `NSCache` is documented as thread-safe; reads/writes from any thread are OK.
    private let cache: NSCache<NSString, UIImage> = {
        let c = NSCache<NSString, UIImage>()
        c.totalCostLimit = LxMediaFrameCache.computeCostLimit()
        return c
    }()

    private let decodeQueue = DispatchQueue(label: "com.lingxia.media.framecache", qos: .userInitiated)
    /// All access to `inFlight` is on the main queue (enqueue from `load`,
    /// drain from the dispatched main-queue completion). No additional lock.
    private var inFlight: [String: [@MainActor @Sendable (UIImage?) -> Void]] = [:]

    private init() {}

    /// Synchronous cache lookup. Cheap; safe on any thread (NSCache is documented
    /// as thread-safe). Callers usually invoke from the main queue.
    func peek(for url: URL) -> UIImage? {
        guard isLocal(url) else { return nil }
        return cache.object(forKey: url.absoluteString as NSString)
    }

    /// Asynchronously extract and cache the first frame for `url`. Callback fires on
    /// the main queue. Cache hits dispatch via `DispatchQueue.main.async` (single hop).
    ///
    /// The callback is `@MainActor`-typed so callers can touch main-actor state
    /// without extra hop. The `MainActor.assumeIsolated` blocks below are safe
    /// because every call site is inside `DispatchQueue.main.async`.
    func load(_ url: URL, completion: @MainActor @Sendable @escaping (UIImage?) -> Void) {
        guard isLocal(url) else {
            DispatchQueue.main.async { MainActor.assumeIsolated { completion(nil) } }
            return
        }
        let key = url.absoluteString
        if let cached = cache.object(forKey: key as NSString) {
            DispatchQueue.main.async { MainActor.assumeIsolated { completion(cached) } }
            return
        }
        // Coalesce concurrent loads for the same URL.
        if var pending = inFlight[key] {
            pending.append(completion)
            inFlight[key] = pending
            return
        }
        inFlight[key] = [completion]
        decodeQueue.async {
            let image = LxMediaFrameCache.extractFirstFrame(from: url)
            DispatchQueue.main.async { [weak self] in
                MainActor.assumeIsolated {
                    guard let self = self else { return }
                    if let image = image {
                        self.cache.setObject(image, forKey: key as NSString, cost: Self.byteCost(of: image))
                    }
                    let callbacks = self.inFlight.removeValue(forKey: key) ?? []
                    callbacks.forEach { $0(image) }
                }
            }
        }
    }

    /// Fire-and-forget prefetch. Skips if already cached or in-flight.
    func prefetch(_ url: URL) {
        guard isLocal(url) else { return }
        let key = url.absoluteString
        if cache.object(forKey: key as NSString) != nil { return }
        if inFlight[key] != nil { return }
        load(url) { _ in /* result is in cache */ }
    }

    func evictAll() {
        cache.removeAllObjects()
    }

    // MARK: - Private

    private func isLocal(_ url: URL) -> Bool {
        if url.isFileURL { return true }
        let scheme = url.scheme?.lowercased()
        return scheme == "file" || scheme == nil
    }

    /// Real pixel byte count for an image (`width × height × scale² × 4`). The
    /// CGImage's `bytesPerRow × height` is the most reliable source.
    private static func byteCost(of image: UIImage) -> Int {
        if let cg = image.cgImage {
            return cg.bytesPerRow * cg.height
        }
        let w = Int(image.size.width * image.scale)
        let h = Int(image.size.height * image.scale)
        return max(1, w * h * 4)
    }

    /// Decode synchronously on the calling (decode) queue.
    private static func extractFirstFrame(from url: URL) -> UIImage? {
        let asset = AVURLAsset(url: url, options: [AVURLAssetPreferPreciseDurationAndTimingKey: false])
        let generator = AVAssetImageGenerator(asset: asset)
        generator.appliesPreferredTrackTransform = true
        // Cap output dimensions to screen-class resolution to control memory.
        generator.maximumSize = CGSize(width: 1920, height: 1920)
        generator.requestedTimeToleranceBefore = .positiveInfinity
        generator.requestedTimeToleranceAfter = .positiveInfinity
        do {
            let cgImage = try generator.copyCGImage(at: .zero, actualTime: nil)
            return UIImage(cgImage: cgImage)
        } catch {
            return nil
        }
    }

    /// Heap-aware cost limit. Conservative: tied to ~1/32 of physical memory
    /// (NSCache evicts when total stored bytes exceed limit OR on memory warning).
    private static func computeCostLimit() -> Int {
        let physical = ProcessInfo.processInfo.physicalMemory
        let target = Int(physical / 32)
        // Clamp to a sensible range for our use case.
        let minLimit = 4 * 1024 * 1024
        let maxLimit = 32 * 1024 * 1024
        return max(minLimit, min(maxLimit, target))
    }
}
#endif
