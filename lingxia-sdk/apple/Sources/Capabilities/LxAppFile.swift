#if os(iOS)
import Foundation
import QuickLook
import UIKit
import UniformTypeIdentifiers
import CLingXiaRustAPI

private func allFileContentTypes() -> [UTType] {
    [.item, .data]
}

@MainActor
enum LxAppFile {
    fileprivate static var previewCoordinator: IOSDocumentPreviewCoordinator?
    fileprivate static var pickerCoordinator: IOSDocumentPickerCoordinator?
    fileprivate static var localChooserCoordinator: IOSLocalFileChooserCoordinator?
    private static var securityScopedFileURLs: [String: URL] = [:]

    @discardableResult
    static func reviewDocument(path: String, mimeType: String?, showMenu: Bool = true) -> Bool {
        let fileURL = URL(fileURLWithPath: path)
        return withSecurityScopedAccess(path: path) {
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
    }

    @discardableResult
    static func openExternal(path: String, mimeType: String?, showMenu: Bool = true) -> Bool {
        let fileURL = URL(fileURLWithPath: path)
        return withSecurityScopedAccess(path: path) {
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

    static func withSecurityScopedAccess<T>(path: String, _ body: () -> T) -> T {
        guard let url = securityScopedFileURLs[path] else {
            return body()
        }
        let accessed = url.startAccessingSecurityScopedResource()
        defer {
            if accessed {
                url.stopAccessingSecurityScopedResource()
            }
        }
        return body()
    }

    fileprivate static func registerSecurityScopedURL(_ url: URL) {
        securityScopedFileURLs[url.path] = url
    }

    @discardableResult
    static func chooseFile(
        title: String,
        defaultPath: String,
        multiple: Bool,
        filtersJson: String,
        callbackId: UInt64
    ) -> Bool {
        guard let presenter = LxApp.topViewController() else {
            let _ = onCallback(callbackId, false, "1000")
            return false
        }

        let filterSpec = IOSLocalFileFilterSpec(filtersJson: filtersJson)
        if !multiple, let rootURL = resolveLocalChooserRoot(defaultPath) {
            let coordinator = IOSLocalFileChooserCoordinator(
                mode: .file(filterSpec: filterSpec),
                rootURL: rootURL,
                callbackId: callbackId
            )
            localChooserCoordinator = coordinator
            return coordinator.present(
                from: presenter,
                title: title.isEmpty ? L10n.string("lx_file_chooser_default_title") : title
            )
        }

        let coordinator = IOSDocumentPickerCoordinator(
            mode: .file(
                multiple: multiple,
                contentTypes: filterSpec.contentTypes
            ),
            callbackId: callbackId
        )
        pickerCoordinator = coordinator
        return coordinator.present(from: presenter, title: title, defaultPath: defaultPath)
    }

    @discardableResult
    static func chooseDirectory(
        title: String,
        defaultPath: String,
        callbackId: UInt64
    ) -> Bool {
        guard let presenter = LxApp.topViewController() else {
            let _ = onCallback(callbackId, false, "1000")
            return false
        }

        if let rootURL = resolveLocalChooserRoot(defaultPath) {
            let coordinator = IOSLocalFileChooserCoordinator(
                mode: .directory,
                rootURL: rootURL,
                callbackId: callbackId
            )
            localChooserCoordinator = coordinator
            return coordinator.present(
                from: presenter,
                title: title.isEmpty ? L10n.string("lx_file_chooser_default_title") : title
            )
        }

        let coordinator = IOSDocumentPickerCoordinator(
            mode: .directory,
            callbackId: callbackId
        )
        pickerCoordinator = coordinator
        return coordinator.present(from: presenter, title: title, defaultPath: defaultPath)
    }

    fileprivate static func clearPickerCoordinator(_ coordinator: IOSDocumentPickerCoordinator? = nil) {
        guard coordinator == nil || pickerCoordinator === coordinator else {
            return
        }
        pickerCoordinator = nil
    }

    fileprivate static func clearLocalChooserCoordinator(_ coordinator: IOSLocalFileChooserCoordinator? = nil) {
        guard coordinator == nil || localChooserCoordinator === coordinator else {
            return
        }
        localChooserCoordinator = nil
    }
}

private enum IOSDocumentPickerMode {
    case file(multiple: Bool, contentTypes: [UTType])
    case directory
}

private struct IOSLocalFileFilterSpec {
    let extensions: Set<String>
    let exactMimeTypes: Set<String>
    let wildcardMimeGroups: Set<String>
    let contentTypes: [UTType]
    let labels: [String]

    init(filtersJson: String) {
        let trimmed = filtersJson.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty,
              let data = trimmed.data(using: .utf8),
              let values = try? JSONSerialization.jsonObject(with: data) as? [String] else {
            self.extensions = []
            self.exactMimeTypes = []
            self.wildcardMimeGroups = []
            self.contentTypes = allFileContentTypes()
            self.labels = []
            return
        }

        var extensions = Set<String>()
        var exactMimeTypes = Set<String>()
        var wildcardMimeGroups = Set<String>()
        var contentTypes: [UTType] = []
        var labels: [String] = []

        for rawValue in values {
            let value = rawValue.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !value.isEmpty else { continue }
            if value.contains("/") {
                let normalized = value.lowercased()
                if normalized.hasSuffix("/*") {
                    wildcardMimeGroups.insert(String(normalized.split(separator: "/").first ?? ""))
                } else {
                    exactMimeTypes.insert(normalized)
                }
                if let type = UTType(mimeType: value) {
                    contentTypes.append(type)
                }
                labels.append(Self.filterLabel(for: normalized))
            } else {
                let ext = value.trimmingCharacters(in: CharacterSet(charactersIn: ".")).lowercased()
                guard !ext.isEmpty else { continue }
                extensions.insert(ext)
                if let type = UTType(filenameExtension: ext) {
                    contentTypes.append(type)
                }
                labels.append(ext.uppercased())
            }
        }

        self.extensions = extensions
        self.exactMimeTypes = exactMimeTypes
        self.wildcardMimeGroups = wildcardMimeGroups
        self.contentTypes = contentTypes.isEmpty ? allFileContentTypes() : Array(Set(contentTypes))
        self.labels = Array(NSOrderedSet(array: labels)) as? [String] ?? labels
    }

    var isEmpty: Bool {
        extensions.isEmpty && exactMimeTypes.isEmpty && wildcardMimeGroups.isEmpty
    }

    func matches(fileURL: URL) -> Bool {
        guard !isEmpty else { return true }
        let ext = fileURL.pathExtension.lowercased()
        if !ext.isEmpty && extensions.contains(ext) {
            return true
        }
        guard !ext.isEmpty,
              let mimeType = UTType(filenameExtension: ext)?.preferredMIMEType?.lowercased() else {
            return false
        }
        if exactMimeTypes.contains(mimeType) {
            return true
        }
        let group = mimeType.split(separator: "/").first.map(String.init) ?? ""
        return !group.isEmpty && wildcardMimeGroups.contains(group)
    }

    private static func filterLabel(for value: String) -> String {
        switch value {
        case "image/*":
            return L10n.string("lx_file_chooser_filter_images")
        case "video/*":
            return L10n.string("lx_file_chooser_filter_videos")
        case "audio/*":
            return L10n.string("lx_file_chooser_filter_audio")
        default:
            return value.split(separator: "/").last.map(String.init)?.uppercased() ?? value.uppercased()
        }
    }
}

@MainActor
private final class IOSDocumentPickerCoordinator: NSObject, UIDocumentPickerDelegate {
    private let mode: IOSDocumentPickerMode
    private let callbackId: UInt64
    private weak var picker: UIDocumentPickerViewController?
    private var didFinish = false

    init(mode: IOSDocumentPickerMode, callbackId: UInt64) {
        self.mode = mode
        self.callbackId = callbackId
    }

    func present(from presenter: UIViewController, title: String, defaultPath: String) -> Bool {
        let controller: UIDocumentPickerViewController
        switch mode {
        case .file(let multiple, let contentTypes):
            controller = UIDocumentPickerViewController(
                forOpeningContentTypes: contentTypes.isEmpty ? allFileContentTypes() : contentTypes,
                asCopy: true
            )
            controller.allowsMultipleSelection = multiple
        case .directory:
            controller = UIDocumentPickerViewController(
                forOpeningContentTypes: [.folder],
                asCopy: false
            )
            controller.allowsMultipleSelection = false
        }

        controller.delegate = self
        controller.modalPresentationStyle = .formSheet
        applyDefaultPath(defaultPath, to: controller)
        picker = controller
        presenter.present(controller, animated: true)
        let _ = title
        return true
    }

    func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
        emitPayload(canceled: true, paths: [])
        finish()
    }

    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        guard !didFinish else {
            return
        }

        let picked = urls
        if picked.isEmpty {
            emitPayload(canceled: true, paths: [])
            finish()
            return
        }

        do {
            let selectedPaths = try picked.map { try registerPickedItem(from: $0) }
            emitPayload(canceled: selectedPaths.isEmpty, paths: selectedPaths)
        } catch {
            let _ = onCallback(callbackId, false, "1000")
        }
        finish()
    }

    private func emitPayload(canceled: Bool, paths: [String]) {
        let payload: [String: Any] = [
            "canceled": canceled,
            "paths": paths,
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: payload, options: []),
              let json = String(data: data, encoding: .utf8) else {
            let _ = onCallback(callbackId, false, "1000")
            return
        }
        let _ = onCallback(callbackId, true, json)
    }

    private func applyDefaultPath(_ defaultPath: String, to controller: UIDocumentPickerViewController) {
        let trimmed = defaultPath.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            return
        }
        let candidate = URL(fileURLWithPath: trimmed)
        var isDirectory: ObjCBool = false
        if FileManager.default.fileExists(atPath: candidate.path, isDirectory: &isDirectory) {
            controller.directoryURL = isDirectory.boolValue
                ? candidate
                : candidate.deletingLastPathComponent()
        }
    }

