import Foundation

#if os(iOS)
import UIKit

// MARK: - Calendar Picker View

@MainActor
final class DateRangePickerView: UIView, UICollectionViewDataSource, UICollectionViewDelegateFlowLayout {
    var selectedRange: (start: Date, end: Date)? {
        didSet { collectionView.reloadData() }
    }
    var selectedDate: Date? {
        didSet { collectionView.reloadData() }
    }
    var minimumDate: Date?
    var maximumDate: Date?
    var isSingleSelection = false
    var showQuickSelect = true
    var onSelectionStateChange: ((Bool) -> Void)?
    var onValueChange: ((Any) -> Void)?

    private var hasCompletedRangeSelection = false

    var isSelectionComplete: Bool {
        isSingleSelection ? (selectedDate != nil) : hasCompletedRangeSelection
    }

    private var isChinese: Bool {
        Locale.current.identifier.lowercased().hasPrefix("zh")
    }

    private var firstWeekday: Int {
        isChinese ? 2 : calendar.firstWeekday
    }

    private var currentMonth: Date = Date()
    private var calendarDays: [Date?] = []
    private var isSelectingStart = true
    private var tempStartDate: Date?

    private let selectionFeedback = UISelectionFeedbackGenerator()
    private let impactFeedback = UIImpactFeedbackGenerator(style: .light)
    private let calendar = Calendar.current

    private lazy var monthLabel: UILabel = {
        let label = UILabel()
        label.font = .systemFont(ofSize: 17, weight: .semibold)
        label.textColor = .label
        label.textAlignment = .center
        return label
    }()

    private lazy var prevButton: UIButton = {
        let btn = UIButton(type: .system)
        btn.setImage(UIImage(systemName: "chevron.left"), for: .normal)
        btn.tintColor = .systemBlue
        btn.addAction(UIAction { [weak self] _ in self?.changeMonth(by: -1) }, for: .touchUpInside)
        return btn
    }()

    private lazy var nextButton: UIButton = {
        let btn = UIButton(type: .system)
        btn.setImage(UIImage(systemName: "chevron.right"), for: .normal)
        btn.tintColor = .systemBlue
        btn.addAction(UIAction { [weak self] _ in self?.changeMonth(by: 1) }, for: .touchUpInside)
        return btn
    }()
    
    private lazy var prevYearButton: UIButton = {
        let btn = UIButton(type: .system)
        btn.setImage(UIImage(systemName: "chevron.left.2"), for: .normal)
        btn.addAction(UIAction { [weak self] _ in self?.changeYear(by: -1) }, for: .touchUpInside)
        return btn
    }()

    private lazy var nextYearButton: UIButton = {
        let btn = UIButton(type: .system)
        btn.setImage(UIImage(systemName: "chevron.right.2"), for: .normal)
        btn.addAction(UIAction { [weak self] _ in self?.changeYear(by: 1) }, for: .touchUpInside)
        return btn
    }()

    private lazy var collectionView: UICollectionView = {
        let layout = UICollectionViewFlowLayout()
        layout.minimumInteritemSpacing = 0
        layout.minimumLineSpacing = 1
        let cv = UICollectionView(frame: .zero, collectionViewLayout: layout)
        cv.backgroundColor = .clear
        cv.dataSource = self
        cv.delegate = self
        cv.register(CalendarDayCell.self, forCellWithReuseIdentifier: "day")
        return cv
    }()

    private lazy var quickSelectStack: UIStackView = {
        let stack = UIStackView()
        stack.axis = .horizontal
        stack.distribution = .fillEqually
        stack.spacing = 8
        return stack
    }()

