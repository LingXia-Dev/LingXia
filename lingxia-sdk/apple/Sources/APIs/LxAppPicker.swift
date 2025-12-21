import Foundation
import os.log
import UIKit
import CLingXiaSwiftAPI
import CLingXiaRustAPI

public class LxAppPicker {

    internal static let log = OSLog(subsystem: "LingXia", category: "Picker")

    // Static variables to track picker state
    @MainActor
    internal static var backgroundView: UIView?

    @MainActor
    internal static var currentPickerData: SimplePickerData?

    @MainActor
    internal static var currentWindow: UIWindow?

    // For custom scroll picker (columns container)
    @MainActor
    internal static var currentColumnsStack: UIStackView?

    // Instance management to avoid race when multiple pickers open/close
    @MainActor
    internal static var currentInstanceID: UInt64 = 0
    @MainActor
    internal static var activeInstanceID: UInt64 = 0
    // Retain scroll delegates (UIScrollView keeps a weak delegate)
    @MainActor
    internal static var scrollDelegates: [ColumnScrollDelegate] = []

    public static func showPicker(options: PickerOptions, callback_id: UInt64) {
        let columns = options.columns_json.toString()
        let cancelText = options.cancel_text.toString()
        let cancelButtonColor = options.cancel_button_color.toString()
        let cancelTextColor = options.cancel_text_color.toString()
        let confirmText = options.confirm_text.toString()
        let confirmButtonColor = options.confirm_button_color.toString()
        let confirmTextColor = options.confirm_text_color.toString()

        Task { @MainActor in
            showPicker(
                columns: columns,
                cancelText: cancelText,
                cancelButtonColor: cancelButtonColor,
                cancelTextColor: cancelTextColor,
                confirmText: confirmText,
                confirmButtonColor: confirmButtonColor,
                confirmTextColor: confirmTextColor,
                callbackID: callback_id
            )
        }
    }

    @MainActor
    public static func showPicker(
        columns: String,
        cancelText: String,
        cancelButtonColor: String,
        cancelTextColor: String,
        confirmText: String,
        confirmButtonColor: String,
        confirmTextColor: String,
        callbackID: UInt64
    ) {
        guard let configuration = PickerConfiguration.parse(
            columns: columns,
            cancelText: cancelText,
            cancelButtonColor: cancelButtonColor,
            cancelTextColor: cancelTextColor,
            confirmText: confirmText,
            confirmButtonColor: confirmButtonColor,
            confirmTextColor: confirmTextColor
        ) else {
            os_log("Failed to parse picker configuration", log: log, type: .error)
            sendPickerError(callback_id: callbackID, code: 1002)
            return
        }

        #if os(iOS)
        showIOSPicker(configuration: configuration, callbackID: callbackID)
        #else
        os_log("macOS picker not implemented", log: log, type: .error)
        sendPickerError(callback_id: callbackID, code: 6000)
        #endif
    }

    internal static func sendPickerResult(callback_id: UInt64, buttonType: String, selectedIndices: [Int]) {
        var payload: [String: Any] = [:]

        // Follow Android format: always use "index" field
        if selectedIndices.count == 1 {
            // Single column: use single number
            payload["index"] = selectedIndices.first ?? 0
        } else {
            // Multiple columns: use array
            payload["index"] = selectedIndices
        }
        payload[buttonType] = true

        let jsonData = try! JSONSerialization.data(withJSONObject: payload)
        let jsonString = String(data: jsonData, encoding: .utf8)!
        _ = onCallback(callback_id, true, jsonString)
    }

    // Specific methods for cancel and confirm to match Android implementation
    internal static func sendPickerResultCancel(callback_id: UInt64) {
        _ = onCallback(callback_id, false, "2000")
    }

    internal static func sendPickerError(callback_id: UInt64, code: Int) {
        _ = onCallback(callback_id, false, "\(code)")
    }

