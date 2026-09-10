import CLingXiaRustAPI
#if os(iOS)
import UIKit
#endif

/// Tablets stay `mobile` for `showOn`. This only widens the compact tab strip
/// from five slots to the ten-item declaration cap.
enum LxAppHostFormFactor {
    static func applyPadFlag() {
        #if os(iOS)
        set_pad(UIDevice.current.userInterfaceIdiom == .pad)
        #endif
    }
}
