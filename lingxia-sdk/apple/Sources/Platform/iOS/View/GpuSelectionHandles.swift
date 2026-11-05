import Foundation
#if os(iOS)
import UIKit
import CLingXiaRustAPI

/// Handles for a static-text selection: drawn where the core places them, dragged
/// through the core, which moves the selection end.
extension GpuPageView {
    /// Redraw after a presented frame: handles follow the selection or go with it.
    func updateSelectionHandles() {
        let ends = gpuPageSelectionHandles(token)
        let count = Int(ends.len()) / 4
        guard count > 0 else {
            handleLayer?.isHidden = true
            return
        }
        let layer = handleLayer ?? {
            let made = CAShapeLayer()
            made.actions = ["path": NSNull(), "hidden": NSNull()]
            self.layer.addSublayer(made)
            handleLayer = made
            return made
        }()
        let argb = gpuPageSelectionHandleColor(token)
        let channel = { (shift: UInt32) in CGFloat((argb >> shift) & 0xff) / 255 }
        layer.fillColor = UIColor(red: channel(16), green: channel(8), blue: channel(0), alpha: channel(24)).cgColor
        let path = UIBezierPath()
        for index in 0..<count {
            let x = CGFloat(ends.get(index: UInt(index * 4 + 1)) ?? 0)
            let bottom = CGFloat(ends.get(index: UInt(index * 4 + 2)) ?? 0)
            // Android's shape: a 2 pt stem under the line and a 7 pt knob below it.
            path.append(UIBezierPath(rect: CGRect(x: x - 1, y: bottom - 1, width: 2, height: 8)))
            path.append(UIBezierPath(ovalIn: CGRect(x: x - 7, y: bottom + 1, width: 14, height: 14)))
        }
        layer.frame = bounds
        layer.path = path.cgPath
        layer.isHidden = false
    }

    /// A touch that grabs a handle drags it; true when the touch is the handle's.
    func handleTouch(_ phase: UInt8, at point: CGPoint) -> Bool {
        switch phase {
        case 0:
            let hit = gpuPageHandleHit(token, Float(point.x), Float(point.y))
            guard hit.len() == 3 else {
                handleGrab = nil
                return false
            }
            handleGrab = (Int32(hit.get(index: 0) ?? 0), CGFloat(hit.get(index: 1) ?? 0), CGFloat(hit.get(index: 2) ?? 0))
            return true
        case 1:
            guard let grab = handleGrab else { return false }
            gpuPageHandleDrag(token, grab.which, Float(point.x - grab.dx), Float(point.y - grab.dy))
            return true
        default:
            guard handleGrab != nil else { return false }
            handleGrab = nil
            gpuPageHandleDrag(token, -1, 0, 0)
            return true
        }
    }
}
#endif