    private func registerPickedItem(from sourceURL: URL) throws -> String {
        guard sourceURL.isFileURL else {
            throw NSError(domain: "LxAppFile", code: 1000, userInfo: nil)
        }

        let accessed = sourceURL.startAccessingSecurityScopedResource()
        defer {
            if accessed {
                sourceURL.stopAccessingSecurityScopedResource()
            }
        }
        var isDirectory: ObjCBool = false
        let exists = FileManager.default.fileExists(atPath: sourceURL.path, isDirectory: &isDirectory)
        guard exists else {
            throw NSError(domain: "LxAppFile", code: 1000, userInfo: nil)
        }

        LxAppFile.registerSecurityScopedURL(sourceURL)
        switch mode {
        case .file:
            return sourceURL.path
        case .directory:
            return sourceURL.path
        }
    }

    private func sanitizedName(_ name: String, fallback: String) -> String {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty {
            return fallback
        }
        let invalid = CharacterSet(charactersIn: "/:\\")
        let parts = trimmed.components(separatedBy: invalid).filter { !$0.isEmpty }
        let joined = parts.joined(separator: "-")
        return joined.isEmpty ? fallback : joined
    }

    private func finish() {
        guard !didFinish else {
            return
        }
        didFinish = true
        picker?.delegate = nil
        picker = nil
        LxAppFile.clearPickerCoordinator(self)
    }
}

