// iOS custom media picker (images/videos/mix) with Limited-access UX
// - Shows authorized assets only (PhotoKit fetch result reflects Limited set)
// - In Limited mode, appends a trailing '+' cell to open system "Choose More Photos"
// - Returns phasset:<localIdentifier> entries via onCallback

#if os(iOS)
import UIKit
@preconcurrency import Photos
import CLingXiaRustAPI

@MainActor
final class MediaPickerViewController: UIViewController, UICollectionViewDataSource, UICollectionViewDelegate, UITableViewDataSource, UITableViewDelegate {
    private let mode: String
    private let maxCount: Int
    private let callbackId: UInt64

    private var assets: [PHAsset] = []
    private var selected: Set<String> = [] // localIdentifiers
    private var isOriginal: Bool = false

    private let imageManager = PHCachingImageManager()
    private var collectionView: UICollectionView!
    private var bottomBar: UIView!
    private var warningBar: UIView?
    private var originalOption: RadioOptionView!
    private var countLabel: UILabel!
    private var doneButton: UIButton!

    private var isLimited: Bool {
        if #available(iOS 14.0, *) {
            return PHPhotoLibrary.authorizationStatus(for: .readWrite) == .limited
        }
        return false
    }

    private var authStatus: PHAuthorizationStatus {
        if #available(iOS 14.0, *) { return PHPhotoLibrary.authorizationStatus(for: .readWrite) }
        return .authorized
    }

    private var showPlus: Bool {
        // Only show plus cell in limited mode, not in other restricted states
        return isLimited
    }


    private func refreshByAuthState() {
        switch authStatus {
        case .authorized, .limited:
            hideAuthOverlay()
            loadAlbums()
            fetchAssetsAndReload()
        case .notDetermined:
            // Don't show overlay immediately, let user interact first
            break
        case .denied, .restricted:
            showAuthOverlay()
        @unknown default:
            showAuthOverlay()
        }
    }

    private func showAuthOverlay() {
        if authOverlay != nil { return }
        let overlay = UIView()
        overlay.backgroundColor = .white
        view.addSubview(overlay)
        overlay.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            overlay.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            overlay.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            overlay.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            overlay.bottomAnchor.constraint(equalTo: bottomBar.topAnchor)
        ])

        let label = UILabel()
        label.text = "选择照片以继续"
        label.textColor = .darkGray
        label.font = UIFont.systemFont(ofSize: 16)
        label.textAlignment = .center
        overlay.addSubview(label)
        label.translatesAutoresizingMaskIntoConstraints = false

        let button = UIButton(type: .system)
        button.setTitle("选择照片", for: .normal)
        button.setTitleColor(.white, for: .normal)
        button.backgroundColor = techBlue
        button.layer.cornerRadius = 8
        button.contentEdgeInsets = UIEdgeInsets(top: 10, left: 20, bottom: 10, right: 20)
        overlay.addSubview(button)
        button.translatesAutoresizingMaskIntoConstraints = false

        NSLayoutConstraint.activate([
            label.centerXAnchor.constraint(equalTo: overlay.centerXAnchor),
            label.centerYAnchor.constraint(equalTo: overlay.centerYAnchor, constant: -16),
            button.centerXAnchor.constraint(equalTo: overlay.centerXAnchor),
            button.topAnchor.constraint(equalTo: label.bottomAnchor, constant: 12)
        ])

        button.addTarget(self, action: #selector(onAuthButton), for: .touchUpInside)
        authOverlay = overlay
    }

    private func hideAuthOverlay() {
        authOverlay?.removeFromSuperview()
        authOverlay = nil
    }

    @objc private func onAuthButton() {
        if #available(iOS 14.0, *) {
            PHPhotoLibrary.requestAuthorization(for: .readWrite) { [weak self] _ in
                DispatchQueue.main.async { self?.refreshByAuthState() }
            }
        }
    }

    init(mode: String, maxCount: Int, callbackId: UInt64) {
        self.mode = mode.lowercased()
        self.maxCount = max(1, maxCount)
        self.callbackId = callbackId
        super.init(nibName: nil, bundle: nil)
        if #available(iOS 14.0, *) {
            PHPhotoLibrary.shared().register(self)
        }
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    deinit {
        if #available(iOS 14.0, *) {
            PHPhotoLibrary.shared().unregisterChangeObserver(self)
        }
    }

    static func present(from parent: UIViewController, mode: String, maxCount: UInt32, callbackId: UInt64) {
        let picker = MediaPickerViewController(mode: mode, maxCount: Int(maxCount), callbackId: callbackId)
        let nav = UINavigationController(rootViewController: picker)
        nav.modalPresentationStyle = .fullScreen
        parent.present(nav, animated: true)
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .white
        setupNav()
        setupTitleDropdown()
        setupCollection()
        setupBottomBar()
        setupWarningBar()
        refreshByAuthState()
    }

    private var techBlue: UIColor { UIColor(red: 0.09, green: 0.47, blue: 1.0, alpha: 1.0) } // #1677FF

    private func rootAlbumTitle() -> String {
        switch mode {
        case "video", "videos":
            return isLimited ? "可访问的视频" : "所有视频"
        case "image", "images":
            return isLimited ? "可访问的照片" : "所有照片"
        default:
            return isLimited ? "可访问的媒体" : "所有媒体"
        }
    }

    private func plusCellHintText() -> String {
        switch mode {
        case "video", "videos": return "添加更多\n可访问视频"
        case "image", "images": return "添加更多\n可访问照片"
        default: return "添加更多\n可访问媒体"
        }
    }

    private func setupNav() {
        if #available(iOS 13.0, *) {
            navigationItem.leftBarButtonItem = UIBarButtonItem(barButtonSystemItem: .close, target: self, action: #selector(onCancel))
        } else {
            navigationItem.leftBarButtonItem = UIBarButtonItem(title: "×", style: .plain, target: self, action: #selector(onCancel))
        }
        navigationController?.navigationBar.tintColor = techBlue
    }

    private var titleButton: UIView!
    private var titleLabel: UILabel!
    private var arrowView: ArrowView!
    private var arrowCircle: UIView!
    private var albumMenuView: UIView?
    private var albums: [(title: String, collection: PHAssetCollection?, count: Int)] = []
    private var currentAlbum: PHAssetCollection? = nil
    private var authOverlay: UIView?

    private func setupTitleDropdown() {
        let container = UIView()
        container.backgroundColor = UIColor(white: 0.9, alpha: 1)
        container.layer.cornerRadius = 15
        container.clipsToBounds = true

        titleLabel = UILabel()
        let title = currentAlbum?.localizedTitle ?? rootAlbumTitle()
        titleLabel.text = title
        titleLabel.textColor = .black
        titleLabel.font = UIFont.systemFont(ofSize: 15, weight: .medium)
        container.addSubview(titleLabel)
        titleLabel.translatesAutoresizingMaskIntoConstraints = false

        arrowCircle = UIView()
        arrowCircle.layer.cornerRadius = 9
        arrowCircle.clipsToBounds = true
        container.addSubview(arrowCircle)
        arrowCircle.translatesAutoresizingMaskIntoConstraints = false
        if #available(iOS 13.0, *) {
            arrowCircle.overrideUserInterfaceStyle = .light
        }

        arrowView = ArrowView()
        // Use neutral/dynamic color for chevron (not blue)
        if #available(iOS 13.0, *) {
            arrowView.tintColor = .secondaryLabel
        } else {
            arrowView.tintColor = UIColor(white: 0.35, alpha: 1)
        }
        arrowCircle.addSubview(arrowView)
        arrowView.translatesAutoresizingMaskIntoConstraints = false
        if #available(iOS 13.0, *) {
            arrowView.overrideUserInterfaceStyle = .light
        }
        applyArrowIndicatorStyle()

        NSLayoutConstraint.activate([
            container.widthAnchor.constraint(lessThanOrEqualToConstant: 200),
            container.heightAnchor.constraint(equalToConstant: 30),

            titleLabel.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 12),
            titleLabel.centerYAnchor.constraint(equalTo: container.centerYAnchor),

            arrowCircle.leadingAnchor.constraint(equalTo: titleLabel.trailingAnchor, constant: 6),
            arrowCircle.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -6),
            arrowCircle.centerYAnchor.constraint(equalTo: container.centerYAnchor),
            arrowCircle.widthAnchor.constraint(equalToConstant: 18),
            arrowCircle.heightAnchor.constraint(equalToConstant: 18),

            arrowView.topAnchor.constraint(equalTo: arrowCircle.topAnchor),
            arrowView.leadingAnchor.constraint(equalTo: arrowCircle.leadingAnchor),
            arrowView.trailingAnchor.constraint(equalTo: arrowCircle.trailingAnchor),
            arrowView.bottomAnchor.constraint(equalTo: arrowCircle.bottomAnchor)
        ])

        let tap = UITapGestureRecognizer(target: self, action: #selector(toggleAlbumMenu))
        container.addGestureRecognizer(tap)
        container.isUserInteractionEnabled = true

        titleButton = container
        navigationItem.titleView = container

        loadAlbums()
    }

    private func loadAlbums() {
        albums.removeAll()

        // Fetch all photos count
        let options = PHFetchOptions()
        switch mode {
        case "video", "videos":
            options.predicate = NSPredicate(format: "mediaType == %d", PHAssetMediaType.video.rawValue)
        case "image", "images":
            options.predicate = NSPredicate(format: "mediaType == %d", PHAssetMediaType.image.rawValue)
        default:
            break
        }
        let firstAlbumTitle = rootAlbumTitle()
        let allCount = PHAsset.fetchAssets(with: options).count
        albums.append((title: firstAlbumTitle, collection: nil, count: allCount))

        // Fetch user albums
        let userAlbums = PHAssetCollection.fetchAssetCollections(with: .album, subtype: .albumRegular, options: nil)
        userAlbums.enumerateObjects { c, _, _ in
            let count = PHAsset.fetchAssets(in: c, options: options).count
            if count > 0 {
                self.albums.append((title: c.localizedTitle ?? "相册", collection: c, count: count))
            }
        }

        titleLabel?.text = currentAlbum?.localizedTitle ?? firstAlbumTitle
    }

    @objc private func toggleAlbumMenu() {
        if let menu = albumMenuView {
            menu.removeFromSuperview()
            albumMenuView = nil
            // Rotate arrow back
            UIView.animate(withDuration: 0.2) {
                self.arrowView.transform = .identity
            }
            return
        }

        let overlay = UIView(frame: view.bounds)
        overlay.backgroundColor = UIColor(white: 0, alpha: 0.7)

        let table = UITableView(frame: .zero, style: .plain)
        table.backgroundColor = UIColor(white: 0.15, alpha: 1)
        table.separatorColor = UIColor(white: 0.3, alpha: 1)
        table.layer.cornerRadius = 8
        table.clipsToBounds = true
        table.rowHeight = 44
        table.dataSource = self
        table.delegate = self
        table.register(UITableViewCell.self, forCellReuseIdentifier: "album")
        overlay.addSubview(table)
        view.addSubview(overlay)
        overlay.translatesAutoresizingMaskIntoConstraints = false
        table.translatesAutoresizingMaskIntoConstraints = false

        // Calculate dynamic height based on album count (44pt per row)
        let rowHeight: CGFloat = 44
        let contentHeight = CGFloat(albums.count) * rowHeight
        let maxHeight = view.bounds.height * 0.6
        let tableHeight = min(contentHeight, maxHeight)

        NSLayoutConstraint.activate([
            overlay.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            overlay.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            overlay.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            overlay.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            table.topAnchor.constraint(equalTo: overlay.topAnchor, constant: 8),
            table.leadingAnchor.constraint(equalTo: overlay.leadingAnchor, constant: 12),
            table.trailingAnchor.constraint(equalTo: overlay.trailingAnchor, constant: -12),
            table.heightAnchor.constraint(equalToConstant: tableHeight)
        ])
        let tap = UITapGestureRecognizer(target: self, action: #selector(toggleAlbumMenu))
        tap.cancelsTouchesInView = false
        overlay.addGestureRecognizer(tap)
        albumMenuView = overlay

        // Refresh circle/arrow appearance when menu is shown
        applyArrowIndicatorStyle()

        // Rotate arrow 180 degrees
        UIView.animate(withDuration: 0.2) {
            self.arrowView.transform = CGAffineTransform(rotationAngle: .pi)
        }
    }

    private func applyArrowIndicatorStyle() {
        guard let arrowCircle else { return }
        // Let ArrowView paint its own filled circle; keep container clear to avoid blend issues
        arrowCircle.backgroundColor = .clear
        arrowCircle.isOpaque = false
        arrowCircle.layer.borderWidth = 0
        arrowCircle.layer.borderColor = nil
        arrowView.setNeedsDisplay()
    }

    private func setupCollection() {
        let layout = UICollectionViewFlowLayout()
        let spacing: CGFloat = 2
        let columns: CGFloat = 4
        let w = (view.bounds.width - (columns - 1) * spacing) / columns
        layout.itemSize = CGSize(width: floor(w), height: floor(w))
        layout.minimumLineSpacing = spacing
        layout.minimumInteritemSpacing = spacing
        collectionView = UICollectionView(frame: .zero, collectionViewLayout: layout)
        collectionView.backgroundColor = UIColor(white: 0.98, alpha: 1)
        collectionView.dataSource = self
        collectionView.delegate = self
        collectionView.alwaysBounceVertical = true
        collectionView.register(MediaCell.self, forCellWithReuseIdentifier: "cell")
        collectionView.register(PlusCell.self, forCellWithReuseIdentifier: "plus")
        view.addSubview(collectionView)
        collectionView.translatesAutoresizingMaskIntoConstraints = false

        // Bottom offset accounts for bottomBar (56) + warningBar (44 if limited, 0 otherwise)
        let bottomOffset = isLimited ? -100 : -56
        NSLayoutConstraint.activate([
            collectionView.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            collectionView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            collectionView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            collectionView.bottomAnchor.constraint(equalTo: view.bottomAnchor, constant: CGFloat(bottomOffset))
        ])
    }

    private func setupBottomBar() {
        bottomBar = UIView()
        bottomBar.backgroundColor = .white
        bottomBar.layer.shadowColor = UIColor.black.cgColor
        bottomBar.layer.shadowOpacity = 0.08
        bottomBar.layer.shadowRadius = 6
        view.addSubview(bottomBar)
        bottomBar.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            bottomBar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            bottomBar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            bottomBar.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor),
            bottomBar.heightAnchor.constraint(equalToConstant: 56)
        ])

        originalOption = RadioOptionView(title: "原图", color: techBlue)
        originalOption.isOn = isOriginal
        originalOption.addTarget(self, action: #selector(originalChanged), for: .valueChanged)
        bottomBar.addSubview(originalOption)
        originalOption.translatesAutoresizingMaskIntoConstraints = false

        countLabel = UILabel()
        countLabel.textColor = techBlue
        countLabel.font = UIFont.systemFont(ofSize: 14, weight: .medium)
        bottomBar.addSubview(countLabel)
        countLabel.translatesAutoresizingMaskIntoConstraints = false

        doneButton = UIButton(type: .system)
        doneButton.setTitle("完成", for: .normal)
        doneButton.setTitleColor(.white, for: .normal)
        doneButton.backgroundColor = techBlue
        doneButton.layer.cornerRadius = 18
        doneButton.contentEdgeInsets = UIEdgeInsets(top: 6, left: 16, bottom: 6, right: 16)
        doneButton.addTarget(self, action: #selector(onDone), for: .touchUpInside)
        bottomBar.addSubview(doneButton)
        doneButton.translatesAutoresizingMaskIntoConstraints = false

        // Hide "原图" option for video mode
        let isVideoMode = mode == "video" || mode == "videos"
        originalOption.isHidden = isVideoMode

        NSLayoutConstraint.activate([
            countLabel.leadingAnchor.constraint(equalTo: bottomBar.leadingAnchor, constant: 16),
            countLabel.centerYAnchor.constraint(equalTo: bottomBar.centerYAnchor),

            originalOption.centerXAnchor.constraint(equalTo: bottomBar.centerXAnchor),
            originalOption.centerYAnchor.constraint(equalTo: bottomBar.centerYAnchor),

            doneButton.trailingAnchor.constraint(equalTo: bottomBar.trailingAnchor, constant: -16),
            doneButton.centerYAnchor.constraint(equalTo: bottomBar.centerYAnchor),
            doneButton.heightAnchor.constraint(equalToConstant: 36)
        ])
        updateDoneLabel()
    }

    private func setupWarningBar() {
        guard isLimited else { return }

        let bar = UIView()
        bar.backgroundColor = UIColor(red: 1.0, green: 0.96, blue: 0.8, alpha: 1.0) // Light yellow
        view.insertSubview(bar, belowSubview: bottomBar)
        bar.translatesAutoresizingMaskIntoConstraints = false

        let iconBg = UIView()
        iconBg.backgroundColor = UIColor(red: 1.0, green: 0.8, blue: 0.0, alpha: 1.0)
        iconBg.layer.cornerRadius = 10
        bar.addSubview(iconBg)
        iconBg.translatesAutoresizingMaskIntoConstraints = false

        let icon = UILabel()
        icon.text = "!"
        icon.textColor = .white
        icon.font = UIFont.boldSystemFont(ofSize: 14)
        icon.textAlignment = .center
        iconBg.addSubview(icon)
        icon.translatesAutoresizingMaskIntoConstraints = false

        let label = UILabel()
        label.text = "你仅开启有限访问相册权限，建议允许访问「所有照片」"
        label.font = UIFont.systemFont(ofSize: 12)
        label.textColor = UIColor.darkGray
        label.numberOfLines = 2
        bar.addSubview(label)
        label.translatesAutoresizingMaskIntoConstraints = false

        let arrowLabel = UILabel()
        arrowLabel.text = ">"
        arrowLabel.font = UIFont.systemFont(ofSize: 14)
        arrowLabel.textColor = UIColor.lightGray
        bar.addSubview(arrowLabel)
        arrowLabel.translatesAutoresizingMaskIntoConstraints = false

        NSLayoutConstraint.activate([
            bar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            bar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            bar.bottomAnchor.constraint(equalTo: bottomBar.topAnchor),
            bar.heightAnchor.constraint(equalToConstant: 44),

            iconBg.leadingAnchor.constraint(equalTo: bar.leadingAnchor, constant: 12),
            iconBg.centerYAnchor.constraint(equalTo: bar.centerYAnchor),
            iconBg.widthAnchor.constraint(equalToConstant: 20),
            iconBg.heightAnchor.constraint(equalToConstant: 20),

            icon.centerXAnchor.constraint(equalTo: iconBg.centerXAnchor),
            icon.centerYAnchor.constraint(equalTo: iconBg.centerYAnchor),

            label.leadingAnchor.constraint(equalTo: iconBg.trailingAnchor, constant: 8),
            label.trailingAnchor.constraint(equalTo: arrowLabel.leadingAnchor, constant: -4),
            label.centerYAnchor.constraint(equalTo: bar.centerYAnchor),

            arrowLabel.trailingAnchor.constraint(equalTo: bar.trailingAnchor, constant: -12),
            arrowLabel.centerYAnchor.constraint(equalTo: bar.centerYAnchor)
        ])

        let tap = UITapGestureRecognizer(target: self, action: #selector(onWarningTap))
        bar.addGestureRecognizer(tap)
        warningBar = bar
    }

    @objc private func onWarningTap() {
        if let url = URL(string: UIApplication.openSettingsURLString) {
            UIApplication.shared.open(url)
        }
    }


    private func updateDoneLabel() {
        let count = selected.count
        countLabel.text = "已选 \(count)/\(maxCount)"
        doneButton.isEnabled = count > 0
        doneButton.alpha = count > 0 ? 1.0 : 0.6
    }


    private func fetchAssetsAndReload() {
        let options = PHFetchOptions()
        options.sortDescriptors = [NSSortDescriptor(key: "creationDate", ascending: false)]
        switch mode {
        case "video", "videos": options.predicate = NSPredicate(format: "mediaType == %d", PHAssetMediaType.video.rawValue)
        case "image", "images": options.predicate = NSPredicate(format: "mediaType == %d", PHAssetMediaType.image.rawValue)
        default: break // mix
        }
        let result: PHFetchResult<PHAsset>
        if let c = currentAlbum {
            result = PHAsset.fetchAssets(in: c, options: options)
        } else {
            result = PHAsset.fetchAssets(with: options)
        }
        var list: [PHAsset] = []
        result.enumerateObjects { a, _, _ in list.append(a) }
        self.assets = list
        self.collectionView?.reloadData()
    }

    @objc private func onCancel() {
        let _ = onCallback(callbackId, true, "{\"cancel\":true}")
        dismiss(animated: true)
    }

    @objc private func originalChanged() { isOriginal = originalOption.isOn }

    @objc private func onDone() {
        var arr: [[String: Any]] = []
        for id in selected {
            let type: String
            if let asset = assets.first(where: { $0.localIdentifier == id }) {
                type = (asset.mediaType == .video) ? "video" : "image"
            } else { type = "image" }
            arr.append([
                "uri": "phasset:\(id)",
                "fileType": type,
                "isOriginal": isOriginal
            ])
        }
        do {
            let data = try JSONSerialization.data(withJSONObject: arr, options: [])
            let json = String(data: data, encoding: .utf8) ?? "[]"
            let _ = onCallback(callbackId, true, json)
        } catch {
            let _ = onCallback(callbackId, false, "Failed to serialize selection")
        }
        dismiss(animated: true)
    }

    func numberOfSections(in collectionView: UICollectionView) -> Int { 1 }
    func collectionView(_ collectionView: UICollectionView, numberOfItemsInSection section: Int) -> Int {
        return assets.count + (showPlus ? 1 : 0)
    }

    func collectionView(_ collectionView: UICollectionView, cellForItemAt indexPath: IndexPath) -> UICollectionViewCell {
        if isLimited && indexPath.item == assets.count {
            let cell = collectionView.dequeueReusableCell(withReuseIdentifier: "plus", for: indexPath) as! PlusCell
            cell.configure(color: techBlue, hintText: plusCellHintText())
            return cell
        }
        let asset = assets[indexPath.item]
        let cell = collectionView.dequeueReusableCell(withReuseIdentifier: "cell", for: indexPath) as! MediaCell
        cell.setSelected(selected.contains(asset.localIdentifier), accent: techBlue)
        let target = (collectionView.collectionViewLayout as? UICollectionViewFlowLayout)?.itemSize ?? CGSize(width: 80, height: 80)
        let opts = PHImageRequestOptions()
        opts.deliveryMode = .opportunistic
        opts.resizeMode = .fast
        imageManager.requestImage(for: asset, targetSize: target, contentMode: .aspectFill, options: opts) { img, _ in
            cell.imageView.image = img
            cell.badgeView.isHidden = asset.mediaType != .video
        }
        return cell
    }

    func collectionView(_ collectionView: UICollectionView, didSelectItemAt indexPath: IndexPath) {
        if showPlus && indexPath.item == assets.count {
            if #available(iOS 14.0, *) {
                switch authStatus {
                case .notDetermined:
                    PHPhotoLibrary.requestAuthorization(for: .readWrite) { [weak self] newStatus in
                        DispatchQueue.main.async {
                            guard let self else { return }
                            switch newStatus {
                            case .authorized:
                                self.fetchAssetsAndReload()
                            case .limited:
                                PHPhotoLibrary.shared().presentLimitedLibraryPicker(from: self)
                            case .denied, .restricted:
                                if let url = URL(string: UIApplication.openSettingsURLString) { UIApplication.shared.open(url) }
                            default:
                                break
                            }
                        }
                    }
                case .limited:
                    PHPhotoLibrary.shared().presentLimitedLibraryPicker(from: self)
                case .denied, .restricted:
                    if let url = URL(string: UIApplication.openSettingsURLString) { UIApplication.shared.open(url) }
                default:
                    break
                }
            }
            return
        }
        let asset = assets[indexPath.item]
        let id = asset.localIdentifier
        if selected.contains(id) {
            selected.remove(id)
        } else {
            if selected.count >= maxCount {
                // remove oldest selected
                if let first = selected.first { selected.remove(first) }
            }
            selected.insert(id)
        }
        updateDoneLabel()
        collectionView.reloadItems(at: [indexPath])
    }

    func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int {
        return albums.count
    }

    func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
        let cell = tableView.dequeueReusableCell(withIdentifier: "album", for: indexPath)
        let album = albums[indexPath.row]
        cell.textLabel?.text = "\(album.title) (\(album.count))"
        cell.textLabel?.textColor = .white
        cell.textLabel?.font = UIFont.systemFont(ofSize: 15)
        cell.backgroundColor = .clear
        cell.selectionStyle = .none

        // Show checkmark for selected album
        let isSelected = (album.collection == currentAlbum) || (album.collection == nil && currentAlbum == nil)

        // Remove any existing accessory view
        cell.accessoryView = nil

        if isSelected {
            // Create custom blue checkmark (matching Android)
            let checkmarkView = UIImageView(frame: CGRect(x: 0, y: 0, width: 20, height: 20))
            checkmarkView.image = UIImage(systemName: "checkmark")
            checkmarkView.tintColor = techBlue // Blue checkmark
            cell.accessoryView = checkmarkView
        }

        return cell
    }

    func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) {
        tableView.deselectRow(at: indexPath, animated: true)
        let selectedAlbum = albums[indexPath.row]

        // Update current album
        currentAlbum = selectedAlbum.collection

        // Update title label text
        titleLabel.text = selectedAlbum.title

        // Dismiss the album menu
        toggleAlbumMenu()

        // Reload assets based on selected album
        fetchAssetsAndReload()
    }

}

