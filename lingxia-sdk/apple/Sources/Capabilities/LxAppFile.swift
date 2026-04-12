#if os(iOS)
import Foundation
import QuickLook
import UIKit

@MainActor
enum LxAppFile {
    fileprivate static var previewCoordinator: IOSDocumentPreviewCoordinator?

    @discardableResult
    static func reviewDocument(path: String, mimeType: String?, showMenu: Bool = true) -> Bool {
        let fileURL = URL(fileURLWithPath: path)
        guard FileManager.default.fileExists(atPath: fileURL.path) else {
            return false
        }

        guard let presenter = LxApp.topViewController() else {
            return false
        }

        let coordinator = IOSDocumentPreviewCoordinator(fileURL: fileURL, showMenu: showMenu)
        previewCoordinator = coordinator
        return coordinator.present(from: presenter)
    }

    @discardableResult
    static func openExternal(path: String, mimeType: String?, showMenu: Bool = true) -> Bool {
        let fileURL = URL(fileURLWithPath: path)
        guard FileManager.default.fileExists(atPath: fileURL.path) else {
            return false
        }

        guard let presenter = LxApp.topViewController() else {
            return false
        }

        let controller = UIDocumentInteractionController(url: fileURL)
        if let mimeType, !mimeType.isEmpty {
            controller.uti = mimeType
        }
        // Hold a strong reference until the interaction finishes
        previewCoordinator = nil
        return controller.presentOpenInMenu(from: .zero, in: presenter.view, animated: true)
    }
}

@MainActor
private final class IOSDocumentPreviewCoordinator: NSObject, QLPreviewControllerDataSource, @preconcurrency QLPreviewControllerDelegate {
    private let fileURL: URL
    private let showMenu: Bool
    private weak var previewController: QLPreviewController?

    init(fileURL: URL, showMenu: Bool) {
        self.fileURL = fileURL
        self.showMenu = showMenu
    }

    func present(from presenter: UIViewController) -> Bool {
        let controller = QLPreviewController()
        controller.dataSource = self
        controller.delegate = self

        if !showMenu {
            controller.navigationItem.rightBarButtonItem = nil
        }

        controller.navigationItem.leftBarButtonItem = nil
        controller.navigationItem.rightBarButtonItem = UIBarButtonItem(
            barButtonSystemItem: .close,
            target: self,
            action: #selector(dismissPreview)
        )

        let navigationController = UINavigationController(rootViewController: controller)
        navigationController.modalPresentationStyle = .fullScreen
        if #available(iOS 13.0, *) {
            navigationController.isModalInPresentation = true
        }

        previewController = controller
        presenter.present(navigationController, animated: true)
        return true
    }

    func numberOfPreviewItems(in controller: QLPreviewController) -> Int {
        1
    }

    func previewController(_ controller: QLPreviewController, previewItemAt index: Int) -> QLPreviewItem {
        fileURL as NSURL
    }

    @available(iOS 13.0, *)
    func previewController(
        _ controller: QLPreviewController,
        editingModeFor previewItem: QLPreviewItem
    ) -> QLPreviewItemEditingMode {
        .disabled
    }

    func previewControllerDidDismiss(_ controller: QLPreviewController) {
        if LxAppFile.previewCoordinator === self {
            LxAppFile.previewCoordinator = nil
        }
    }

    @objc
    private func dismissPreview() {
        if let nav = previewController?.navigationController {
            nav.dismiss(animated: true)
        } else {
            previewController?.dismiss(animated: true)
        }
    }
}
#elseif os(macOS)
import AppKit
import Foundation
import Quartz

@MainActor
enum LxAppFile {
    static var qlController: MacDocumentQuickLookController?

    static func clearQLController(_ controller: MacDocumentQuickLookController? = nil) {
        guard controller == nil || qlController === controller else {
            return
        }
        qlController = nil
    }

    static func closeQLController() {
        qlController?.finish(shouldClosePanel: true)
    }

    @discardableResult
    static func reviewDocument(path: String, mimeType: String?, showMenu: Bool = true) -> Bool {
        let fileURL = URL(fileURLWithPath: path)
        guard FileManager.default.fileExists(atPath: fileURL.path) else {
            return false
        }

        let _ = (mimeType, showMenu)
        LxAppMedia.closeQLController()
        closeQLController()

        let controller = MacDocumentQuickLookController(fileURL: fileURL)
        guard controller.show() else {
            return false
        }
        qlController = controller
        return true
    }

    @discardableResult
    static func openExternal(path: String, mimeType: String?, showMenu: Bool = true) -> Bool {
        let fileURL = URL(fileURLWithPath: path)
        guard FileManager.default.fileExists(atPath: fileURL.path) else {
            return false
        }

        let _ = (mimeType, showMenu)
        return NSWorkspace.shared.open(fileURL)
    }
}

@MainActor
final class MacDocumentQuickLookController: NSObject, @preconcurrency QLPreviewPanelDataSource, @preconcurrency QLPreviewPanelDelegate {
    private let item: QLPreviewURL
    private var closeObserver: NSObjectProtocol?
    private var didFinish = false

    init(fileURL: URL) {
        self.item = QLPreviewURL(url: fileURL)
        super.init()
    }

    func show() -> Bool {
        guard let panel = QLPreviewPanel.shared() else {
            return false
        }
        panel.dataSource = self
        panel.delegate = self
        installCloseObserver(for: panel)
        panel.reloadData()
        panel.currentPreviewItemIndex = 0
        panel.makeKeyAndOrderFront(nil)
        return true
    }

    func finish(shouldClosePanel: Bool) {
        guard !didFinish else {
            return
        }
        didFinish = true

        let panel = QLPreviewPanel.shared()
        removeCloseObserver()
        panel?.delegate = nil
        panel?.dataSource = nil
        LxAppFile.clearQLController(self)

        if shouldClosePanel {
            panel?.orderOut(nil)
        }
    }

    private func installCloseObserver(for panel: QLPreviewPanel) {
        removeCloseObserver()
        closeObserver = NotificationCenter.default.addObserver(
            forName: NSWindow.willCloseNotification,
            object: panel,
            queue: nil
        ) { [weak self] _ in
            DispatchQueue.main.async {
                self?.finish(shouldClosePanel: false)
            }
        }
    }

    private func removeCloseObserver() {
        guard let closeObserver else {
            return
        }
        NotificationCenter.default.removeObserver(closeObserver)
        self.closeObserver = nil
    }

    func numberOfPreviewItems(in panel: QLPreviewPanel!) -> Int {
        1
    }

    func previewPanel(_ panel: QLPreviewPanel!, previewItemAt index: Int) -> (any QLPreviewItem)! {
        item
    }

    func previewPanel(_ panel: QLPreviewPanel!, handle event: NSEvent!) -> Bool {
        false
    }
}

private final class QLPreviewURL: NSObject, QLPreviewItem {
    let previewItemURL: URL?
    let previewItemTitle: String?

    init(url: URL) {
        self.previewItemURL = url
        self.previewItemTitle = url.lastPathComponent
        super.init()
    }
}
#endif