private enum IOSLocalChooserMode {
    case file(filterSpec: IOSLocalFileFilterSpec)
    case directory
}

@MainActor
private final class IOSLocalFileChooserCoordinator: NSObject {
    private let mode: IOSLocalChooserMode
    private let rootURL: URL
    private let callbackId: UInt64
    private weak var navigationController: UINavigationController?
    private var didFinish = false

    init(mode: IOSLocalChooserMode, rootURL: URL, callbackId: UInt64) {
        self.mode = mode
        self.rootURL = rootURL
        self.callbackId = callbackId
    }

    func present(from presenter: UIViewController, title: String) -> Bool {
        let controller = IOSLocalFileChooserViewController(
            titleText: title,
            rootURL: rootURL,
            mode: mode,
            onCancel: { [weak self] in
                self?.emitPayload(canceled: true, paths: [])
                self?.finish()
            },
            onSelect: { [weak self] url in
                guard let self else { return }
                do {
                    let resolvedPath = try self.prepareSelectedPath(url)
                    self.emitPayload(canceled: false, paths: [resolvedPath])
                } catch {
                    let _ = onCallback(self.callbackId, false, "1000")
                }
                self.finish()
            }
        )
        let nav = UINavigationController(rootViewController: controller)
        nav.modalPresentationStyle = UIModalPresentationStyle.fullScreen
        navigationController = nav
        presenter.present(nav, animated: true)
        return true
    }