extension MediaPickerViewController: PHPhotoLibraryChangeObserver {
    nonisolated func photoLibraryDidChange(_ changeInstance: PHChange) {
        Task { @MainActor [weak self] in
            self?.fetchAssetsAndReload()
        }
    }
}

private final class MediaCell: UICollectionViewCell {
    let imageView = UIImageView()
    let overlay = UIView()
    let checkboxBg = UIView()
    let checkboxRing = CAShapeLayer()
    let checkmark = CAShapeLayer()
    let badgeView = UILabel()

    override init(frame: CGRect) {
        super.init(frame: frame)
        imageView.clipsToBounds = true
        imageView.contentMode = .scaleAspectFill
        contentView.addSubview(imageView)
        imageView.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            imageView.topAnchor.constraint(equalTo: contentView.topAnchor),
            imageView.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
            imageView.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
            imageView.bottomAnchor.constraint(equalTo: contentView.bottomAnchor)
        ])

        overlay.backgroundColor = UIColor(white: 0, alpha: 0.25)
        overlay.isHidden = true
        contentView.addSubview(overlay)
        overlay.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            overlay.topAnchor.constraint(equalTo: contentView.topAnchor),
            overlay.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
            overlay.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
            overlay.bottomAnchor.constraint(equalTo: contentView.bottomAnchor)
        ])

        // Circular checkbox
        checkboxBg.backgroundColor = .clear
        checkboxBg.layer.cornerRadius = 10
        checkboxBg.clipsToBounds = true
        contentView.addSubview(checkboxBg)
        checkboxBg.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            checkboxBg.trailingAnchor.constraint(equalTo: contentView.trailingAnchor, constant: -4),
            checkboxBg.topAnchor.constraint(equalTo: contentView.topAnchor, constant: 4),
            checkboxBg.widthAnchor.constraint(equalToConstant: 20),
            checkboxBg.heightAnchor.constraint(equalToConstant: 20)
        ])

        checkboxBg.layer.addSublayer(checkboxRing)
        checkboxBg.layer.addSublayer(checkmark)

        badgeView.text = "VID"
        badgeView.textColor = .white
        badgeView.font = UIFont.boldSystemFont(ofSize: 10)
        badgeView.backgroundColor = UIColor(white: 0, alpha: 0.5)
        badgeView.layer.cornerRadius = 3
        badgeView.clipsToBounds = true
        badgeView.isHidden = true
        contentView.addSubview(badgeView)
        badgeView.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            badgeView.leadingAnchor.constraint(equalTo: contentView.leadingAnchor, constant: 4),
            badgeView.bottomAnchor.constraint(equalTo: contentView.bottomAnchor, constant: -4)
        ])
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override func layoutSubviews() {
        super.layoutSubviews()
        let size: CGFloat = 20

        // Ring
        let ringPath = UIBezierPath(ovalIn: CGRect(x: 2, y: 2, width: size-4, height: size-4))
        checkboxRing.path = ringPath.cgPath
        checkboxRing.fillColor = UIColor.clear.cgColor
        checkboxRing.strokeColor = UIColor.white.cgColor
        checkboxRing.lineWidth = 1.5
    }

    func setSelected(_ value: Bool, accent: UIColor) {
        overlay.isHidden = !value

        if value {
            checkboxBg.backgroundColor = accent
            checkboxRing.strokeColor = UIColor.clear.cgColor

            // Draw checkmark
            let path = UIBezierPath()
            path.move(to: CGPoint(x: 6, y: 10))
            path.addLine(to: CGPoint(x: 9, y: 13))
            path.addLine(to: CGPoint(x: 14, y: 7))
            checkmark.path = path.cgPath
            checkmark.strokeColor = UIColor.white.cgColor
            checkmark.fillColor = UIColor.clear.cgColor
            checkmark.lineWidth = 2
            checkmark.lineCap = .round
            checkmark.lineJoin = .round
        } else {
            checkboxBg.backgroundColor = .clear
            checkboxRing.strokeColor = UIColor.white.cgColor
            checkmark.path = nil
        }
    }
}

