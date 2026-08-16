import Foundation

/// The two business codes the user-dismissable `lx.*` family speaks.
///
/// `userDismissed` is the whole contract behind that family: the Logic layer
/// turns it into `{ canceled: true }` and anything else into a rejection. An
/// adapter that sends it for a real failure therefore reports a crash as "the
/// user said no" — silently, and in the direction where the caller keeps going.
/// Named here and in `crates/lingxia-logic/src/dismissal.rs` so both ends of
/// the boundary stay greppable.
enum LxAppDismissal {
    /// The user closed the UI without choosing.
    static let userDismissedCode = "2000"
    /// The operation failed. Never send this for a dismissal, or the reverse.
    static let failureCode = "1000"
}