    internal static func sendPickerResultConfirm(callback_id: UInt64, selectedIndices: [Int]) {
        sendPickerResult(callback_id: callback_id, buttonType: "confirm", selectedIndices: selectedIndices)
    }
    internal static func sendPickerResultScroll(callback_id: UInt64, selectedIndices: [Int]) {
        var payload: [String: Any] = [:]
        if selectedIndices.count == 1 {
            payload["index"] = selectedIndices.first ?? 0
        } else {
            payload["index"] = selectedIndices
        }

        let jsonData = try! JSONSerialization.data(withJSONObject: payload)
        if let jsonString = String(data: jsonData, encoding: .utf8) {
            _ = onCallback(callback_id, true, jsonString)
        }
    }
}

#if os(iOS)
extension LxAppPicker {
    @MainActor
    internal static func showIOSPicker(configuration: PickerConfiguration, callbackID: UInt64) {
        guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = windowScene.windows.first(where: { $0.isKeyWindow }) ?? windowScene.windows.first,
              let rootViewController = window.rootViewController else {
            os_log("Unable to locate window or root view controller", log: log, type: .error)
            sendPickerError(callback_id: callbackID, code: 1000)
            return
        }

        // Store window reference for later use
        LxAppPicker.currentWindow = window

        // Find the topmost view controller
        var topViewController = rootViewController
        while let presentedViewController = topViewController.presentedViewController {
            topViewController = presentedViewController
        }

        // Ensure no previous picker remains to avoid overlap/race
        if let oldView = LxAppPicker.backgroundView {
            oldView.removeFromSuperview()
            LxAppPicker.backgroundView = nil
            LxAppPicker.currentPickerData = nil
            LxAppPicker.currentColumnsStack = nil
        }

        LxAppPicker.currentInstanceID &+= 1
        let instanceID = LxAppPicker.currentInstanceID
        LxAppPicker.activeInstanceID = instanceID

        // Create the picker directly here
        let screenBounds = UIScreen.main.bounds

        // Background overlay
        let backgroundView = UIView(frame: screenBounds)
        backgroundView.backgroundColor = UIColor.black.withAlphaComponent(0.4)
        backgroundView.alpha = 0
        backgroundView.translatesAutoresizingMaskIntoConstraints = false

        // Tag background with instance id and store for dismissal
        objc_setAssociatedObject(backgroundView, "pickerInstanceID", NSNumber(value: instanceID), .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        LxAppPicker.backgroundView = backgroundView

        // Container for picker content
        let containerView = UIView()
        containerView.backgroundColor = UIColor.white
        containerView.layer.cornerRadius = 16
        containerView.layer.maskedCorners = [.layerMinXMinYCorner, .layerMaxXMinYCorner] // Top corners only
        containerView.translatesAutoresizingMaskIntoConstraints = false

        // Store picker data for button callbacks (must be set BEFORE building columns)
        let pickerData = SimplePickerData(configuration: configuration)
        pickerData.callbackID = callbackID
        LxAppPicker.currentPickerData = pickerData

        // Create custom scroll picker container (aligns with Android behavior)
        let pickerContainer = createCustomPickerContainer(configuration: configuration, callbackID: callbackID)

        // Create button container
        let buttonContainer = UIView()
        buttonContainer.backgroundColor = UIColor.white  // Match picker background
        buttonContainer.translatesAutoresizingMaskIntoConstraints = false
        buttonContainer.isUserInteractionEnabled = true

        // Create cancel button with proper styling
        let cancelButton = UIButton(type: .system)
        cancelButton.setTitle(configuration.cancelText, for: .normal)
        cancelButton.backgroundColor = resolveColor(configuration.cancelButtonColor, fallback: UIColor.systemGray5)
        cancelButton.setTitleColor(resolveColor(configuration.cancelTextColor, fallback: UIColor.label), for: .normal)
        cancelButton.titleLabel?.font = UIFont.systemFont(ofSize: 17)
        cancelButton.layer.cornerRadius = 8
        cancelButton.translatesAutoresizingMaskIntoConstraints = false
        cancelButton.isUserInteractionEnabled = true
        cancelButton.isEnabled = true

        // Create confirm button with configurable colors
        let confirmButton = UIButton(type: .system)
        confirmButton.setTitle(configuration.confirmText, for: .normal)
        confirmButton.backgroundColor = resolveColor(configuration.confirmButtonColor, fallback: UIColor.systemBlue)
        confirmButton.setTitleColor(resolveColor(configuration.confirmTextColor, fallback: UIColor.white), for: .normal)
        confirmButton.titleLabel?.font = UIFont.boldSystemFont(ofSize: 17)
        confirmButton.layer.cornerRadius = 8
        confirmButton.translatesAutoresizingMaskIntoConstraints = false
        confirmButton.isUserInteractionEnabled = true
        confirmButton.isEnabled = true

        // pickerData created above already has callbackID set

        // Use modern UIAction for reliable button handling
        cancelButton.addAction(UIAction { _ in
            Task { @MainActor in
                guard let pickerData = LxAppPicker.currentPickerData else {
                    os_log("No picker data available for cancel", log: LxAppPicker.log, type: .error)
                    return
                }
                LxAppPicker.sendPickerResultCancel(callback_id: pickerData.callbackID)
                LxAppPicker.dismissPicker(expectedID: instanceID)
            }
        }, for: .touchUpInside)

        confirmButton.addAction(UIAction { _ in
            Task { @MainActor in
                guard let pickerData = LxAppPicker.currentPickerData else {
                    os_log("No picker data available for confirm", log: LxAppPicker.log, type: .error)
                    return
                }
                LxAppPicker.sendPickerResultConfirm(callback_id: pickerData.callbackID, selectedIndices: pickerData.currentSelection)
                LxAppPicker.dismissPicker(expectedID: instanceID)
            }
        }, for: .touchUpInside)

        // Add subviews
        buttonContainer.addSubview(cancelButton)
        buttonContainer.addSubview(confirmButton)
        containerView.addSubview(pickerContainer)
        containerView.addSubview(buttonContainer)
        backgroundView.addSubview(containerView)

        objc_setAssociatedObject(window, "currentPicker", backgroundView, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        window.addSubview(backgroundView)

        // Setup constraints with explicit priorities
        let constraints = [
            // Background view constraints (fill window)
            backgroundView.topAnchor.constraint(equalTo: window.topAnchor),
            backgroundView.leadingAnchor.constraint(equalTo: window.leadingAnchor),
            backgroundView.trailingAnchor.constraint(equalTo: window.trailingAnchor),
            backgroundView.bottomAnchor.constraint(equalTo: window.bottomAnchor),

            // Container constraints - extend to bottom
            containerView.leadingAnchor.constraint(equalTo: backgroundView.leadingAnchor),
            containerView.trailingAnchor.constraint(equalTo: backgroundView.trailingAnchor),
            containerView.bottomAnchor.constraint(equalTo: backgroundView.bottomAnchor),

            // Custom picker constraints
            pickerContainer.topAnchor.constraint(equalTo: containerView.topAnchor, constant: 16),
            pickerContainer.leadingAnchor.constraint(equalTo: containerView.leadingAnchor),
            pickerContainer.trailingAnchor.constraint(equalTo: containerView.trailingAnchor),
            pickerContainer.heightAnchor.constraint(equalToConstant: 216),

            // Button container constraints - add top padding
            buttonContainer.topAnchor.constraint(equalTo: pickerContainer.bottomAnchor, constant: 10),
            buttonContainer.leadingAnchor.constraint(equalTo: containerView.leadingAnchor),
            buttonContainer.trailingAnchor.constraint(equalTo: containerView.trailingAnchor),
            buttonContainer.bottomAnchor.constraint(equalTo: containerView.bottomAnchor, constant: -20),
            buttonContainer.heightAnchor.constraint(equalToConstant: 60),

            // Button constraints - centered with proper spacing
            cancelButton.centerYAnchor.constraint(equalTo: buttonContainer.centerYAnchor),
            cancelButton.heightAnchor.constraint(equalToConstant: 44),
            cancelButton.widthAnchor.constraint(equalToConstant: 120),

            confirmButton.centerYAnchor.constraint(equalTo: buttonContainer.centerYAnchor),
            confirmButton.heightAnchor.constraint(equalToConstant: 44),
            confirmButton.widthAnchor.constraint(equalToConstant: 120),

            // Center buttons horizontally with spacing
            cancelButton.trailingAnchor.constraint(equalTo: buttonContainer.centerXAnchor, constant: -10),
            confirmButton.leadingAnchor.constraint(equalTo: buttonContainer.centerXAnchor, constant: 10)
        ]

        constraints.forEach { $0.priority = UILayoutPriority(999) }
        NSLayoutConstraint.activate(constraints)

        window.layoutIfNeeded()
        containerView.transform = CGAffineTransform(translationX: 0, y: 400)

        UIView.animate(withDuration: 0.3, delay: 0, options: .curveEaseOut) {
            backgroundView.alpha = 1
            containerView.transform = .identity
        }
    }

    @MainActor
    internal static func dismissPicker(expectedID: UInt64? = nil) {
        var window: UIWindow? = currentWindow

        if window == nil {
            guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene else {
                os_log("No window scene found", log: LxAppPicker.log, type: .error)
                return
            }
            window = windowScene.windows.first(where: { $0.isKeyWindow }) ?? windowScene.windows.first
        }

        guard let window = window else {
            os_log("No window found for dismissing picker", log: LxAppPicker.log, type: .error)
            return
        }

        // Find the specific picker view by instance id when provided, otherwise fallback to current
        var targetView: UIView? = nil
        if let eid = expectedID {
            for v in window.subviews {
                if let idNum = objc_getAssociatedObject(v, "pickerInstanceID") as? NSNumber, idNum.uint64Value == eid {
                    targetView = v
                    break
                }
            }
        }
        if targetView == nil {
            targetView = LxAppPicker.backgroundView
        }

        guard let pickerView = targetView else {
            os_log("No picker view found to dismiss", log: LxAppPicker.log, type: .error)
            return
        }

        UIView.animate(withDuration: 0.3, animations: {
            pickerView.alpha = 0
            if let containerView = pickerView.subviews.first {
                containerView.transform = CGAffineTransform(translationX: 0, y: 400)
            }
        }) { _ in
            pickerView.removeFromSuperview()

            var shouldClear = false
            if let eid = expectedID, eid == LxAppPicker.activeInstanceID { shouldClear = true }
            if pickerView === LxAppPicker.backgroundView { shouldClear = true }
            if shouldClear {
                LxAppPicker.backgroundView = nil
                LxAppPicker.currentPickerData = nil
                LxAppPicker.currentWindow = nil
                LxAppPicker.currentColumnsStack = nil
            }
        }
    }

}

// Simple data source for UIPickerView
internal class SimplePickerData: NSObject, UIPickerViewDataSource, UIPickerViewDelegate {
    var configuration: PickerConfiguration
    var currentSelection: [Int]
    var callbackID: UInt64 = 0

    init(configuration: PickerConfiguration) {
        self.configuration = configuration
        self.currentSelection = Array(repeating: 0, count: configuration.columns.count)
        super.init()
    }

    func numberOfComponents(in pickerView: UIPickerView) -> Int {
        return configuration.columns.count
    }

    func pickerView(_ pickerView: UIPickerView, numberOfRowsInComponent component: Int) -> Int {
        guard component < configuration.columns.count else { return 0 }
        return configuration.columns[component].count
    }

    func pickerView(_ pickerView: UIPickerView, viewForRow row: Int, forComponent component: Int, reusing view: UIView?) -> UIView {
        guard component < configuration.columns.count,
              row < configuration.columns[component].count else {
            return UILabel()
        }

        let label = view as? UILabel ?? UILabel()
        label.text = configuration.columns[component][row]
        label.textAlignment = .center
        label.font = UIFont.systemFont(ofSize: 16, weight: .regular)
        label.textColor = UIColor(red: 0.2, green: 0.2, blue: 0.2, alpha: 1.0)
        label.backgroundColor = UIColor.clear
        label.numberOfLines = 1

        // Force fixed frame to prevent system from resizing
        let componentWidth = pickerView.frame.width / CGFloat(configuration.columns.count)
        label.frame = CGRect(x: 0, y: 0, width: max(componentWidth, 180), height: 44)
        label.translatesAutoresizingMaskIntoConstraints = false

        // Disable font scaling to prevent text truncation
        label.adjustsFontSizeToFitWidth = false
        label.lineBreakMode = .byClipping

        return label
    }

    func pickerView(_ pickerView: UIPickerView, widthForComponent component: Int) -> CGFloat {
        // Give each component equal width, ensuring enough space for text
        let totalWidth = pickerView.frame.width
        let componentCount = CGFloat(configuration.columns.count)
        let componentWidth = totalWidth / componentCount

        // Ensure minimum width for text display - increased to 200
        return max(componentWidth, 200.0)
    }

    func pickerView(_ pickerView: UIPickerView, rowHeightForComponent component: Int) -> CGFloat {
        return 44.0
    }

    func pickerView(_ pickerView: UIPickerView, didSelectRow row: Int, inComponent component: Int) {
        guard component < currentSelection.count else { return }
        currentSelection[component] = row
        pickerView.reloadComponent(component)

        // Handle cascading for dual column cascading picker
        if configuration.isCascading && component == 0 && configuration.columns.count == 2 {
            // Update second column based on first column selection
            if let cascadingData = configuration.cascadingData,
               row < configuration.columns[0].count {
                let selectedFirstColumnValue = configuration.columns[0][row]
                let newSecondColumnValues = cascadingData[selectedFirstColumnValue] ?? []

                // Update the configuration's second column
                var mutableColumns = configuration.columns
                mutableColumns[1] = newSecondColumnValues

                // Create new configuration with updated columns
                let newConfiguration = PickerConfiguration(
                    columns: mutableColumns,
                    cancelText: configuration.cancelText,
                    cancelButtonColor: configuration.cancelButtonColor,
                    cancelTextColor: configuration.cancelTextColor,
                    confirmText: configuration.confirmText,
                    confirmButtonColor: configuration.confirmButtonColor,
                    confirmTextColor: configuration.confirmTextColor,
                    isCascading: configuration.isCascading,
                    cascadingData: configuration.cascadingData
                )

                // Update our configuration
                self.configuration = newConfiguration

                currentSelection[1] = 0
                pickerView.selectRow(0, inComponent: 1, animated: true)
                pickerView.reloadComponent(1)
            }
        }
    }


}
#endif

// Configuration parsing
internal struct PickerConfiguration {
    let columns: [[String]]
    let cancelText: String
    let cancelButtonColor: String
    let cancelTextColor: String
    let confirmText: String
    let confirmButtonColor: String
    let confirmTextColor: String
    let isCascading: Bool
    let cascadingData: [String: [String]]? // Store the original cascading data

    static func parse(
        columns: String,
        cancelText: String,
        cancelButtonColor: String,
        cancelTextColor: String,
        confirmText: String,
        confirmButtonColor: String,
        confirmTextColor: String
    ) -> PickerConfiguration? {
        guard let data = columns.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) else {
            return nil
        }

        var parsedColumns: [[String]] = []
        var isCascading = false
        var cascadingData: [String: [String]]? = nil

        if let simpleArray = json as? [[String]] {
            // Simple format: [["col1"], ["col2"]]
            parsedColumns = simpleArray
        } else if let cascadingArray = json as? [Any], cascadingArray.count == 2 {
            // Cascading format: [["col1"], {"key1": ["sub1"], "key2": ["sub2"]}]
            if let firstColumn = cascadingArray[0] as? [String],
               let secondColumnDict = cascadingArray[1] as? [String: [String]] {
                parsedColumns.append(firstColumn)
                // For cascading, start with the first key's values
                let firstKey = firstColumn.first ?? ""
                let initialSecondColumn = secondColumnDict[firstKey] ?? []
                parsedColumns.append(initialSecondColumn)
                isCascading = true
                cascadingData = secondColumnDict
            }
        }

        guard !parsedColumns.isEmpty else { return nil }

        return PickerConfiguration(
            columns: parsedColumns,
            cancelText: cancelText,
            cancelButtonColor: cancelButtonColor,
            cancelTextColor: cancelTextColor,
            confirmText: confirmText,
            confirmButtonColor: confirmButtonColor,
            confirmTextColor: confirmTextColor,
            isCascading: isCascading,
            cascadingData: cascadingData
        )
    }
}

extension LxAppPicker {
    /// Create custom scroll picker container (like Android implementation)
    @MainActor
    internal static func createCustomPickerContainer(configuration: PickerConfiguration, callbackID: UInt64) -> UIView {
        let container = UIView()
        container.backgroundColor = UIColor.clear
        container.translatesAutoresizingMaskIntoConstraints = false

        // Create horizontal stack for columns
        let stackView = UIStackView()
        stackView.axis = .horizontal
        stackView.distribution = .fillEqually
        stackView.spacing = 0
        stackView.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(stackView)
        // Keep a reference for cascading updates
        LxAppPicker.currentColumnsStack = stackView

        // Add constraints for stack view
        NSLayoutConstraint.activate([
            stackView.topAnchor.constraint(equalTo: container.topAnchor),
            stackView.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            stackView.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            stackView.bottomAnchor.constraint(equalTo: container.bottomAnchor)
        ])

        // Create scroll pickers for each column
        for (columnIndex, columnItems) in configuration.columns.enumerated() {
            let scrollPicker = createCustomScrollPicker(
                items: columnItems,
                columnIndex: columnIndex,
                callbackID: callbackID
            )
            stackView.addArrangedSubview(scrollPicker)
        }

        return container
    }

