import AppKit

/// Toolbar for simulator window - Xcode Simulator style
/// Floating toolbar with device selector and window controls
@MainActor
public class SimulatorToolbar: NSView {
    
    // MARK: - Layout Constants
    
    public struct Layout {
        public static let height: CGFloat = 32
        public static let cornerRadius: CGFloat = 8
        public static let buttonSize: CGFloat = 12
        public static let buttonSpacing: CGFloat = 8
        public static let sideMargin: CGFloat = 10
    }
    
    // MARK: - UI Components

    private var deviceSelector: NSPopUpButton!
    private var closeButton: NSButton!
    private var minimizeButton: NSButton!
    private var rotateButton: NSButton!
    private var inspectButton: NSButton!

    // MARK: - State

    public var onDeviceSelected: ((MobileDeviceSize) -> Void)?
    public var onCloseClicked: (() -> Void)?
    public var onRotateClicked: (() -> Void)?
    public var onInspectClicked: (() -> Void)?

    private var currentDevice: MobileDeviceSize = .defaultDevice
    
    // MARK: - Initialization
    
    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        setupUI()
    }
    
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
    
    // MARK: - Setup
    
    private func setupUI() {
        wantsLayer = true
        layer?.backgroundColor = NSColor(white: 0.18, alpha: 0.98).cgColor
        layer?.cornerRadius = Layout.cornerRadius
        
        // Subtle shadow
        shadow = NSShadow()
        layer?.shadowColor = NSColor.black.cgColor
        layer?.shadowOpacity = 0.3
        layer?.shadowOffset = CGSize(width: 0, height: -1)
        layer?.shadowRadius = 4
        
        setupWindowButtons()
        setupDeviceSelector()
        setupRotateButton()
        setupInspectButton()
    }
    
    private func setupWindowButtons() {
        // Close button (red)
        closeButton = createWindowButton(color: NSColor(red: 1.0, green: 0.38, blue: 0.35, alpha: 1.0))
        closeButton.action = #selector(closeClicked)
        addSubview(closeButton)
        
        // Minimize button (yellow)  
        minimizeButton = createWindowButton(color: NSColor(red: 1.0, green: 0.74, blue: 0.22, alpha: 1.0))
        minimizeButton.action = #selector(minimizeClicked)
        addSubview(minimizeButton)
        
        NSLayoutConstraint.activate([
            closeButton.leadingAnchor.constraint(equalTo: leadingAnchor, constant: Layout.sideMargin),
            closeButton.centerYAnchor.constraint(equalTo: centerYAnchor),
            closeButton.widthAnchor.constraint(equalToConstant: Layout.buttonSize),
            closeButton.heightAnchor.constraint(equalToConstant: Layout.buttonSize),
            
            minimizeButton.leadingAnchor.constraint(equalTo: closeButton.trailingAnchor, constant: Layout.buttonSpacing),
            minimizeButton.centerYAnchor.constraint(equalTo: centerYAnchor),
            minimizeButton.widthAnchor.constraint(equalToConstant: Layout.buttonSize),
            minimizeButton.heightAnchor.constraint(equalToConstant: Layout.buttonSize)
        ])
    }
    
    private func createWindowButton(color: NSColor) -> NSButton {
        let button = NSButton()
        button.translatesAutoresizingMaskIntoConstraints = false
        button.isBordered = false
        button.title = ""
        button.wantsLayer = true
        button.layer?.backgroundColor = color.cgColor
        button.layer?.cornerRadius = Layout.buttonSize / 2
        button.target = self
        return button
    }
    
    private func setupDeviceSelector() {
        deviceSelector = NSPopUpButton()
        deviceSelector.translatesAutoresizingMaskIntoConstraints = false
        deviceSelector.bezelStyle = .texturedRounded
        deviceSelector.isBordered = false
        deviceSelector.font = NSFont.systemFont(ofSize: 12, weight: .medium)
        deviceSelector.target = self
        deviceSelector.action = #selector(deviceSelectionChanged)
        deviceSelector.contentTintColor = NSColor.white.withAlphaComponent(0.9)
        
        if let cell = deviceSelector.cell as? NSPopUpButtonCell {
            cell.arrowPosition = .arrowAtBottom
        }
        
        let devices = MobileDeviceSize.allCases
        var previousShape: RunnerDeviceShape?
        for device in devices {
            if let previousShape, previousShape != device.shape {
                deviceSelector.menu?.addItem(.separator())
            }
            let menuItem = NSMenuItem()
            menuItem.title = device.displayName
            menuItem.representedObject = device
            deviceSelector.menu?.addItem(menuItem)
            previousShape = device.shape
        }
        
        selectDevice(currentDevice)
        
        addSubview(deviceSelector)
        
        NSLayoutConstraint.activate([
            deviceSelector.centerXAnchor.constraint(equalTo: centerXAnchor),
            deviceSelector.centerYAnchor.constraint(equalTo: centerYAnchor)
        ])
    }
    
    private func setupInspectButton() {
        inspectButton = NSButton()
        let config = NSImage.SymbolConfiguration(pointSize: 13, weight: .semibold)
        let image = NSImage(systemSymbolName: "gearshape", accessibilityDescription: "DevTools")?
            .withSymbolConfiguration(config)
        inspectButton.image = image
        inspectButton.imagePosition = .imageOnly
        inspectButton.title = ""
        inspectButton.isBordered = false
        inspectButton.bezelStyle = .regularSquare
        inspectButton.target = self
        inspectButton.action = #selector(inspectClicked)
        inspectButton.contentTintColor = NSColor.white.withAlphaComponent(0.7)
        inspectButton.toolTip = "Toggle DevTools"
        inspectButton.translatesAutoresizingMaskIntoConstraints = false
        addSubview(inspectButton)

        NSLayoutConstraint.activate([
            inspectButton.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -Layout.sideMargin),
            inspectButton.centerYAnchor.constraint(equalTo: centerYAnchor),
            rotateButton.trailingAnchor.constraint(equalTo: inspectButton.leadingAnchor, constant: -6),
            rotateButton.centerYAnchor.constraint(equalTo: centerYAnchor),
            rotateButton.widthAnchor.constraint(equalToConstant: 24),
            rotateButton.heightAnchor.constraint(equalToConstant: 24),
        ])
    }

    private func setupRotateButton() {
        rotateButton = NSButton()
        let config = NSImage.SymbolConfiguration(pointSize: 13, weight: .semibold)
        let image = NSImage(systemSymbolName: "rotate.right", accessibilityDescription: "Rotate")?
            .withSymbolConfiguration(config)
        rotateButton.image = image
        rotateButton.imagePosition = .imageOnly
        rotateButton.title = ""
        rotateButton.isBordered = false
        rotateButton.bezelStyle = .regularSquare
        rotateButton.target = self
        rotateButton.action = #selector(rotateClicked)
        rotateButton.contentTintColor = NSColor.white.withAlphaComponent(0.7)
        rotateButton.toolTip = "Rotate device"
        rotateButton.translatesAutoresizingMaskIntoConstraints = false
        addSubview(rotateButton)
    }

    // MARK: - Actions

    @objc private func inspectClicked() {
        onInspectClicked?()
    }

    @objc private func rotateClicked() {
        onRotateClicked?()
    }

    @objc private func closeClicked() {
        window?.close()
    }
    
    @objc private func minimizeClicked() {
        window?.miniaturize(nil)
    }
    
    @objc private func deviceSelectionChanged() {
        guard let selectedItem = deviceSelector.selectedItem,
              let device = selectedItem.representedObject as? MobileDeviceSize else { return }
        
        currentDevice = device
        onDeviceSelected?(device)
    }
    
    // MARK: - Public API
    
    public func setCurrentDevice(_ device: MobileDeviceSize) {
        currentDevice = device
        selectDevice(device)
        rotateButton?.isEnabled = device.supportsOrientation
        rotateButton?.contentTintColor = device.supportsOrientation
            ? NSColor.white.withAlphaComponent(0.7)
            : NSColor.white.withAlphaComponent(0.25)
    }

    private func selectDevice(_ device: MobileDeviceSize) {
        guard let item = deviceSelector.itemArray.first(where: {
            ($0.representedObject as? MobileDeviceSize)?.id == device.id
        }) else {
            return
        }
        deviceSelector.select(item)
    }
}
