import CoreGraphics
import Foundation

enum NativeComponentClip {
    static func intersection(_ rect: CGRect, clips: Any?) -> CGRect? {
        guard let clips = clips as? [[String: Any]], !clips.isEmpty else { return nil }
        return clips.reduce(rect) { visible, clip in
            guard let x = clip["x"] as? NSNumber, let y = clip["y"] as? NSNumber,
                  let width = clip["width"] as? NSNumber, let height = clip["height"] as? NSNumber
            else { return .zero }
            let next = visible.intersection(CGRect(x: x.doubleValue, y: y.doubleValue,
                                                   width: width.doubleValue, height: height.doubleValue))
            return next.isNull ? .zero : next
        }
    }
}