    /// Create individual scroll picker for a column
    @MainActor
    internal static func createCustomScrollPicker(items: [String], columnIndex: Int, callbackID: UInt64) -> UIView {
        let container = UIView()
        container.backgroundColor = UIColor.clear
        container.tag = 100 + columnIndex // mark column index

        // Create scroll view
        let scrollView = UIScrollView()
        scrollView.backgroundColor = UIColor.clear
        scrollView.showsVerticalScrollIndicator = false
        scrollView.showsHorizontalScrollIndicator = false
        scrollView.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(scrollView)

        // Create content stack view
        let contentStack = UIStackView()
        contentStack.axis = .vertical
        contentStack.distribution = .fill
        contentStack.spacing = 0
        contentStack.translatesAutoresizingMaskIntoConstraints = false
        scrollView.addSubview(contentStack)

        // Add padding views at top and bottom for centering
        let itemHeight: CGFloat = 44
        let visibleItems = 5
        let paddingHeight = CGFloat(visibleItems / 2) * itemHeight

        let topPadding = UIView()
        topPadding.backgroundColor = UIColor.clear
        topPadding.translatesAutoresizingMaskIntoConstraints = false
        contentStack.addArrangedSubview(topPadding)

        // Add item labels
        for (index, item) in items.enumerated() {
            let label = UILabel()
            label.text = item
            label.textAlignment = .center
            label.font = UIFont.systemFont(ofSize: 18, weight: .regular)
            label.textColor = UIColor(red: 0.2, green: 0.2, blue: 0.2, alpha: 1.0)
            label.backgroundColor = UIColor.clear
            label.translatesAutoresizingMaskIntoConstraints = false

            // Add tap gesture
            let tapGesture = UITapGestureRecognizer(target: self, action: #selector(itemTapped(_:)))
            label.addGestureRecognizer(tapGesture)
            label.isUserInteractionEnabled = true
            label.tag = index

            contentStack.addArrangedSubview(label)

            // Set height constraint
            label.heightAnchor.constraint(equalToConstant: itemHeight).isActive = true
        }

        let bottomPadding = UIView()
        bottomPadding.backgroundColor = UIColor.clear
        bottomPadding.translatesAutoresizingMaskIntoConstraints = false
        contentStack.addArrangedSubview(bottomPadding)

        // Set constraints
        NSLayoutConstraint.activate([
            scrollView.topAnchor.constraint(equalTo: container.topAnchor),
            scrollView.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            scrollView.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            scrollView.bottomAnchor.constraint(equalTo: container.bottomAnchor),

            contentStack.topAnchor.constraint(equalTo: scrollView.topAnchor),
            contentStack.leadingAnchor.constraint(equalTo: scrollView.leadingAnchor),
            contentStack.trailingAnchor.constraint(equalTo: scrollView.trailingAnchor),
            contentStack.bottomAnchor.constraint(equalTo: scrollView.bottomAnchor),
            contentStack.widthAnchor.constraint(equalTo: scrollView.widthAnchor),

            topPadding.heightAnchor.constraint(equalToConstant: paddingHeight),
            bottomPadding.heightAnchor.constraint(equalToConstant: paddingHeight)
        ])

        // Add selection indicator overlay
        let selectionIndicator = UIView()
        selectionIndicator.backgroundColor = UIColor.clear // no shadow background
        selectionIndicator.isUserInteractionEnabled = false
        selectionIndicator.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(selectionIndicator)

        // Improve tap/scroll responsiveness
        scrollView.delaysContentTouches = false
        scrollView.alwaysBounceVertical = true
        let tapOnScroll = UITapGestureRecognizer(target: self, action: #selector(scrollViewTapped(_:)))
        scrollView.addGestureRecognizer(tapOnScroll)
        scrollView.tag = 200 + columnIndex

        // Add snapping delegate retained globally
        let delegate = ColumnScrollDelegate(columnIndex: columnIndex)
        scrollView.delegate = delegate
        LxAppPicker.scrollDelegates.append(delegate)

        NSLayoutConstraint.activate([
            selectionIndicator.centerYAnchor.constraint(equalTo: container.centerYAnchor),
            selectionIndicator.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            selectionIndicator.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            selectionIndicator.heightAnchor.constraint(equalToConstant: itemHeight)
        ])

        // Add separator lines at top/bottom of selection area
        let topLine = UIView(); topLine.translatesAutoresizingMaskIntoConstraints = false; topLine.backgroundColor = UIColor.systemGray3
        let bottomLine = UIView(); bottomLine.translatesAutoresizingMaskIntoConstraints = false; bottomLine.backgroundColor = UIColor.systemGray3
        container.addSubview(topLine); container.addSubview(bottomLine)
        NSLayoutConstraint.activate([
            topLine.heightAnchor.constraint(equalToConstant: 0.5),
            topLine.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 20),
            topLine.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -20),
            topLine.centerYAnchor.constraint(equalTo: container.centerYAnchor, constant: -itemHeight/2),

            bottomLine.heightAnchor.constraint(equalToConstant: 0.5),
            bottomLine.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 20),
            bottomLine.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -20),
            bottomLine.centerYAnchor.constraint(equalTo: container.centerYAnchor, constant: itemHeight/2)
        ])