    override init(frame: CGRect) {
        super.init(frame: frame)
        selectionFeedback.prepare()
        impactFeedback.prepare()
        setupUI()
        updateMonth()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func setupForMode(singleSelection: Bool, showQuick: Bool) {
        isSingleSelection = singleSelection
        showQuickSelect = showQuick
        hasCompletedRangeSelection = singleSelection
        setupUI()
        updateMonth()
    }

    private func emitValueChange() {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd"

        if isSingleSelection {
            guard let date = selectedDate else { return }
            onValueChange?(formatter.string(from: date))
            return
        }

        guard let range = selectedRange else { return }
        onValueChange?([formatter.string(from: range.start), formatter.string(from: range.end)])
    }

    func setInitialDate(_ date: Date) {
        selectedDate = date
        currentMonth = date
        hasCompletedRangeSelection = true
        isSelectingStart = true
        tempStartDate = nil
        updateMonth()
        onSelectionStateChange?(true)
    }

    func setInitialRange(start: Date, end: Date) {
        selectedRange = (start, end)
        currentMonth = start
        hasCompletedRangeSelection = true
        isSelectingStart = true
        tempStartDate = nil
        updateMonth()
        onSelectionStateChange?(true)
    }

    private func setupUI() {
        backgroundColor = .systemBackground

        subviews.forEach { $0.removeFromSuperview() }
        quickSelectStack.arrangedSubviews.forEach { $0.removeFromSuperview() }

        if showQuickSelect && !isSingleSelection {
            quickSelectStack.axis = .vertical
            quickSelectStack.spacing = 6

            let row1Options: [(String, QuickSelectType)] = [
                (L10n.string("lx_date_last_7_days"), .last7Days),
                (L10n.string("lx_date_last_30_days"), .last30Days),
                (L10n.string("lx_date_this_week"), .thisWeek)
            ]
            let row2Options: [(String, QuickSelectType)] = [
                (L10n.string("lx_date_last_week"), .lastWeek),
                (L10n.string("lx_date_this_month"), .thisMonth),
                (L10n.string("lx_date_last_month"), .lastMonth)
            ]

            func createButtonRow(_ options: [(String, QuickSelectType)]) -> UIStackView {
                let row = UIStackView()
                row.axis = .horizontal
                row.distribution = .fillEqually
                row.spacing = 8
                for (title, type) in options {
                    let btn = UIButton(type: .system)
                    btn.setTitle(title, for: .normal)
                    btn.titleLabel?.font = .systemFont(ofSize: 13, weight: .medium)
                    btn.setTitleColor(.systemBlue, for: .normal)
                    btn.backgroundColor = UIColor.systemBlue.withAlphaComponent(0.08)
                    btn.layer.cornerRadius = 14
                    btn.tag = type.rawValue
                    btn.addAction(UIAction { [weak self] action in
                        if let tag = (action.sender as? UIButton)?.tag,
                           let selectType = QuickSelectType(rawValue: tag) {
                            self?.quickSelectByType(selectType)
                        }
                    }, for: .touchUpInside)
                    row.addArrangedSubview(btn)
                }
                return row
            }

            quickSelectStack.addArrangedSubview(createButtonRow(row1Options))
            quickSelectStack.addArrangedSubview(createButtonRow(row2Options))
        }

        let monthStack = UIStackView(arrangedSubviews: [prevYearButton, prevButton, monthLabel, nextButton, nextYearButton])
        monthStack.axis = .horizontal
        monthStack.distribution = .fill
        monthStack.alignment = .center
        monthStack.spacing = 4

        let weekdaySymbols = calendar.shortWeekdaySymbols
        let weekdays = (0..<weekdaySymbols.count).map { weekdaySymbols[($0 + firstWeekday - 1) % weekdaySymbols.count] }
        let weekdayStack = UIStackView()
        weekdayStack.axis = .horizontal
        weekdayStack.distribution = .fillEqually
        for day in weekdays {
            let label = UILabel()
            label.text = day
            label.font = .systemFont(ofSize: 11, weight: .medium)
            label.textColor = .secondaryLabel
            label.textAlignment = .center
            weekdayStack.addArrangedSubview(label)
        }

        [quickSelectStack, monthStack, weekdayStack, collectionView].forEach {
            $0.translatesAutoresizingMaskIntoConstraints = false
            addSubview($0)
        }

        let quickSelectHeight: CGFloat = (showQuickSelect && !isSingleSelection) ? 62 : 0

        NSLayoutConstraint.activate([
            quickSelectStack.topAnchor.constraint(equalTo: topAnchor, constant: 6),
            quickSelectStack.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            quickSelectStack.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
            quickSelectStack.heightAnchor.constraint(equalToConstant: quickSelectHeight),

            monthStack.topAnchor.constraint(equalTo: quickSelectStack.bottomAnchor, constant: quickSelectHeight > 0 ? 8 : 0),
            monthStack.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            monthStack.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
            monthStack.heightAnchor.constraint(equalToConstant: 32),

            prevButton.widthAnchor.constraint(equalToConstant: 32),
            nextButton.widthAnchor.constraint(equalToConstant: 32),
            prevYearButton.widthAnchor.constraint(equalToConstant: 32),
            nextYearButton.widthAnchor.constraint(equalToConstant: 32),

            weekdayStack.topAnchor.constraint(equalTo: monthStack.bottomAnchor, constant: 12),
            weekdayStack.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
            weekdayStack.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
            weekdayStack.heightAnchor.constraint(equalToConstant: 20),

            collectionView.topAnchor.constraint(equalTo: weekdayStack.bottomAnchor, constant: 4),
            collectionView.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
            collectionView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
            collectionView.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -6)
        ])

        if isSingleSelection {
            if selectedDate == nil { selectedDate = calendar.startOfDay(for: Date()) }
            onSelectionStateChange?(true)
        } else {
            onSelectionStateChange?(hasCompletedRangeSelection)
        }
    }

    private func quickSelectByType(_ type: QuickSelectType) {
        impactFeedback.impactOccurred()
        let today = calendar.startOfDay(for: Date())

        let range: (Date, Date)
        switch type {
        case .last7Days:
            let start = calendar.date(byAdding: .day, value: -6, to: today) ?? today
            range = (start, today)
        case .last30Days:
            let start = calendar.date(byAdding: .day, value: -29, to: today) ?? today
            range = (start, today)
        case .thisWeek:
            let weekday = calendar.component(.weekday, from: today)
            let daysFromStart = (weekday - firstWeekday + 7) % 7
            let startOfWeek = calendar.date(byAdding: .day, value: -daysFromStart, to: today) ?? today
            range = (startOfWeek, today)
        case .lastWeek:
            let weekday = calendar.component(.weekday, from: today)
            let daysFromStart = (weekday - firstWeekday + 7) % 7
            let startOfThisWeek = calendar.date(byAdding: .day, value: -daysFromStart, to: today) ?? today
            let startOfLastWeek = calendar.date(byAdding: .day, value: -7, to: startOfThisWeek) ?? today
            let endOfLastWeek = calendar.date(byAdding: .day, value: 6, to: startOfLastWeek) ?? today
            range = (startOfLastWeek, endOfLastWeek)
        case .thisMonth:
            let components = calendar.dateComponents([.year, .month], from: today)
            let startOfMonth = calendar.date(from: components) ?? today
            range = (startOfMonth, today)
        case .lastMonth:
            let components = calendar.dateComponents([.year, .month], from: today)
            let startOfThisMonth = calendar.date(from: components) ?? today
            let startOfLastMonth = calendar.date(byAdding: .month, value: -1, to: startOfThisMonth) ?? today
            let endOfLastMonth = calendar.date(byAdding: .day, value: -1, to: startOfThisMonth) ?? today
            range = (startOfLastMonth, endOfLastMonth)
        }

        selectedRange = range
        isSelectingStart = true
        tempStartDate = nil
        hasCompletedRangeSelection = true
        onSelectionStateChange?(true)
        emitValueChange()
        currentMonth = range.0
        updateMonth()
    }

    private enum QuickSelectType: Int {
        case last7Days = 1
        case last30Days = 2
        case thisWeek = 3
        case lastWeek = 4
        case thisMonth = 5
        case lastMonth = 6
    }

    private func changeMonth(by offset: Int) {
        selectionFeedback.selectionChanged()
        if let newMonth = calendar.date(byAdding: .month, value: offset, to: currentMonth) {
            currentMonth = newMonth
            updateMonth()
        }
    }
    
    private func changeYear(by offset: Int) {
        selectionFeedback.selectionChanged()
        if let newMonth = calendar.date(byAdding: .year, value: offset, to: currentMonth) {
            currentMonth = newMonth
            updateMonth()
        }
    }

