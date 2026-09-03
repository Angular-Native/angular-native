import Foundation

/// Warnings that are said once.
///
/// The tree is rebuilt thirty times a second, so a warning put where the drawing
/// happens would be said thirty times a second and the log would stop being good
/// for anything. The Rust side has the same thing, for the same reason, in
/// `snapshot.rs`.
///
/// It is `@MainActor` and not a `Mutex` because everything that warns —drawing,
/// decoding the picture, reconciling the controls— happens on the main thread: a
/// lock here would be a lock nobody contends.
@MainActor
enum AnWarnings {
    private static var said: Set<String> = []

    static func once(_ key: String, _ say: () -> Void) {
        guard said.insert(key).inserted else { return }
        say()
    }
}