        // Center initial row 0
        applySelection(columnIndex: columnIndex, selectedIndex: 0, scrollView: scrollView, emitScroll: false)
        return container
    }

    @MainActor
    internal static func resolveColor(_ value: String, fallback: UIColor) -> UIColor {
        guard value.hasPrefix("#") else { return fallback }
        let defaultArgb = LxAppColorUtils.argbValue(from: fallback.resolvedColor(with: UIScreen.main.traitCollection))
        let argb = LxAppColorUtils.parseColorString(value, defaultColor: defaultArgb)
        return LxAppColorUtils.platformColor(from: argb)
    }

    // Core selection application (used by label taps and scroll view taps)
    @MainActor
    internal static func applySelection(columnIndex: Int, selectedIndex: Int, scrollView: UIScrollView?, emitScroll: Bool = true) {
        guard let pickerData = LxAppPicker.currentPickerData else { return }
        while pickerData.currentSelection.count <= columnIndex { pickerData.currentSelection.append(0) }
        let clamped = max(0, min(selectedIndex, max(0, pickerData.configuration.columns[columnIndex].count - 1)))
        pickerData.currentSelection[columnIndex] = clamped

        // Cascading: update second column when first changes
        if columnIndex == 0, pickerData.configuration.isCascading,
           let cascadingMap = pickerData.configuration.cascadingData,
           clamped < pickerData.configuration.columns[0].count {
            let firstValue = pickerData.configuration.columns[0][clamped]
            let newSecond = cascadingMap[firstValue] ?? []
            var cols = pickerData.configuration.columns
            if cols.count >= 2 { cols[1] = newSecond }
            pickerData.configuration = PickerConfiguration(
                columns: cols,
                cancelText: pickerData.configuration.cancelText,
                cancelButtonColor: pickerData.configuration.cancelButtonColor,
                cancelTextColor: pickerData.configuration.cancelTextColor,
                confirmText: pickerData.configuration.confirmText,
                confirmButtonColor: pickerData.configuration.confirmButtonColor,
                confirmTextColor: pickerData.configuration.confirmTextColor,
                isCascading: pickerData.configuration.isCascading,
                cascadingData: pickerData.configuration.cascadingData
            )
            if pickerData.currentSelection.count >= 2 { pickerData.currentSelection[1] = 0 }
            if let stack = LxAppPicker.currentColumnsStack, stack.arrangedSubviews.count >= 2 {
                let oldSecond = stack.arrangedSubviews[1]
                stack.removeArrangedSubview(oldSecond); oldSecond.removeFromSuperview()
                let newSecondView = createCustomScrollPicker(
                    items: newSecond,
                    columnIndex: 1,
                    callbackID: pickerData.callbackID
                )
                stack.insertArrangedSubview(newSecondView, at: 1)
            }
        }

        // Scroll to center the selected item
        if let sv = scrollView {
            let itemHeight: CGFloat = 44
            let paddingHeight = CGFloat(5 / 2) * itemHeight
            let targetY = paddingHeight + CGFloat(clamped) * itemHeight - (sv.frame.height / 2) + (itemHeight / 2)
            sv.setContentOffset(CGPoint(x: 0, y: max(0, targetY)), animated: true)
            if emitScroll {
                LxAppPicker.sendPickerResultScroll(callback_id: pickerData.callbackID, selectedIndices: pickerData.currentSelection)
            }
        }
    }


    @objc @MainActor
    internal static func itemTapped(_ gesture: UITapGestureRecognizer) {
        guard let label = gesture.view as? UILabel else { return }
        let selectedIndex = label.tag
        var columnIndex = 0
        if let columnContainer = label.superview?.superview?.superview, columnContainer.tag >= 100 { columnIndex = columnContainer.tag - 100 }
        let scrollView = label.superview?.superview as? UIScrollView
        applySelection(columnIndex: columnIndex, selectedIndex: selectedIndex, scrollView: scrollView)
    }

    @objc @MainActor
    internal static func scrollViewTapped(_ gesture: UITapGestureRecognizer) {
        guard let sv = gesture.view as? UIScrollView else { return }
        var columnIndex = 0
        if sv.tag >= 200 { columnIndex = sv.tag - 200 }
        let itemHeight: CGFloat = 44
        let paddingHeight = CGFloat(5 / 2) * itemHeight
        let point = gesture.location(in: sv)
        let rawIndex = Int(round((point.y - paddingHeight) / itemHeight))
        let cols = LxAppPicker.currentPickerData?.configuration.columns ?? []
        let count = (columnIndex >= 0 && columnIndex < cols.count) ? cols[columnIndex].count : 0
        let clamped = max(0, min(rawIndex, max(0, count - 1)))
        applySelection(columnIndex: columnIndex, selectedIndex: clamped, scrollView: sv)
    }
}

