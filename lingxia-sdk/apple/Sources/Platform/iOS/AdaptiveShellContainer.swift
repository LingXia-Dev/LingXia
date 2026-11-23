#if os(iOS)
import UIKit

/// Responsive shell container for iOS, iPadOS, and foldable Apple hardware (e.g. iPhone Duo).
///
/// Driven purely by live container width (`compact < 600 pt`, `regular >= 600 pt` with ±24 pt
/// hysteresis) and `LxAppLayoutReconcileriOS` plans — zero device-class hardcoding.
/// Mobile hosts never render host-level sidebar Pins; the sidebar holds live switchers only.
/// Not yet mounted by the iOS host: pages still present one main at a time.
@MainActor
public final class AdaptiveShellContainerViewController: UIViewController {
    public enum ShellSizeClass: String {
        case compact
        case regular
    }

    public static let compactMaxWidth: CGFloat = 600
    public static let hysteresis: CGFloat = 24
    public static let sidebarExpandedWidth: CGFloat = 220
    public static let sidebarRailWidth: CGFloat = 56
    public static let sidebarExpandMinFreeWidth: CGFloat = 820
    public static let defaultAsideWidth: CGFloat = 360

    public var onSelectMain: ((String) -> Void)?
    public var onSelectAsideTab: ((String) -> Void)?
    public var onCloseAsideTab: ((String) -> Void)?
    public var onPromoteAsideToMain: ((String) -> Void)?
    public var onDemoteMainToAside: ((String) -> Void)?
    public var onToggleAsideCollapsed: ((Bool) -> Void)?
    public var onToggleAsideMaximized: ((Bool) -> Void)?

    private(set) public var sizeClass: ShellSizeClass = .compact
    private var currentPlan: LxAppLayoutReconcileriOS.LayoutPresentationPlan?
    private var sidebarManuallyCollapsed = false
    private var asideCollapsed = false
    private var asideMaximized = false

    private let sidebarContainer = UIView()
    private let mainContainer = UIView()
    private let asideContainer = UIView()
    private let asideContentContainer = UIView()
    private let asideTabBar = UIStackView()
    private let collapsedEarButton = UIButton(type: .system)

    private var sidebarWidthConstraint: NSLayoutConstraint?
    private var asideWidthConstraint: NSLayoutConstraint?
    nonisolated(unsafe) private var planObserver: NSObjectProtocol?