    private func updateMonth() {
        let formatter = DateFormatter()
        formatter.dateFormat = "MMMM yyyy"
        monthLabel.text = formatter.string(from: currentMonth)

        updateNavigationButtons()

        // Build a fixed 6-week grid (42 days), including adjacent month days, to avoid awkward empty space.
        let components = calendar.dateComponents([.year, .month], from: currentMonth)
        guard let firstOfMonth = calendar.date(from: components) else { return }

        let weekday = calendar.component(.weekday, from: firstOfMonth)
        let leadingDays = (weekday - self.firstWeekday + 7) % 7
        let gridStart = calendar.date(byAdding: .day, value: -leadingDays, to: firstOfMonth) ?? firstOfMonth

        calendarDays = (0..<42).compactMap { dayOffset in
            calendar.date(byAdding: .day, value: dayOffset, to: gridStart)
        }

        collectionView.reloadData()
    }

    private func monthStart(for date: Date) -> Date {
        let comps = calendar.dateComponents([.year, .month], from: date)
        return calendar.date(from: comps) ?? date
    }

    private func monthEnd(for date: Date) -> Date {
        let start = monthStart(for: date)
        let next = calendar.date(byAdding: .month, value: 1, to: start) ?? start
        return calendar.date(byAdding: .day, value: -1, to: next) ?? start
    }

    private func setNavButton(_ button: UIButton, enabled: Bool, tintColor: UIColor) {
        button.isEnabled = enabled
        button.tintColor = enabled ? tintColor : .tertiaryLabel
    }

    private func updateNavigationButtons() {
        let minDate = minimumDate.map { calendar.startOfDay(for: $0) }
        let maxDate = maximumDate.map { calendar.startOfDay(for: $0) }

        let prevMonth = calendar.date(byAdding: .month, value: -1, to: currentMonth) ?? currentMonth
        let nextMonth = calendar.date(byAdding: .month, value: 1, to: currentMonth) ?? currentMonth
        let prevYear = calendar.date(byAdding: .year, value: -1, to: currentMonth) ?? currentMonth
        let nextYear = calendar.date(byAdding: .year, value: 1, to: currentMonth) ?? currentMonth

        let canPrevMonth = minDate.map { monthEnd(for: prevMonth) >= $0 } ?? true
        let canPrevYear = minDate.map { monthEnd(for: prevYear) >= $0 } ?? true
        let canNextMonth = maxDate.map { monthStart(for: nextMonth) <= $0 } ?? true
        let canNextYear = maxDate.map { monthStart(for: nextYear) <= $0 } ?? true

        setNavButton(prevButton, enabled: canPrevMonth, tintColor: .systemBlue)
        setNavButton(nextButton, enabled: canNextMonth, tintColor: .systemBlue)
        setNavButton(prevYearButton, enabled: canPrevYear, tintColor: UIColor.systemBlue.withAlphaComponent(0.55))
        setNavButton(nextYearButton, enabled: canNextYear, tintColor: UIColor.systemBlue.withAlphaComponent(0.55))
    }

    // MARK: - UICollectionViewDataSource

    func collectionView(_ collectionView: UICollectionView, numberOfItemsInSection section: Int) -> Int {
        return calendarDays.count
    }

    func collectionView(_ collectionView: UICollectionView, cellForItemAt indexPath: IndexPath) -> UICollectionViewCell {
        let cell = collectionView.dequeueReusableCell(withReuseIdentifier: "day", for: indexPath) as! CalendarDayCell
        let date = calendarDays[indexPath.item]
        let isInCurrentMonth = date.map { calendar.isDate($0, equalTo: currentMonth, toGranularity: .month) } ?? false
        cell.configure(
            date: date,
            isInCurrentMonth: isInCurrentMonth,
            range: isSingleSelection ? nil : selectedRange,
            selectedDate: isSingleSelection ? selectedDate : nil,
            isToday: date.map { calendar.isDateInToday($0) } ?? false,
            calendar: calendar,
            minimumDate: minimumDate,
            maximumDate: maximumDate
        )
        return cell
    }

    // MARK: - UICollectionViewDelegateFlowLayout

    func collectionView(_ collectionView: UICollectionView, layout collectionViewLayout: UICollectionViewLayout, sizeForItemAt indexPath: IndexPath) -> CGSize {
        let width = (collectionView.bounds.width - 14) / 7
        return CGSize(width: width, height: 34)
    }

    func collectionView(_ collectionView: UICollectionView, didSelectItemAt indexPath: IndexPath) {
        guard let date = calendarDays[indexPath.item] else { return }

        if let min = minimumDate, date < min { return }
        if let max = maximumDate, date > max { return }

        impactFeedback.impactOccurred()

        if isSingleSelection {
            selectedDate = date
            hasCompletedRangeSelection = true
            onSelectionStateChange?(true)
            emitValueChange()
        } else {
            if isSelectingStart {
                tempStartDate = date
                selectedRange = (date, date)
                isSelectingStart = false
                hasCompletedRangeSelection = false
                onSelectionStateChange?(false)
                emitValueChange()
            } else {
                guard let start = tempStartDate else {
                    tempStartDate = date
                    selectedRange = (date, date)
                    return
                }
                if date < start {
                    selectedRange = (date, start)
                } else {
                    selectedRange = (start, date)
                }
                isSelectingStart = true
                tempStartDate = nil
                hasCompletedRangeSelection = true
                onSelectionStateChange?(true)
                emitValueChange()
            }
        }

        if !calendar.isDate(date, equalTo: currentMonth, toGranularity: .month) {
            currentMonth = date
            updateMonth()
        }
    }
}

// MARK: - Calendar Day Cell

private class CalendarDayCell: UICollectionViewCell {
    private let dayLabel = UILabel()
    private let selectionBackground = UIView()
    private let rangeBackground = UIView()