    fileprivate func emitPayload(canceled: Bool, paths: [String]) {
        let payload: [String: Any] = [
            "canceled": canceled,
            "paths": paths,
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: payload, options: []),
              let json = String(data: data, encoding: .utf8) else {
            let _ = onCallback(callbackId, false, "1000")
            return
        }
        let _ = onCallback(callbackId, true, json)
    }

    private func prepareSelectedPath(_ url: URL) throws -> String {
        switch mode {
        case .directory:
            return url.path
        case .file:
            return url.path
        }
    }

    fileprivate func finish() {
        guard !didFinish else { return }
        didFinish = true
        navigationController?.dismiss(animated: true)
        navigationController = nil
        LxAppFile.clearLocalChooserCoordinator(self)
    }
}

@MainActor
private final class IOSLocalFileChooserViewController: UITableViewController {
    private struct Entry {
        let url: URL
        let isDirectory: Bool
    }

    private let titleText: String
    private let rootURL: URL
    private let mode: IOSLocalChooserMode
    private let onCancel: () -> Void
    private let onSelect: (URL) -> Void
    private var currentURL: URL
    private var entries: [Entry] = []
    private var revealedDeletePath: String?

    init(
        titleText: String,
        rootURL: URL,
        mode: IOSLocalChooserMode,
        onCancel: @escaping () -> Void,
        onSelect: @escaping (URL) -> Void
    ) {
        self.titleText = titleText
        self.rootURL = rootURL
        self.mode = mode
        self.onCancel = onCancel
        self.onSelect = onSelect
        self.currentURL = rootURL
        super.init(style: .insetGrouped)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        title = titleText
        tableView.register(UITableViewCell.self, forCellReuseIdentifier: "entry")
        tableView.separatorInset = UIEdgeInsets(top: 0, left: 58, bottom: 0, right: 0)
        navigationItem.backButtonDisplayMode = .minimal
        if currentURL == rootURL {
            navigationItem.leftBarButtonItem = UIBarButtonItem(
                image: UIImage(systemName: "chevron.backward"),
                style: .plain,
                target: self,
                action: #selector(cancelTapped)
            )
        }
        if case .directory = mode {
            navigationItem.rightBarButtonItem = UIBarButtonItem(
                title: L10n.string("lx_common_done"),
                style: .done,
                target: self,
                action: #selector(confirmDirectory)
            )
        }
        applyFilterHeader()
        reloadEntries()
    }

    private func applyFilterHeader() {
        guard case .file(let filterSpec) = mode, !filterSpec.labels.isEmpty else {
            tableView.tableHeaderView = UIView(frame: CGRect(x: 0, y: 0, width: tableView.bounds.width, height: 8))
            return
        }

        let summary = filterSpec.labels.prefix(3).joined(separator: " · ")
        let extraCount = max(0, filterSpec.labels.count - 3)
        let summaryText = extraCount > 0 ? "\(summary) +\(extraCount)" : summary

        let width = max(tableView.bounds.width, view.bounds.width)
        let container = UIView(frame: CGRect(x: 0, y: 0, width: width, height: 52))
        container.backgroundColor = .clear

        let chipBackground = UIView(frame: CGRect(x: 16, y: 8, width: max(0, width - 32), height: 32))
        chipBackground.autoresizingMask = [.flexibleWidth]
        chipBackground.backgroundColor = UIColor.systemBlue.withAlphaComponent(0.08)
        chipBackground.layer.cornerRadius = 12

        let label = UILabel(frame: CGRect(x: 12, y: 0, width: max(0, chipBackground.bounds.width - 24), height: chipBackground.bounds.height))
        label.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        label.font = .systemFont(ofSize: 12, weight: .semibold)
        label.textColor = .systemBlue
        label.text = L10n.string("lx_file_chooser_filter_label").replacingOccurrences(of: "%1$s", with: summaryText)

        chipBackground.addSubview(label)
        container.addSubview(chipBackground)
        tableView.tableHeaderView = container
    }

