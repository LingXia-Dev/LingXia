import Foundation

#if os(iOS)
import UIKit
public typealias PlatformView = UIView
public typealias PlatformViewController = UIViewController
public typealias PlatformColor = UIColor
public typealias PlatformApplication = UIApplication
public typealias PlatformFont = UIFont
public typealias PlatformEdgeInsets = UIEdgeInsets
public typealias PlatformLayoutConstraint = NSLayoutConstraint
public typealias PlatformLayoutPriority = UILayoutPriority
public typealias PlatformButton = UIButton
public typealias PlatformLabel = UILabel
public typealias PlatformTextField = UITextField
public typealias PlatformScrollView = UIScrollView
public typealias PlatformStackView = UIStackView
public typealias PlatformImageView = UIImageView
public typealias PlatformImage = UIImage

// iOS specific constants
public let PLATFORM_STATUS_BAR_HEIGHT: CGFloat = 48
public let PLATFORM_NAV_BAR_HEIGHT: CGFloat = 44
public let PLATFORM_TAB_BAR_HEIGHT: CGFloat = 64
public let PLATFORM_NAV_TITLE_VERTICAL_POSITION: CGFloat = 48 + 8

#elseif os(macOS)
import Cocoa
public typealias PlatformView = NSView
public typealias PlatformViewController = NSViewController
public typealias PlatformColor = NSColor
public typealias PlatformApplication = NSApplication
public typealias PlatformFont = NSFont
public typealias PlatformEdgeInsets = NSEdgeInsets
public typealias PlatformLayoutConstraint = NSLayoutConstraint
public typealias PlatformLayoutPriority = NSLayoutConstraint.Priority
public typealias PlatformButton = NSButton
public typealias PlatformLabel = NSTextField
public typealias PlatformTextField = NSTextField
public typealias PlatformScrollView = NSScrollView
public typealias PlatformStackView = NSStackView
public typealias PlatformImageView = NSImageView
public typealias PlatformImage = NSImage

// macOS specific constants
public let PLATFORM_STATUS_BAR_HEIGHT: CGFloat = 28
public let PLATFORM_NAV_BAR_HEIGHT: CGFloat = 32
public let PLATFORM_TAB_BAR_HEIGHT: CGFloat = 40
public let PLATFORM_NAV_TITLE_VERTICAL_POSITION: CGFloat = 0

#endif

// Common constants
public let CAPSULE_BUTTON_HEIGHT: CGFloat = 32
public let CAPSULE_BUTTON_WIDTH: CGFloat = 87

// Platform-specific extensions
extension PlatformColor {
    static var platformBackground: PlatformColor {
        #if os(iOS)
        return .systemBackground
        #else
        return .windowBackgroundColor
        #endif
    }
    
    static var platformLabel: PlatformColor {
        #if os(iOS)
        return .label
        #else
        return .labelColor
        #endif
    }

    static var platformSecondaryLabel: PlatformColor {
        #if os(iOS)
        return .secondaryLabel
        #else
        return .secondaryLabelColor
        #endif
    }
}

extension PlatformFont {
    static func platformSystemFont(ofSize size: CGFloat, weight: PlatformFont.Weight = .regular) -> PlatformFont {
        #if os(iOS)
        return .systemFont(ofSize: size, weight: weight)
        #else
        return .systemFont(ofSize: size, weight: weight)
        #endif
    }
}