    override init(frame: CGRect) {
        super.init(frame: frame)
        setupUI()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func setupUI() {
        rangeBackground.translatesAutoresizingMaskIntoConstraints = false
        contentView.addSubview(rangeBackground)

        selectionBackground.translatesAutoresizingMaskIntoConstraints = false
        contentView.addSubview(selectionBackground)

        dayLabel.font = .systemFont(ofSize: 15, weight: .medium)
        dayLabel.textAlignment = .center
        dayLabel.translatesAutoresizingMaskIntoConstraints = false
        contentView.addSubview(dayLabel)

        NSLayoutConstraint.activate([
            rangeBackground.topAnchor.constraint(equalTo: contentView.topAnchor, constant: 2),
            rangeBackground.bottomAnchor.constraint(equalTo: contentView.bottomAnchor, constant: -2),
            rangeBackground.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
            rangeBackground.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),

            selectionBackground.centerXAnchor.constraint(equalTo: contentView.centerXAnchor),
            selectionBackground.centerYAnchor.constraint(equalTo: contentView.centerYAnchor),
            selectionBackground.widthAnchor.constraint(equalToConstant: 30),
            selectionBackground.heightAnchor.constraint(equalToConstant: 30),

            dayLabel.centerXAnchor.constraint(equalTo: contentView.centerXAnchor),
            dayLabel.centerYAnchor.constraint(equalTo: contentView.centerYAnchor)
        ])
    }

    func configure(date: Date?, isInCurrentMonth: Bool, range: (start: Date, end: Date)?, selectedDate: Date? = nil, isToday: Bool, calendar: Calendar, minimumDate: Date? = nil, maximumDate: Date? = nil) {
        guard let date = date else {
            dayLabel.text = ""
            selectionBackground.isHidden = true
            rangeBackground.isHidden = true
            return
        }

        dayLabel.text = "\(calendar.component(.day, from: date))"

        // Check if date is outside allowed range
        let isDisabled = (minimumDate.map { date < $0 } ?? false) || (maximumDate.map { date > $0 } ?? false)

        // Apply styling based on state
        if isDisabled {
            // Disabled dates: very light gray + low opacity
            dayLabel.textColor = .quaternaryLabel
            dayLabel.alpha = 0.3
        } else {
            // Enabled dates: darker color based on current month
            dayLabel.textColor = isInCurrentMonth ? .label : .secondaryLabel
            dayLabel.alpha = 1.0
        }

        selectionBackground.layer.borderWidth = 0
        rangeBackground.isHidden = true

        if let selected = selectedDate {
            let isSelected = calendar.isDate(date, inSameDayAs: selected)
            if isSelected {
                selectionBackground.isHidden = false
                selectionBackground.backgroundColor = .systemBlue
                selectionBackground.layer.cornerRadius = 15
                dayLabel.textColor = .white
            } else {
                selectionBackground.isHidden = true
            }

            if isToday && !isSelected {
                selectionBackground.isHidden = false
                selectionBackground.backgroundColor = .clear
                selectionBackground.layer.cornerRadius = 15
                selectionBackground.layer.borderWidth = 1.5
                selectionBackground.layer.borderColor = UIColor.systemBlue.cgColor
            }
            return
        }

        let isStart = range.map { calendar.isDate(date, inSameDayAs: $0.start) } ?? false
        let isEnd = range.map { calendar.isDate(date, inSameDayAs: $0.end) } ?? false
        let isInRange = range.map { date >= $0.start && date <= $0.end } ?? false
        let isSameDay = range.map { calendar.isDate($0.start, inSameDayAs: $0.end) } ?? false

        if isStart || isEnd {
            selectionBackground.isHidden = false
            selectionBackground.backgroundColor = .systemBlue
            selectionBackground.layer.cornerRadius = 15
            dayLabel.textColor = .white
        } else {
            selectionBackground.isHidden = true
        }

        if isInRange && !isSameDay {
            rangeBackground.isHidden = false
            rangeBackground.backgroundColor = UIColor.systemBlue.withAlphaComponent(0.15)

            if isStart {
                rangeBackground.layer.cornerRadius = 15
                rangeBackground.layer.maskedCorners = [.layerMinXMinYCorner, .layerMinXMaxYCorner]
            } else if isEnd {
                rangeBackground.layer.cornerRadius = 15
                rangeBackground.layer.maskedCorners = [.layerMaxXMinYCorner, .layerMaxXMaxYCorner]
            } else {
                rangeBackground.layer.cornerRadius = 0
                rangeBackground.layer.maskedCorners = []
            }
        } else {
            rangeBackground.isHidden = true
        }

        if isToday && !isStart && !isEnd {
            selectionBackground.isHidden = false
            selectionBackground.backgroundColor = .clear
            selectionBackground.layer.cornerRadius = 15
            selectionBackground.layer.borderWidth = 1.5
            selectionBackground.layer.borderColor = UIColor.systemBlue.cgColor
        } else if !isStart && !isEnd {
            selectionBackground.layer.borderWidth = 0
        }
    }
}

// MARK: - Year Picker View (fields=year)

@MainActor
final class YearPickerView: UIView, UIPickerViewDataSource, UIPickerViewDelegate {
    var selectedYear: Int {
        didSet {
            let clamped = max(minYear, min(selectedYear, maxYear))
            if clamped != selectedYear {
                selectedYear = clamped
                return
            }
            pickerView.selectRow(clamped - minYear, inComponent: 0, animated: false)
        }
    }

    private let minYear: Int
    private let maxYear: Int
    private let pickerView = UIPickerView()

    init(minYear: Int = 1970, maxYear: Int = 2100, initialYear: Int? = nil) {
        self.minYear = minYear
        self.maxYear = maxYear
        self.selectedYear = initialYear ?? Calendar.current.component(.year, from: Date())
        super.init(frame: .zero)
        setupUI()
    }

    required init?(coder: NSCoder) { fatalError() }

    private func setupUI() {
        pickerView.dataSource = self
        pickerView.delegate = self
        pickerView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(pickerView)

        NSLayoutConstraint.activate([
            pickerView.topAnchor.constraint(equalTo: topAnchor),
            pickerView.leadingAnchor.constraint(equalTo: leadingAnchor),
            pickerView.trailingAnchor.constraint(equalTo: trailingAnchor),
            pickerView.bottomAnchor.constraint(equalTo: bottomAnchor)
        ])

        pickerView.selectRow(selectedYear - minYear, inComponent: 0, animated: false)
    }