private final class PlusCell: UICollectionViewCell {
    private let plus = UILabel()
    private let hint = UILabel()
    override init(frame: CGRect) {
        super.init(frame: frame)
        contentView.backgroundColor = UIColor(white: 0.95, alpha: 1)
        contentView.layer.cornerRadius = 8

        plus.text = "+"
        plus.textAlignment = .center
        plus.font = UIFont.systemFont(ofSize: 32, weight: .light)
        plus.textColor = .systemBlue
        contentView.addSubview(plus)
        plus.translatesAutoresizingMaskIntoConstraints = false

        hint.textAlignment = .center
        hint.numberOfLines = 2
        hint.font = UIFont.systemFont(ofSize: 10)
        hint.textColor = UIColor.darkGray
        contentView.addSubview(hint)
        hint.translatesAutoresizingMaskIntoConstraints = false

        NSLayoutConstraint.activate([
            plus.centerXAnchor.constraint(equalTo: contentView.centerXAnchor),
            plus.centerYAnchor.constraint(equalTo: contentView.centerYAnchor, constant: -12),
            hint.centerXAnchor.constraint(equalTo: contentView.centerXAnchor),
            hint.topAnchor.constraint(equalTo: plus.bottomAnchor, constant: 2)
        ])
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    func configure(color: UIColor, hintText: String) {
        plus.textColor = color
        hint.text = hintText
    }
}

private final class RadioOptionView: UIControl {
    private let ringLayer = CAShapeLayer()
    private let dotLayer = CAShapeLayer()
    private let titleLabel = UILabel()
    private let color: UIColor
    var isOn: Bool = false { didSet { updateAppearance() } }