    private func reloadEntries() {
        let urls = (try? FileManager.default.contentsOfDirectory(
            at: currentURL,
            includingPropertiesForKeys: [.isDirectoryKey],
            options: [.skipsHiddenFiles]
        )) ?? []

        let sorted = urls.sorted { lhs, rhs in
            let lhsDir = (try? lhs.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) ?? false
            let rhsDir = (try? rhs.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) ?? false
            if lhsDir != rhsDir {
                return lhsDir && !rhsDir
            }
            return lhs.lastPathComponent.localizedCaseInsensitiveCompare(rhs.lastPathComponent) == .orderedAscending
        }

        entries = sorted.compactMap { url in
            let isDirectory = (try? url.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) ?? false
            if !isDirectory, case .file(let filterSpec) = mode, !filterSpec.matches(fileURL: url) {
                return nil
            }
            return Entry(url: url, isDirectory: isDirectory)
        }
        updateEmptyState(allURLs: sorted)
        tableView.reloadData()
    }

    private func updateEmptyState(allURLs: [URL]) {
        guard entries.isEmpty else {
            tableView.backgroundView = nil
            tableView.separatorStyle = .singleLine
            return
        }

        let titleText: String
        let subtitleText: String
        switch mode {
        case .directory:
            titleText = L10n.string("lx_file_chooser_empty_title")
            subtitleText = L10n.string("lx_file_chooser_empty_subtitle")
        case .file(let filterSpec):
            let hasReadableFiles = allURLs.contains { url in
                ((try? url.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) ?? false) == false
            }
            if !filterSpec.isEmpty && hasReadableFiles {
                titleText = L10n.string("lx_file_chooser_filtered_empty_title")
                subtitleText = L10n.string("lx_file_chooser_filtered_empty_subtitle")
            } else {
                titleText = L10n.string("lx_file_chooser_empty_title")
                subtitleText = L10n.string("lx_file_chooser_empty_subtitle")
            }
        }

        let container = UIView()
        let stack = UIStackView()
        stack.axis = .vertical
        stack.alignment = .center
        stack.spacing = 8
        stack.translatesAutoresizingMaskIntoConstraints = false

        let titleLabel = UILabel()
        titleLabel.text = titleText
        titleLabel.font = .boldSystemFont(ofSize: 20)
        titleLabel.textColor = .label
        titleLabel.textAlignment = .center
        titleLabel.numberOfLines = 0

        let subtitleLabel = UILabel()
        subtitleLabel.text = subtitleText
        subtitleLabel.font = .systemFont(ofSize: 14)
        subtitleLabel.textColor = .secondaryLabel
        subtitleLabel.textAlignment = .center
        subtitleLabel.numberOfLines = 0

        stack.addArrangedSubview(titleLabel)
        stack.addArrangedSubview(subtitleLabel)
        container.addSubview(stack)

        NSLayoutConstraint.activate([
            stack.centerXAnchor.constraint(equalTo: container.centerXAnchor),
            stack.centerYAnchor.constraint(equalTo: container.centerYAnchor),
            stack.leadingAnchor.constraint(greaterThanOrEqualTo: container.leadingAnchor, constant: 24),
            stack.trailingAnchor.constraint(lessThanOrEqualTo: container.trailingAnchor, constant: -24),
        ])

        tableView.backgroundView = container
        tableView.separatorStyle = .none
    }

    override func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int {
        entries.count
    }