    func numberOfComponents(in pickerView: UIPickerView) -> Int { 1 }

    func pickerView(_ pickerView: UIPickerView, numberOfRowsInComponent component: Int) -> Int {
        maxYear - minYear + 1
    }

    func pickerView(_ pickerView: UIPickerView, titleForRow row: Int, forComponent component: Int) -> String? {
        "\(minYear + row)"
    }

    func pickerView(_ pickerView: UIPickerView, didSelectRow row: Int, inComponent component: Int) {
        selectedYear = minYear + row
    }
}

// MARK: - Year-Month Picker View (fields=month)

@MainActor
final class YearMonthPickerView: UIView, UIPickerViewDataSource, UIPickerViewDelegate {
    var selectedYear: Int {
        didSet {
            let clamped = max(minYear, min(selectedYear, maxYear))
            if clamped != selectedYear {
                selectedYear = clamped
                return
            }
            pickerView.selectRow(clamped - minYear, inComponent: 0, animated: false)
        }
    }
    var selectedMonth: Int {
        didSet {
            let clamped = max(1, min(selectedMonth, 12))
            if clamped != selectedMonth {
                selectedMonth = clamped
                return
            }
            pickerView.selectRow(clamped - 1, inComponent: 1, animated: false)
        }
    }

    private let minYear: Int
    private let maxYear: Int
    private let pickerView = UIPickerView()
    private let monthSymbols: [String]

    init(minYear: Int = 1970, maxYear: Int = 2100, initialYear: Int? = nil, initialMonth: Int? = nil) {
        self.minYear = minYear
        self.maxYear = maxYear
        self.selectedYear = initialYear ?? Calendar.current.component(.year, from: Date())
        self.selectedMonth = initialMonth ?? Calendar.current.component(.month, from: Date())
        self.monthSymbols = Calendar.current.monthSymbols
        super.init(frame: .zero)
        setupUI()
    }

    required init?(coder: NSCoder) { fatalError() }

    private func setupUI() {
        pickerView.dataSource = self
        pickerView.delegate = self
        pickerView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(pickerView)

        NSLayoutConstraint.activate([
            pickerView.topAnchor.constraint(equalTo: topAnchor),
            pickerView.leadingAnchor.constraint(equalTo: leadingAnchor),
            pickerView.trailingAnchor.constraint(equalTo: trailingAnchor),
            pickerView.bottomAnchor.constraint(equalTo: bottomAnchor)
        ])

        pickerView.selectRow(selectedYear - minYear, inComponent: 0, animated: false)
        pickerView.selectRow(selectedMonth - 1, inComponent: 1, animated: false)
    }

    func numberOfComponents(in pickerView: UIPickerView) -> Int { 2 }

    func pickerView(_ pickerView: UIPickerView, numberOfRowsInComponent component: Int) -> Int {
        component == 0 ? (maxYear - minYear + 1) : 12
    }

    func pickerView(_ pickerView: UIPickerView, titleForRow row: Int, forComponent component: Int) -> String? {
        component == 0 ? "\(minYear + row)" : monthSymbols[row]
    }

    func pickerView(_ pickerView: UIPickerView, didSelectRow row: Int, inComponent component: Int) {
        if component == 0 {
            selectedYear = minYear + row
        } else {
            selectedMonth = row + 1
        }
    }
}

// MARK: - Time Range Picker View (mode=time)

@MainActor
final class TimeRangePickerView: UIView, UIPickerViewDataSource, UIPickerViewDelegate {
    var onValueChange: ((String) -> Void)?

    private let pickerView = UIPickerView()
    private let separatorLabel = UILabel()

    private let minMinutes: Int
    private let maxMinutes: Int
    private let allowedHours: [Int]
    private var allowedMinutes: [Int] = []

    private var selectedHour: Int
    private var selectedMinute: Int