    init(title: String, color: UIColor) {
        self.color = color
        super.init(frame: .zero)
        isAccessibilityElement = true
        accessibilityTraits = [.button]
        isUserInteractionEnabled = true

        layer.addSublayer(ringLayer)
        layer.addSublayer(dotLayer)
        titleLabel.text = title
        titleLabel.textColor = .darkGray
        titleLabel.font = UIFont.systemFont(ofSize: 14)
        titleLabel.isUserInteractionEnabled = false
        addSubview(titleLabel)
        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            titleLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 24),
            titleLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
            titleLabel.trailingAnchor.constraint(equalTo: trailingAnchor),
            heightAnchor.constraint(equalToConstant: 44),
            widthAnchor.constraint(equalToConstant: 80)
        ])
        addTarget(self, action: #selector(onTap), for: .touchUpInside)
        backgroundColor = .clear
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override var intrinsicContentSize: CGSize {
        return CGSize(width: 80, height: 44)
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        let center = CGPoint(x: 10, y: bounds.height/2)
        let ringPath = UIBezierPath(ovalIn: CGRect(x: center.x-8, y: center.y-8, width: 16, height: 16))
        ringLayer.path = ringPath.cgPath
        ringLayer.fillColor = UIColor.clear.cgColor
        ringLayer.strokeColor = UIColor.lightGray.cgColor
        ringLayer.lineWidth = 1.5

        let dotPath = UIBezierPath(ovalIn: CGRect(x: center.x-5, y: center.y-5, width: 10, height: 10))
        dotLayer.path = dotPath.cgPath
        dotLayer.fillColor = isOn ? color.cgColor : UIColor.clear.cgColor
    }

    private func updateAppearance() {
        dotLayer.fillColor = isOn ? color.cgColor : UIColor.clear.cgColor
        accessibilityValue = isOn ? "selected" : "not selected"
    }

    @objc private func onTap() {
        isOn.toggle()
        sendActions(for: .valueChanged)
    }
}

