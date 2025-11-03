#if os(iOS)
import Foundation
import QuickLook
import UIKit

@MainActor
public enum LxAppDocument {
    private static var previewCoordinator: DocumentPreviewCoordinator?

    @discardableResult
    public static func openDocument(path: String, mimeType: String?, showMenu: Bool = true) -> Bool {
        let fileURL = URL(fileURLWithPath: path)
        guard FileManager.default.fileExists(atPath: fileURL.path) else {
            return false
        }

        guard let presenter = LxApp.topViewController() else {
            return false
        }

        let coordinator = DocumentPreviewCoordinator(fileURL: fileURL, showMenu: showMenu)
        previewCoordinator = coordinator
        return coordinator.present(from: presenter)
    }

    @MainActor
    private final class DocumentPreviewCoordinator: NSObject, QLPreviewControllerDataSource, @preconcurrency QLPreviewControllerDelegate {
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

            // Configure toolbar based on showMenu setting
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
            return 1
        }

        func previewController(_ controller: QLPreviewController, previewItemAt index: Int) -> QLPreviewItem {
            return fileURL as NSURL
        }

        @available(iOS 13.0, *)
        func previewController(_ controller: QLPreviewController, editingModeFor previewItem: QLPreviewItem) -> QLPreviewItemEditingMode {
            return .disabled
        }

        func previewControllerDidDismiss(_ controller: QLPreviewController) {
            if LxAppDocument.previewCoordinator === self {
                LxAppDocument.previewCoordinator = nil
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
}
#endif