    init(start: String?, end: String?, initialValue: String?) {
        let startMinutes = Self.parseTimeToMinutes(start) ?? 0
        let endMinutes = Self.parseTimeToMinutes(end) ?? (23 * 60 + 59)
        self.minMinutes = min(startMinutes, endMinutes)
        self.maxMinutes = max(startMinutes, endMinutes)

        let minHour = self.minMinutes / 60
        let maxHour = self.maxMinutes / 60
        self.allowedHours = Array(minHour...maxHour)

        let initialMinutes = Self.parseTimeToMinutes(initialValue) ?? self.minMinutes
        let clamped = max(self.minMinutes, min(initialMinutes, self.maxMinutes))
        self.selectedHour = clamped / 60
        self.selectedMinute = clamped % 60

        super.init(frame: .zero)
        setupUI()
        applySelection(hour: selectedHour, minute: selectedMinute, animated: false, emit: false)
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    func currentValueString() -> String {
        String(format: "%02d:%02d", selectedHour, selectedMinute)
    }

    private func setupUI() {
        backgroundColor = .clear

        pickerView.dataSource = self
        pickerView.delegate = self
        pickerView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(pickerView)

        separatorLabel.text = ":"
        separatorLabel.textColor = .secondaryLabel
        separatorLabel.font = .systemFont(ofSize: 22, weight: .semibold)
        separatorLabel.translatesAutoresizingMaskIntoConstraints = false
        addSubview(separatorLabel)

        NSLayoutConstraint.activate([
            pickerView.topAnchor.constraint(equalTo: topAnchor),
            pickerView.leadingAnchor.constraint(equalTo: leadingAnchor),
            pickerView.trailingAnchor.constraint(equalTo: trailingAnchor),
            pickerView.bottomAnchor.constraint(equalTo: bottomAnchor),

            separatorLabel.centerXAnchor.constraint(equalTo: centerXAnchor),
            separatorLabel.centerYAnchor.constraint(equalTo: centerYAnchor)
        ])
    }

    private static func parseTimeToMinutes(_ value: String?) -> Int? {
        guard let value else { return nil }
        let parts = value.split(separator: ":").compactMap { Int($0) }
        guard parts.count >= 2 else { return nil }
        let hour = max(0, min(parts[0], 23))
        let minute = max(0, min(parts[1], 59))
        return hour * 60 + minute
    }

    private func minutesForHour(_ hour: Int) -> [Int] {
        let minHour = minMinutes / 60
        let maxHour = maxMinutes / 60
        let minMinute = minMinutes % 60
        let maxMinute = maxMinutes % 60

        if minHour == maxHour {
            return Array(minMinute...maxMinute)
        }
        if hour == minHour {
            return Array(minMinute...59)
        }
        if hour == maxHour {
            return Array(0...maxMinute)
        }
        return Array(0...59)
    }

    private func applySelection(hour: Int, minute: Int, animated: Bool, emit: Bool) {
        selectedHour = hour
        allowedMinutes = minutesForHour(hour)
        pickerView.reloadComponent(1)

        let hourRow = allowedHours.firstIndex(of: hour) ?? 0
        let clampedMinute = allowedMinutes.contains(minute) ? minute : (allowedMinutes.first ?? 0)
        let minuteRow = allowedMinutes.firstIndex(of: clampedMinute) ?? 0

        selectedMinute = clampedMinute
        pickerView.selectRow(hourRow, inComponent: 0, animated: animated)
        pickerView.selectRow(minuteRow, inComponent: 1, animated: animated)

        if emit {
            onValueChange?(currentValueString())
        }
    }

    func numberOfComponents(in pickerView: UIPickerView) -> Int { 2 }

    func pickerView(_ pickerView: UIPickerView, numberOfRowsInComponent component: Int) -> Int {
        component == 0 ? allowedHours.count : allowedMinutes.count
    }

    func pickerView(_ pickerView: UIPickerView, widthForComponent component: Int) -> CGFloat {
        bounds.width * 0.45
    }

    func pickerView(_ pickerView: UIPickerView, rowHeightForComponent component: Int) -> CGFloat {
        44
    }

    func pickerView(_ pickerView: UIPickerView, titleForRow row: Int, forComponent component: Int) -> String? {
        if component == 0 { return String(format: "%02d", allowedHours[row]) }
        return String(format: "%02d", allowedMinutes[row])
    }

    func pickerView(_ pickerView: UIPickerView, didSelectRow row: Int, inComponent component: Int) {
        if component == 0 {
            let hour = allowedHours[row]
            applySelection(hour: hour, minute: selectedMinute, animated: true, emit: true)
        } else {
            guard row >= 0, row < allowedMinutes.count else { return }
            selectedMinute = allowedMinutes[row]
            onValueChange?(currentValueString())
        }
    }
}

#endif

#if os(iOS)

// MARK: - Background Tap Delegate (only triggers on actual background, not container)

private class BackgroundTapDelegate: NSObject, UIGestureRecognizerDelegate {
    static let shared = BackgroundTapDelegate()

    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        guard let backgroundView = gestureRecognizer.view,
              let container = backgroundView.subviews.first else { return true }
        let location = touch.location(in: backgroundView)
        return !container.frame.contains(location)
    }
}

class LxAppDatePicker {
    private nonisolated(unsafe) static var callbackIDKey: UInt8 = 0
    private nonisolated(unsafe) static var modeKey: UInt8 = 1
    private nonisolated(unsafe) static var fieldsKey: UInt8 = 2
    private nonisolated(unsafe) static var pickerViewKey: UInt8 = 3

    @MainActor
    private static var backgroundView: UIView?

    @MainActor
    private static var currentWindow: UIWindow?