    override func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
        let cell = UITableViewCell(style: .subtitle, reuseIdentifier: "entry")
        let entry = entries[indexPath.row]
        var content = cell.defaultContentConfiguration()
        content.text = entry.url.lastPathComponent
        content.secondaryText = subtitle(for: entry)
        content.image = UIImage(systemName: entry.isDirectory ? "folder.fill" : "doc.fill")
        content.imageProperties.tintColor = entry.isDirectory ? .systemBlue : .secondaryLabel
        content.imageProperties.maximumSize = CGSize(width: 22, height: 22)
        content.textProperties.font = .systemFont(ofSize: 17, weight: .medium)
        content.secondaryTextProperties.color = .secondaryLabel
        cell.contentConfiguration = content
        cell.accessoryType = entry.isDirectory && revealedDeletePath != entry.url.path ? .disclosureIndicator : .none
        cell.selectionStyle = .default
        if revealedDeletePath == entry.url.path {
            let deleteButton = UIButton(type: .system)
            deleteButton.setTitle(L10n.string("lx_common_delete"), for: .normal)
            deleteButton.setTitleColor(.systemRed, for: .normal)
            deleteButton.titleLabel?.font = .boldSystemFont(ofSize: 14)
            deleteButton.frame = CGRect(x: 0, y: 0, width: 56, height: 28)
            deleteButton.addAction(UIAction { [weak self] _ in
                self?.deleteEntry(entry)
            }, for: .touchUpInside)
            cell.accessoryView = deleteButton
        } else {
            cell.accessoryView = nil
        }
        return cell
    }

    override func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) {
        tableView.deselectRow(at: indexPath, animated: true)
        if revealedDeletePath != nil {
            return
        }
        let entry = entries[indexPath.row]
        if entry.isDirectory {
            let next = IOSLocalFileChooserViewController(
                titleText: titleText,
                rootURL: rootURL,
                mode: mode,
                onCancel: onCancel,
                onSelect: onSelect
            )
            next.currentURL = entry.url
            navigationController?.pushViewController(next, animated: true)
            return
        }

        if case .file = mode {
            onSelect(entry.url)
        }
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        if tableView.gestureRecognizers?.contains(where: { $0 is UILongPressGestureRecognizer }) != true {
            let recognizer = UILongPressGestureRecognizer(target: self, action: #selector(handleLongPress(_:)))
            recognizer.minimumPressDuration = 0.5
            tableView.addGestureRecognizer(recognizer)
        }
    }

    @objc
    private func cancelTapped() {
        if revealedDeletePath != nil {
            revealedDeletePath = nil
            tableView.reloadData()
            return
        }
        onCancel()
    }

    @objc
    private func confirmDirectory() {
        onSelect(currentURL)
    }

    private func deleteEntry(_ entry: Entry) {
        do {
            guard entry.url.path.hasPrefix(rootURL.path) else {
                return
            }
            try FileManager.default.removeItem(at: entry.url)
            revealedDeletePath = nil
            reloadEntries()
        } catch {
            revealedDeletePath = nil
            reloadEntries()
        }
    }

    @objc
    private func handleLongPress(_ recognizer: UILongPressGestureRecognizer) {
        guard recognizer.state == .began else { return }
        let location = recognizer.location(in: tableView)
        guard let indexPath = tableView.indexPathForRow(at: location) else { return }
        let entry = entries[indexPath.row]
        revealedDeletePath = entry.url.path
        tableView.reloadData()
    }

    private func subtitle(for entry: Entry) -> String {
        if entry.isDirectory {
            return L10n.string("lx_file_chooser_folder_subtitle")
        }

        let attributes = try? FileManager.default.attributesOfItem(atPath: entry.url.path)
        let fileSize = (attributes?[.size] as? NSNumber)?.int64Value ?? 0
        let date = attributes?[.modificationDate] as? Date
        let sizeText = ByteCountFormatter.string(fromByteCount: fileSize, countStyle: .file)
        if let date {
            let relative = RelativeDateTimeFormatter().localizedString(for: date, relativeTo: Date())
            return "\(sizeText) · \(relative)"
        }
        return sizeText
    }
}

private func parseDocumentContentTypes(_ filtersJson: String) -> [UTType] {
    guard !filtersJson.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
          let data = filtersJson.data(using: .utf8),
          let values = try? JSONSerialization.jsonObject(with: data) as? [String] else {
        return allFileContentTypes()
    }

    let contentTypes = values.compactMap { raw -> UTType? in
        let value = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !value.isEmpty else {
            return nil
        }
        if value.contains("/") {
            return UTType(mimeType: value)
        }
        let ext = value.trimmingCharacters(in: CharacterSet(charactersIn: "."))
        return ext.isEmpty ? nil : UTType(filenameExtension: ext)
    }

    return contentTypes.isEmpty ? allFileContentTypes() : Array(Set(contentTypes))
}

