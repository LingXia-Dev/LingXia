import Foundation
#if os(iOS)
import UIKit
import CLingXiaRustAPI

/// UIKit pickers for GPU page controls: an action sheet for `<select>`, a date or time
/// picker for editors whose value is picked. The choice goes back to the core.
extension GpuPageView {
    private func rect(_ node: GpuFrameNode) -> CGRect {
        let b = node.visibleBounds.count == 4 ? node.visibleBounds : node.bounds
        guard b.count == 4 else { return .zero }
        return CGRect(x: CGFloat(b[0]), y: CGFloat(b[1]), width: CGFloat(b[2]), height: CGFloat(b[3]))
    }

    private var presenter: UIViewController? {
        var controller = window?.rootViewController
        while let presented = controller?.presentedViewController { controller = presented }
        return controller
    }

    func openPicker(generation: UInt64, node: GpuFrameNode) {
        guard let select = node.select, !select.options.isEmpty, let presenter else { return }
        let sheet = UIAlertController(title: nil, message: nil, preferredStyle: .actionSheet)
        for (index, option) in select.options.enumerated() {
            let action = UIAlertAction(title: option.label, style: .default) { [weak self] _ in
                guard let self else { return }
                self.imeRequest(.select(identity: node.identity, generation: generation, index: UInt64(index))) { [weak self] reply in
                    self?.imeRender(rebuild: reply?.needsViewRender == true)
                }
            }
            action.isEnabled = option.enabled
            if select.selected == UInt64(index) { action.setValue(true, forKey: "checked") }
            sheet.addAction(action)
        }
        sheet.addAction(UIAlertAction(title: NSLocalizedString("Cancel", comment: ""), style: .cancel))
        sheet.popoverPresentationController?.sourceView = self
        sheet.popoverPresentationController?.sourceRect = rect(node)
        presenter.present(sheet, animated: true)
    }

    /// A connected editor whose value comes from a picker shows it in place of the keyboard.
    func showValuePicker() {
        guard let editor = ime.editor, let spec = ime.ime?.picker, let node = ime.node, let presenter else { return }
        let time = spec.kind == GpuFramePickerKind.time.rawValue
        let calendar = Calendar(identifier: .gregorian)
        func date(_ parts: [UInt32]?) -> Date? {
            guard let parts, parts.count == 3 else { return nil }
            var components = DateComponents()
            if time {
                (components.hour, components.minute) = (Int(parts[0]), Int(parts[1]))
                (components.year, components.month, components.day) = (2000, 1, 1)
            } else {
                (components.year, components.month, components.day) = (Int(parts[0]), Int(parts[1]), Int(parts[2]))
            }
            return calendar.date(from: components)
        }
        let picker = UIDatePicker()
        picker.datePickerMode = time ? .time : .date
        picker.preferredDatePickerStyle = time ? .wheels : .inline
        picker.calendar = calendar
        picker.date = date(spec.seed) ?? Date()
        picker.minimumDate = date(spec.minParts)
        picker.maximumDate = date(spec.maxParts)

        let controller = GpuValuePickerController(picker: picker) { [weak self, weak editor] chosen in
            guard let self else { return }
            self.valuePicker = nil
            guard let editor, self.ime.editor === editor else { return }
            guard let chosen else { return self.ime.pick(nil) }
            let parts = calendar.dateComponents([.year, .month, .day, .hour, .minute], from: chosen)
            self.ime.pick(time
                ? [UInt32(parts.hour ?? 0), UInt32(parts.minute ?? 0), 0]
                : [UInt32(parts.year ?? 0), UInt32(parts.month ?? 1), UInt32(parts.day ?? 1)])
        }
        controller.modalPresentationStyle = .popover
        controller.popoverPresentationController?.sourceView = self
        controller.popoverPresentationController?.sourceRect = rect(node)
        controller.popoverPresentationController?.delegate = controller
        valuePicker = controller
        presenter.present(controller, animated: true)
    }
}

/// A date or time picker with Done; closing it any other way is a dismissal.
@MainActor
final class GpuValuePickerController: UIViewController, UIPopoverPresentationControllerDelegate {
    private let picker: UIDatePicker
    private var outcome: ((Date?) -> Void)?

    init(picker: UIDatePicker, outcome: @escaping (Date?) -> Void) {
        self.picker = picker
        self.outcome = outcome
        super.init(nibName: nil, bundle: nil)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is unavailable") }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground
        let done = UIButton(type: .system, primaryAction: UIAction(title: NSLocalizedString("Done", comment: "")) { [weak self] _ in
            guard let self else { return }
            self.finish(self.picker.date)
            self.dismiss(animated: true)
        })
        let stack = UIStackView(arrangedSubviews: [picker, done])
        stack.axis = .vertical
        stack.spacing = 8
        stack.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 12),
            stack.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 12),
            stack.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -12),
            stack.bottomAnchor.constraint(lessThanOrEqualTo: view.safeAreaLayoutGuide.bottomAnchor, constant: -12),
        ])
        preferredContentSize = stack.systemLayoutSizeFitting(UIView.layoutFittingCompressedSize)
    }

    /// A phone shows the popover as a sheet.
    func adaptivePresentationStyle(for controller: UIPresentationController, traitCollection: UITraitCollection) -> UIModalPresentationStyle {
        .automatic
    }

    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        finish(nil)
    }

    func close() { dismiss(animated: true) }

    private func finish(_ date: Date?) {
        let outcome = self.outcome
        self.outcome = nil
        outcome?(date)
    }
}
#endif