    @MainActor
    static func showDatePicker(
        mode: String,
        fields: String = "day",
        value: Any?,
        start: String?,
        end: String?,
        cancelText: String,
        cancelButtonColor: String,
        cancelTextColor: String,
        confirmText: String,
        confirmButtonColor: String,
        confirmTextColor: String,
        callbackID: UInt64
    ) {
        guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = windowScene.windows.first(where: { $0.isKeyWindow }) ?? windowScene.windows.first,
              window.rootViewController != nil else {
            sendError(callback_id: callbackID)
            return
        }

        if let existing = backgroundView {
            existing.layer.removeAllAnimations()
            existing.removeFromSuperview()
            backgroundView = nil
            currentWindow = nil
        }

        currentWindow = window

        let background = UIView()
        background.backgroundColor = UIColor.black.withAlphaComponent(0.4)
        background.alpha = 0
        background.translatesAutoresizingMaskIntoConstraints = false
        backgroundView = background

        let tapGesture = UITapGestureRecognizer(target: self, action: #selector(backgroundTapped))
        tapGesture.delegate = BackgroundTapDelegate.shared
        background.addGestureRecognizer(tapGesture)

        let container = UIView()
        container.backgroundColor = .systemBackground
        container.layer.cornerRadius = 16
        container.layer.maskedCorners = [.layerMinXMinYCorner, .layerMaxXMinYCorner]
        container.translatesAutoresizingMaskIntoConstraints = false

        let pickerHeight: CGFloat
        switch (mode, fields) {
        case ("time", _):
            pickerHeight = 220
        case (_, "year"), (_, "month"):
            pickerHeight = 220
        case (_, "range"):
            pickerHeight = 364  // Calendar + quick select
        default:
            pickerHeight = 292  // Single date calendar
        }
        let pickerView = createDatePickerView(
            mode: mode,
            fields: fields,
            value: value,
            start: start,
            end: end,
            callbackID: callbackID
        )
        pickerView.translatesAutoresizingMaskIntoConstraints = false

        let impactGenerator = UIImpactFeedbackGenerator(style: .light)
        impactGenerator.prepare()

        let buttonStack = UIStackView()
        buttonStack.axis = .horizontal
        buttonStack.distribution = .fillEqually
        buttonStack.spacing = 12
        buttonStack.translatesAutoresizingMaskIntoConstraints = false

        let cancelButton = UIButton(type: .system)
        let cancelButtonText = cancelText.isEmpty ? L10n.string("lx_common_cancel") : cancelText
        cancelButton.setTitle(cancelButtonText, for: .normal)
        if !cancelButtonColor.isEmpty {
            cancelButton.backgroundColor = resolveColor(cancelButtonColor, fallback: .secondarySystemBackground)
        } else {
            cancelButton.backgroundColor = .secondarySystemBackground
        }
        if !cancelTextColor.isEmpty {
            cancelButton.setTitleColor(resolveColor(cancelTextColor, fallback: .label), for: .normal)
        } else {
            cancelButton.setTitleColor(.label, for: .normal)
        }
        cancelButton.titleLabel?.font = .systemFont(ofSize: 17, weight: .medium)
        cancelButton.layer.cornerRadius = 12
        cancelButton.translatesAutoresizingMaskIntoConstraints = false

        let confirmButton = UIButton(type: .system)
        let confirmButtonText = confirmText.isEmpty ? L10n.string("lx_common_confirm") : confirmText
        confirmButton.setTitle(confirmButtonText, for: .normal)
        if !confirmButtonColor.isEmpty {
            confirmButton.backgroundColor = resolveColor(confirmButtonColor, fallback: .systemBlue)
        } else {
            confirmButton.backgroundColor = .systemBlue
        }
        if !confirmTextColor.isEmpty {
            confirmButton.setTitleColor(resolveColor(confirmTextColor, fallback: .white), for: .normal)
        } else {
            confirmButton.setTitleColor(.white, for: .normal)
        }
        confirmButton.titleLabel?.font = .systemFont(ofSize: 17, weight: .semibold)
        confirmButton.layer.cornerRadius = 12
        confirmButton.translatesAutoresizingMaskIntoConstraints = false

        func setConfirmEnabled(_ enabled: Bool) {
            confirmButton.isEnabled = enabled
            confirmButton.alpha = enabled ? 1 : 0.45
        }

        if mode == "date", fields == "range", let calendarView = pickerView as? DateRangePickerView {
            calendarView.onSelectionStateChange = { isComplete in
                setConfirmEnabled(isComplete)
            }
            setConfirmEnabled(calendarView.isSelectionComplete)
        } else {
            setConfirmEnabled(true)
        }

        objc_setAssociatedObject(container, &callbackIDKey, NSNumber(value: callbackID), .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        objc_setAssociatedObject(container, &modeKey, mode, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        objc_setAssociatedObject(container, &fieldsKey, fields, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        objc_setAssociatedObject(container, &pickerViewKey, pickerView, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)

        cancelButton.addAction(UIAction { [weak background] _ in
            impactGenerator.impactOccurred()
            guard let bg = background, let container = bg.subviews.first else { return }
            if let callbackID = (objc_getAssociatedObject(container, &callbackIDKey) as? NSNumber)?.uint64Value {
                LxAppPicker.sendPickerResultCancel(callback_id: callbackID)
            }
            dismissDatePicker()
        }, for: .touchUpInside)

        confirmButton.addAction(UIAction { [weak background] _ in
            impactGenerator.impactOccurred()
            guard let bg = background, let container = bg.subviews.first else { return }
            if let callbackID = (objc_getAssociatedObject(container, &callbackIDKey) as? NSNumber)?.uint64Value,
               let fields = objc_getAssociatedObject(container, &fieldsKey) as? String,
               let pickerView = objc_getAssociatedObject(container, &pickerViewKey) as? UIView {
                let value = extractValueFromPicker(pickerView, fields: fields)
                sendResult(callback_id: callbackID, value: value)
            }
            dismissDatePicker()
        }, for: .touchUpInside)

        buttonStack.addArrangedSubview(cancelButton)
        buttonStack.addArrangedSubview(confirmButton)
        container.addSubview(pickerView)
        container.addSubview(buttonStack)
        background.addSubview(container)
        window.addSubview(background)

        NSLayoutConstraint.activate([
            background.topAnchor.constraint(equalTo: window.topAnchor),
            background.leadingAnchor.constraint(equalTo: window.leadingAnchor),
            background.trailingAnchor.constraint(equalTo: window.trailingAnchor),
            background.bottomAnchor.constraint(equalTo: window.bottomAnchor),

            container.leadingAnchor.constraint(equalTo: background.leadingAnchor),
            container.trailingAnchor.constraint(equalTo: background.trailingAnchor),
            container.bottomAnchor.constraint(equalTo: background.bottomAnchor),

            pickerView.topAnchor.constraint(equalTo: container.topAnchor, constant: 10),
            pickerView.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            pickerView.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            pickerView.heightAnchor.constraint(equalToConstant: pickerHeight),

            buttonStack.topAnchor.constraint(equalTo: pickerView.bottomAnchor, constant: 6),
            buttonStack.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 16),
            buttonStack.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -16),
            buttonStack.bottomAnchor.constraint(equalTo: container.safeAreaLayoutGuide.bottomAnchor, constant: -8),
            buttonStack.heightAnchor.constraint(equalToConstant: 46),

            cancelButton.heightAnchor.constraint(equalToConstant: 46),
            confirmButton.heightAnchor.constraint(equalToConstant: 46)
        ])

        container.transform = CGAffineTransform(translationX: 0, y: 400)

        UIView.animate(withDuration: 0.3, delay: 0, options: .curveEaseOut) {
            background.alpha = 1
            container.transform = .identity
        }
    }

    @MainActor @objc private static func backgroundTapped() {
        if let container = backgroundView?.subviews.first,
           let callbackID = (objc_getAssociatedObject(container, &callbackIDKey) as? NSNumber)?.uint64Value {
            LxAppPicker.sendPickerResultCancel(callback_id: callbackID)
        }
        dismissDatePicker()
    }

    @MainActor
    private static func createDatePickerView(mode: String, fields: String, value: Any?, start: String?, end: String?, callbackID: UInt64) -> UIView {
        let dateFormatter = DateFormatter()
        dateFormatter.dateFormat = "yyyy-MM-dd"

        if mode == "time" {
            let pickerView = TimeRangePickerView(
                start: start,
                end: end,
                initialValue: value as? String
            )
            pickerView.onValueChange = { timeValue in
                sendResultScroll(callback_id: callbackID, value: timeValue)
            }
            return pickerView
        } else if fields == "range" {
            let pickerView = DateRangePickerView()

            if let startString = start, let date = dateFormatter.date(from: startString) {
                pickerView.minimumDate = date
            }
            if let endString = end, let date = dateFormatter.date(from: endString) {
                pickerView.maximumDate = date
            }

            pickerView.setupForMode(singleSelection: false, showQuick: true)
            pickerView.onValueChange = { value in
                sendResultScroll(callback_id: callbackID, value: value)
            }

            if let valueArray = value as? [String], valueArray.count == 2,
               let startDate = dateFormatter.date(from: valueArray[0]),
               let endDate = dateFormatter.date(from: valueArray[1]) {
                pickerView.setInitialRange(start: startDate, end: endDate)
            }

            return pickerView
        } else if fields == "day" {
            let pickerView = DateRangePickerView()

            if let startString = start, let date = dateFormatter.date(from: startString) {
                pickerView.minimumDate = date
            }
            if let endString = end, let date = dateFormatter.date(from: endString) {
                pickerView.maximumDate = date
            }

            pickerView.setupForMode(singleSelection: true, showQuick: true)
            pickerView.onValueChange = { value in
                sendResultScroll(callback_id: callbackID, value: value)
            }

            if let dateString = value as? String, let date = dateFormatter.date(from: dateString) {
                pickerView.setInitialDate(date)
            }

            return pickerView
        } else if fields == "month" {
            var initialYear: Int?
            var initialMonth: Int?
            if let dateString = value as? String {
                let parts = dateString.split(separator: "-")
                if parts.count >= 1 { initialYear = Int(parts[0]) }
                if parts.count >= 2 { initialMonth = Int(parts[1]) }
            }
            
            let minYear = start.flatMap { Int($0.prefix(4)) } ?? 1970
            let maxYear = end.flatMap { Int($0.prefix(4)) } ?? 2100

            let pickerView = YearMonthPickerView(
                minYear: minYear,
                maxYear: maxYear,
                initialYear: initialYear,
                initialMonth: initialMonth
            )
            return pickerView
        } else if fields == "year" {
            var initialYear: Int?
            if let dateString = value as? String {
                initialYear = Int(dateString)
            }
            
            let minYear = start.flatMap { Int($0.prefix(4)) } ?? 1970
            let maxYear = end.flatMap { Int($0.prefix(4)) } ?? 2100

            let pickerView = YearPickerView(
                minYear: minYear,
                maxYear: maxYear,
                initialYear: initialYear
            )
            return pickerView
        } else {
            let pickerView = DateRangePickerView()
            if let startString = start, let date = dateFormatter.date(from: startString) {
                pickerView.minimumDate = date
            }
            if let endString = end, let date = dateFormatter.date(from: endString) {
                pickerView.maximumDate = date
            }

            pickerView.setupForMode(singleSelection: true, showQuick: true)
            pickerView.onValueChange = { value in
                sendResultScroll(callback_id: callbackID, value: value)
            }

            if let dateString = value as? String, let date = dateFormatter.date(from: dateString) {
                pickerView.setInitialDate(date)
            }

            return pickerView
        }
    }

    @MainActor
    private static func extractValueFromPicker(_ pickerView: UIView, fields: String) -> Any {
        let dateFormatter = DateFormatter()
        dateFormatter.dateFormat = "yyyy-MM-dd"

        if let timePicker = pickerView as? TimeRangePickerView {
            return timePicker.currentValueString()
        }

        if fields == "range" {
            if let rangeView = pickerView as? DateRangePickerView,
               let range = rangeView.selectedRange {
                return [dateFormatter.string(from: range.start), dateFormatter.string(from: range.end)]
            }
            let today = Date()
            return [dateFormatter.string(from: today), dateFormatter.string(from: today)]
        }

        if let calendarView = pickerView as? DateRangePickerView {
            if let selectedDate = calendarView.selectedDate {
                return dateFormatter.string(from: selectedDate)
            }
            return dateFormatter.string(from: Date())
        }

        if let yearPicker = pickerView as? YearPickerView {
            return "\(yearPicker.selectedYear)"
        }

        if let yearMonthPicker = pickerView as? YearMonthPickerView {
            return String(format: "%04d-%02d", yearMonthPicker.selectedYear, yearMonthPicker.selectedMonth)
        }

        return ""
    }
    
    @MainActor
    private static func sendResultScroll(callback_id: UInt64, value: Any) {
        let payload: [String: Any] = ["value": value]
        guard let jsonData = try? JSONSerialization.data(withJSONObject: payload),
              let jsonString = String(data: jsonData, encoding: .utf8),
              let localCallback = LxAppPicker.localCallbacks[callback_id] else { return }
        localCallback(true, jsonString)
    }

    @MainActor
    private static func sendResult(callback_id: UInt64, value: Any) {
        let payload: [String: Any] = ["confirm": true, "value": value]
        guard let jsonData = try? JSONSerialization.data(withJSONObject: payload),
              let jsonString = String(data: jsonData, encoding: .utf8),
              let localCallback = LxAppPicker.localCallbacks[callback_id] else { return }
        localCallback(true, jsonString)
    }

    @MainActor
    private static func sendError(callback_id: UInt64) {
        if let localCallback = LxAppPicker.localCallbacks[callback_id] {
            localCallback(false, "1000")
        }
    }

    @MainActor
    static func dismissDatePicker() {
        guard let backgroundView = backgroundView else { return }

        UIView.animate(withDuration: 0.3, animations: {
            backgroundView.alpha = 0
            if let containerView = backgroundView.subviews.first {
                containerView.transform = CGAffineTransform(translationX: 0, y: 400)
            }
        }) { _ in
            backgroundView.removeFromSuperview()
            self.backgroundView = nil
            self.currentWindow = nil
        }
    }

    @MainActor
    private static func resolveColor(_ value: String, fallback: UIColor) -> UIColor {
        guard value.hasPrefix("#") else { return fallback }
        let defaultArgb = LxAppColorUtils.argbValue(from: fallback.resolvedColor(with: UIScreen.main.traitCollection))
        let argb = LxAppColorUtils.parseColorString(value, defaultColor: defaultArgb)
        return LxAppColorUtils.platformColor(from: argb)
    }
}

#endif