    public override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground
        setupLayout()
        planObserver = NotificationCenter.default.addObserver(
            forName: LxAppLayoutReconcileriOS.layoutPlanDidUpdateNotification,
            object: nil,
            queue: .main
        ) { [weak self] note in
            let windowId = note.userInfo?["windowId"] as? String ?? "primary"
            MainActor.assumeIsolated {
                if let plan = LxAppLayoutReconcileriOS.latestPlanByWindow[windowId] {
                    self?.apply(plan: plan)
                }
            }
        }
    }

    deinit {
        if let observer = planObserver {
            NotificationCenter.default.removeObserver(observer)
        }
    }

    public override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        let width = view.bounds.width
        guard width > 0 else { return }
        let next = Self.resolveSizeClass(previous: sizeClass, width: width)
        if next != sizeClass {
            sizeClass = next
        }
        reconcileGeometry()
    }

    public static func resolveSizeClass(previous: ShellSizeClass, width: CGFloat) -> ShellSizeClass {
        let raw: ShellSizeClass = width < compactMaxWidth ? .compact : .regular
        if raw == previous { return raw }
        return abs(width - compactMaxWidth) < hysteresis ? previous : raw
    }

    public func setMainContentView(_ contentView: UIView) {
        mainContainer.subviews.forEach { $0.removeFromSuperview() }
        contentView.translatesAutoresizingMaskIntoConstraints = false
        mainContainer.addSubview(contentView)
        NSLayoutConstraint.activate([
            contentView.topAnchor.constraint(equalTo: mainContainer.topAnchor),
            contentView.bottomAnchor.constraint(equalTo: mainContainer.bottomAnchor),
            contentView.leadingAnchor.constraint(equalTo: mainContainer.leadingAnchor),
            contentView.trailingAnchor.constraint(equalTo: mainContainer.trailingAnchor),
        ])
    }

    public func setAsideContentView(_ contentView: UIView) {
        asideContentContainer.subviews.forEach { $0.removeFromSuperview() }
        contentView.translatesAutoresizingMaskIntoConstraints = false
        asideContentContainer.addSubview(contentView)
        NSLayoutConstraint.activate([
            contentView.topAnchor.constraint(equalTo: asideContentContainer.topAnchor),
            contentView.bottomAnchor.constraint(equalTo: asideContentContainer.bottomAnchor),
            contentView.leadingAnchor.constraint(equalTo: asideContentContainer.leadingAnchor),
            contentView.trailingAnchor.constraint(equalTo: asideContentContainer.trailingAnchor),
        ])
    }

    public func apply(plan: LxAppLayoutReconcileriOS.LayoutPresentationPlan) {
        currentPlan = plan
        if let next = ShellSizeClass(rawValue: plan.sizeClass) {
            sizeClass = next
        }
        if let unified = plan.unifiedAsideSlot {
            asideCollapsed = unified.collapsed
            asideMaximized = unified.maximized
        }
        rebuildAsideTabs(plan: plan)
        reconcileGeometry()
    }

    private func setupLayout() {
        sidebarContainer.translatesAutoresizingMaskIntoConstraints = false
        sidebarContainer.backgroundColor = .secondarySystemBackground
        mainContainer.translatesAutoresizingMaskIntoConstraints = false
        asideContainer.translatesAutoresizingMaskIntoConstraints = false
        asideContainer.backgroundColor = .systemBackground
        asideContentContainer.translatesAutoresizingMaskIntoConstraints = false
        asideTabBar.translatesAutoresizingMaskIntoConstraints = false
        asideTabBar.axis = .horizontal
        asideTabBar.spacing = 6
        asideTabBar.alignment = .center
        asideContainer.addSubview(asideTabBar)
        asideContainer.addSubview(asideContentContainer)

        collapsedEarButton.translatesAutoresizingMaskIntoConstraints = false
        collapsedEarButton.backgroundColor = .tintColor
        collapsedEarButton.setTitleColor(.white, for: .normal)
        collapsedEarButton.titleLabel?.font = .monospacedSystemFont(ofSize: 12, weight: .bold)
        collapsedEarButton.layer.cornerRadius = 10
        collapsedEarButton.isHidden = true
        collapsedEarButton.addTarget(self, action: #selector(handleExpandEar), for: .touchUpInside)

        view.addSubview(sidebarContainer)
        view.addSubview(mainContainer)
        view.addSubview(asideContainer)
        view.addSubview(collapsedEarButton)

        let sbWidth = sidebarContainer.widthAnchor.constraint(equalToConstant: 0)
        let asWidth = asideContainer.widthAnchor.constraint(equalToConstant: 0)
        sidebarWidthConstraint = sbWidth
        asideWidthConstraint = asWidth

        NSLayoutConstraint.activate([
            sidebarContainer.topAnchor.constraint(equalTo: view.topAnchor),
            sidebarContainer.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            sidebarContainer.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            sbWidth,

            mainContainer.topAnchor.constraint(equalTo: view.topAnchor),
            mainContainer.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            mainContainer.leadingAnchor.constraint(equalTo: sidebarContainer.trailingAnchor),
            mainContainer.trailingAnchor.constraint(equalTo: asideContainer.leadingAnchor),

            asideContainer.topAnchor.constraint(equalTo: view.topAnchor),
            asideContainer.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            asideContainer.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            asWidth,

            asideTabBar.topAnchor.constraint(equalTo: asideContainer.safeAreaLayoutGuide.topAnchor, constant: 4),
            asideTabBar.leadingAnchor.constraint(equalTo: asideContainer.leadingAnchor, constant: 8),
            asideTabBar.trailingAnchor.constraint(equalTo: asideContainer.trailingAnchor, constant: -8),
            asideTabBar.heightAnchor.constraint(equalToConstant: 36),

            asideContentContainer.topAnchor.constraint(equalTo: asideTabBar.bottomAnchor),
            asideContentContainer.bottomAnchor.constraint(equalTo: asideContainer.bottomAnchor),
            asideContentContainer.leadingAnchor.constraint(equalTo: asideContainer.leadingAnchor),
            asideContentContainer.trailingAnchor.constraint(equalTo: asideContainer.trailingAnchor),

            collapsedEarButton.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            collapsedEarButton.centerYAnchor.constraint(equalTo: view.centerYAnchor),
            collapsedEarButton.widthAnchor.constraint(equalToConstant: 28),
            collapsedEarButton.heightAnchor.constraint(equalToConstant: 56),
        ])
    }

    private func reconcileGeometry() {
        let totalWidth = view.bounds.width
        let mainsCount = currentPlan?.mains.count ?? 1
        let tabIds = currentPlan?.unifiedAsideSlot?.tabs ?? currentPlan?.asides.map(\.id) ?? []
        let asideCount = tabIds.count
        let isRtl = view.effectiveUserInterfaceLayoutDirection == .rightToLeft
        collapsedEarButton.layer.maskedCorners = isRtl
            ? [.layerMaxXMinYCorner, .layerMaxXMaxYCorner]
            : [.layerMinXMinYCorner, .layerMinXMaxYCorner]

        if sizeClass == .compact {
            sidebarWidthConstraint?.constant = 0
            asideWidthConstraint?.constant = 0
            asideContainer.isHidden = true
            collapsedEarButton.isHidden = asideCount == 0
            collapsedEarButton.setTitle(isRtl ? "\(asideCount) ›" : "‹ \(asideCount)", for: .normal)
            return
        }

        // Regular (>= 600 pt): dock aside beside main (snapping to vertical hinge if present),
        // and expand sidebar to 220 pt only when free width >= 820 pt.
        let showAside = asideCount > 0 && !asideCollapsed
        let asideW: CGFloat
        if !showAside {
            asideW = 0
        } else if asideMaximized {
            asideW = max(Self.defaultAsideWidth, totalWidth - (mainsCount > 1 ? Self.sidebarRailWidth : 0))
        } else if let hinge = currentPlan?.hinge, hinge.orientation == "vertical" {
            asideW = isRtl
                ? max(240, CGFloat(hinge.x))
                : max(240, totalWidth - CGFloat(hinge.x + hinge.width))
        } else {
            asideW = min(Self.defaultAsideWidth, totalWidth * 0.45)
        }

        let freeWidth = max(0, totalWidth - asideW)
        let sidebarW: CGFloat
        if mainsCount <= 1 {
            sidebarW = 0
        } else if !sidebarManuallyCollapsed && freeWidth >= Self.sidebarExpandMinFreeWidth {
            sidebarW = Self.sidebarExpandedWidth
        } else {
            sidebarW = Self.sidebarRailWidth
        }

        sidebarWidthConstraint?.constant = sidebarW
        asideWidthConstraint?.constant = asideW
        asideContainer.isHidden = !showAside
        collapsedEarButton.isHidden = !(asideCount > 0 && asideCollapsed)
        collapsedEarButton.setTitle(isRtl ? "\(asideCount) ›" : "‹ \(asideCount)", for: .normal)
    }

    private func rebuildAsideTabs(plan: LxAppLayoutReconcileriOS.LayoutPresentationPlan) {
        asideTabBar.arrangedSubviews.forEach { view in
            asideTabBar.removeArrangedSubview(view)
            view.removeFromSuperview()
        }
        let tabs = plan.unifiedAsideSlot?.tabs ?? plan.asides.map(\.id)
        let activeTab = plan.unifiedAsideSlot?.activeTab ?? tabs.last
        for tabId in tabs {
            let button = UIButton(type: .system)
            button.setTitle(tabId, for: .normal)
            button.titleLabel?.font = .systemFont(
                ofSize: 12,
                weight: tabId == activeTab ? .bold : .medium
            )
            button.accessibilityIdentifier = "aside-tab-\(tabId)"
            button.addAction(UIAction { [weak self] _ in
                self?.onSelectAsideTab?(tabId)
            }, for: .touchUpInside)
            asideTabBar.addArrangedSubview(button)
        }

        let spacer = UIView()
        spacer.setContentHuggingPriority(.defaultLow, for: .horizontal)
        asideTabBar.addArrangedSubview(spacer)

        let maximizeButton = UIButton(type: .system)
        maximizeButton.setTitle("⤢", for: .normal)
        maximizeButton.accessibilityLabel = asideMaximized ? "Restore Aside" : "Maximize Aside"
        maximizeButton.accessibilityIdentifier = "aside-maximize"
        maximizeButton.addAction(UIAction { [weak self] _ in
            guard let self else { return }
            self.asideMaximized.toggle()
            self.onToggleAsideMaximized?(self.asideMaximized)
            self.reconcileGeometry()
        }, for: .touchUpInside)
        asideTabBar.addArrangedSubview(maximizeButton)

        let collapseButton = UIButton(type: .system)
        collapseButton.setTitle("⇥", for: .normal)
        collapseButton.accessibilityLabel = "Collapse Aside"
        collapseButton.accessibilityIdentifier = "aside-collapse"
        collapseButton.addAction(UIAction { [weak self] _ in
            guard let self else { return }
            self.asideCollapsed = true
            self.onToggleAsideCollapsed?(true)
            self.reconcileGeometry()
        }, for: .touchUpInside)
        asideTabBar.addArrangedSubview(collapseButton)
    }

    @objc private func handleExpandEar() {
        asideCollapsed = false
        onToggleAsideCollapsed?(false)
        reconcileGeometry()
    }
}
#endif