private func resolveLocalChooserRoot(_ defaultPath: String) -> URL? {
    let trimmed = defaultPath.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty else {
        return nil
    }

    let candidate = URL(fileURLWithPath: trimmed)
    var isDirectory: ObjCBool = false
    guard FileManager.default.fileExists(atPath: candidate.path, isDirectory: &isDirectory) else {
        return nil
    }
    return isDirectory.boolValue ? candidate : candidate.deletingLastPathComponent()
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
import UniformTypeIdentifiers
import CLingXiaRustAPI

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

    static func withSecurityScopedAccess<T>(path: String, _ body: () -> T) -> T {
        let _ = path
        return body()
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

    @discardableResult
    static func chooseFile(
        title: String,
        defaultPath: String,
        multiple: Bool,
        filtersJson: String,
        callbackId: UInt64
    ) -> Bool {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = multiple
        panel.resolvesAliases = true
        if !title.isEmpty {
            panel.title = title
        }
        applyDefaultPath(defaultPath, to: panel)
        let contentTypes = parseDocumentContentTypesForMac(filtersJson)
        if !contentTypes.isEmpty, !(contentTypes.count == 1 && contentTypes.first == .item) {
            panel.allowedContentTypes = contentTypes
        }

        guard panel.runModal() == .OK else {
            let _ = onCallback(callbackId, true, "{\"canceled\":true,\"paths\":[]}")
            return true
        }

        let paths = panel.urls.map(\.path)
        guard let data = try? JSONSerialization.data(withJSONObject: [
            "canceled": paths.isEmpty,
            "paths": paths,
        ]),
        let json = String(data: data, encoding: .utf8) else {
            let _ = onCallback(callbackId, false, "1000")
            return false
        }
        let _ = onCallback(callbackId, true, json)
        return true
    }

    @discardableResult
    static func chooseDirectory(
        title: String,
        defaultPath: String,
        callbackId: UInt64
    ) -> Bool {
        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false
        panel.resolvesAliases = true
        if !title.isEmpty {
            panel.title = title
        }
        applyDefaultPath(defaultPath, to: panel)

        guard panel.runModal() == .OK else {
            let _ = onCallback(callbackId, true, "{\"canceled\":true,\"paths\":[]}")
            return true
        }

        let paths = panel.urls.map(\.path)
        guard let data = try? JSONSerialization.data(withJSONObject: [
            "canceled": paths.isEmpty,
            "paths": paths,
        ]),
        let json = String(data: data, encoding: .utf8) else {
            let _ = onCallback(callbackId, false, "1000")
            return false
        }
        let _ = onCallback(callbackId, true, json)
        return true
    }

    private static func applyDefaultPath(_ defaultPath: String, to panel: NSOpenPanel) {
        let trimmed = defaultPath.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            return
        }
        let candidate = URL(fileURLWithPath: trimmed)
        var isDirectory: ObjCBool = false
        if FileManager.default.fileExists(atPath: candidate.path, isDirectory: &isDirectory) {
            panel.directoryURL = isDirectory.boolValue ? candidate : candidate.deletingLastPathComponent()
        }
    }

    private static func parseDocumentContentTypesForMac(_ filtersJson: String) -> [UTType] {
        guard !filtersJson.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              let data = filtersJson.data(using: .utf8),
              let values = try? JSONSerialization.jsonObject(with: data) as? [String] else {
            return [.item]
        }

        let contentTypes = values.compactMap { raw -> UTType? in
            let value = raw.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !value.isEmpty else { return nil }
            if value.contains("/") {
                return UTType(mimeType: value)
            }
            let ext = value.trimmingCharacters(in: CharacterSet(charactersIn: "."))
            return ext.isEmpty ? nil : UTType(filenameExtension: ext)
        }

        return contentTypes.isEmpty ? [.item] : Array(Set(contentTypes))
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