private final class ArrowView: UIView {
    override func didMoveToWindow() {
        super.didMoveToWindow()
        isOpaque = true
        backgroundColor = .clear
    }
    override func tintColorDidChange() {
        super.tintColorDidChange()
        setNeedsDisplay()
    }

    override func draw(_ rect: CGRect) {
        super.draw(rect)
        guard let ctx = UIGraphicsGetCurrentContext() else { return }

        // Fill circle background (opaque) to avoid bar vibrancy/appearance darkening
        let circleFill = UIColor.white.cgColor
        let circleStroke: CGColor
        if #available(iOS 13.0, *) {
            circleStroke = UIColor.systemGray3.cgColor
        } else {
            circleStroke = UIColor(white: 0.8, alpha: 1).cgColor
        }
        let inset: CGFloat = 0.5
        let circleRect = rect.insetBy(dx: inset, dy: inset)
        ctx.setFillColor(circleFill)
        ctx.fillEllipse(in: circleRect)
        ctx.setStrokeColor(circleStroke)
        ctx.setLineWidth(1)
        ctx.strokeEllipse(in: circleRect)

        // Draw chevron
        let w = rect.width
        let h = rect.height
        let left = w * 0.32
        let right = w * 0.68
        let top = h * 0.38
        let bottom = h * 0.62

        ctx.setStrokeColor((tintColor ?? UIColor(white: 0.35, alpha: 1)).cgColor)
        ctx.setLineWidth(1.0)
        ctx.setLineCap(.round)
        ctx.setLineJoin(.round)

        let path = CGMutablePath()
        path.move(to: CGPoint(x: left, y: top))
        path.addLine(to: CGPoint(x: w/2, y: bottom))
        path.addLine(to: CGPoint(x: right, y: top))
        ctx.addPath(path)
        ctx.strokePath()
    }
}
#endif
