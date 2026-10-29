import Foundation

extension GpuPageView {
    /// The user's clock, `h12` when hours carry a day period: the system setting overrides the
    /// language's custom, so the View's time picker follows what the status bar shows.
    static func hourCycle() -> String {
        let format = DateFormatter.dateFormat(fromTemplate: "j", options: 0, locale: Locale.current) ?? ""
        return format.contains("a") ? "h12" : "h23"
    }
}
