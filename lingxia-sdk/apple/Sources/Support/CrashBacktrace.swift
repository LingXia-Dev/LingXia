import Foundation

/// Prints a backtrace to stderr when the process dies by SIGTRAP.
///
/// Headless CI runners do not always hand the trap corpse to ReportCrash, so
/// a trapped host left no .ips and the only evidence was "terminated by
/// signal 5". stderr is inherited by `lingxia dev`, so the frames land in
/// the dev/session log next to the runtime trail. The handler re-raises with
/// the default disposition, keeping the exit status a trap.
public enum CrashBacktrace {
    nonisolated(unsafe) private static var installed = false

    public nonisolated static func install() {
        guard !installed else { return }
        installed = true
        signal(SIGTRAP) { _ in
            var frames = [UnsafeMutableRawPointer?](repeating: nil, count: 64)
            let count = backtrace(&frames, Int32(frames.count))
            if let symbols = backtrace_symbols(&frames, count) {
                for index in 0 ..< Int(count) {
                    guard let line = symbols[index] else { continue }
                    _ = write(STDERR_FILENO, line, strlen(line))
                    _ = write(STDERR_FILENO, "\n", 1)
                }
            }
            signal(SIGTRAP, SIG_DFL)
            raise(SIGTRAP)
        }
    }
}