// Snapping delegate for scroll-based picker columns
internal class ColumnScrollDelegate: NSObject, UIScrollViewDelegate {
    let columnIndex: Int
    init(columnIndex: Int) { self.columnIndex = columnIndex }

    private func snap(_ sv: UIScrollView) {
        let itemHeight: CGFloat = 44
        let paddingHeight = CGFloat(5 / 2) * itemHeight
        let midY = sv.contentOffset.y + sv.bounds.height / 2
        let raw = Int(round((midY - paddingHeight - itemHeight / 2) / itemHeight))
        let cols = LxAppPicker.currentPickerData?.configuration.columns ?? []
        let count = (columnIndex >= 0 && columnIndex < cols.count) ? cols[columnIndex].count : 0
        let clamped = max(0, min(raw, max(0, count - 1)))
        LxAppPicker.applySelection(columnIndex: columnIndex, selectedIndex: clamped, scrollView: sv)
    }

    func scrollViewDidEndDragging(_ scrollView: UIScrollView, willDecelerate decelerate: Bool) {
        if !decelerate { snap(scrollView) }
    }

    func scrollViewDidEndDecelerating(_ scrollView: UIScrollView) {
        snap(scrollView)
    }
}

private extension Array {
    subscript(safe index: Int) -> Element? {
        return indices.contains(index) ? self[index] : nil
    }
}
