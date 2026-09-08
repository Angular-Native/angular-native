import Foundation
import Observation

/// The observable model SwiftUI looks at.
///
/// It is the mirror of the tree Rust maintains. It is not built from Swift: it is
/// decoded from the snapshot `an_watch_runtime_snapshot` returns, and only when
/// the revision changes. On a settled frame nothing is decoded and SwiftUI does
/// not recompose.
@Observable
final class AnTree {
    private(set) var root: AnNode?
    /// What the system presents on top —`an-alert` and `an-modal`—, which
    /// arrives outside the tree because in SwiftUI they are not views to be
    /// placed but modifiers on the root.
    private(set) var overlays: [AnNode] = []
    /// The revision already mirrored here. It starts out of range so that the
    /// first frame always gets in.
    private var mirrored: UInt64 = .max

    /// Rust sends `border_radius`; Swift wants it as `borderRadius`. The
    /// conversion is done by the decoder with a rule, not with a forty-line
    /// `CodingKeys` table that would have to be touched in two places every time
    /// the snapshot grows.
    private static let decoder: JSONDecoder = {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return decoder
    }()

    /// Dumps the snapshot if Rust says it changed. It returns whether there was
    /// a change, which is what the shell looks at to know whether recording the
    /// frame makes any sense.
    @discardableResult
    func sync(runtime: OpaquePointer?) -> Bool {
        guard let runtime else { return false }
        let revision = an_watch_runtime_revision(runtime)
        guard revision != mirrored else { return false }
        mirrored = revision

        guard let raw = an_watch_runtime_snapshot(runtime) else {
            NSLog("angular-native: the tree could not be serialised")
            return false
        }
        // `String(cString:)` copies. The Rust pointer is only good until the
        // next call, so it is not kept.
        let json = String(cString: raw)
        guard let data = json.data(using: .utf8) else { return false }
        do {
            let snapshot = try Self.decoder.decode(AnSnapshot.self, from: data)
            root = snapshot.root
            overlays = snapshot.overlays ?? []
            return true
        } catch {
            NSLog("angular-native: the tree snapshot cannot be understood: \(error)")
            return false
        }
    }
}

struct AnSnapshot: Decodable {
    let revision: UInt64
    let root: AnNode?
    let overlays: [AnNode]?
}

/// A node exactly as Rust sends it.
///
/// `Identifiable` with the id JS assigned, which is stable between frames:
/// without it SwiftUI would treat every snapshot as new content and would lose,
/// among other things, the scroll position on every change.
///
/// Everything is optional except what every node has. Rust omits whatever does
/// not apply to the kind —a `Text` does not send `minimum`— and that way a
/// screen's snapshot can be read at a glance when it has to be debugged.
struct AnNode: Decodable, Identifiable, Equatable {
    let id: UInt32
    let kind: String
    let x: Double
    let y: Double
    let width: Double
    let height: Double

    /// Why the watch does not paint this. Rust decides it, which is where the
    /// list and the reason live; here only the gap is left.
    let unsupported: String?

    let background: [Double]?
    let color: [Double]?
    let borderRadius: Double?
    /// The four corners clockwise from the top left, and only when they are not
    /// all the same. `AnNode.anCornerShape` is what turns either of the two into
    /// the one shape the clip and the border share.
    let borderRadii: [Double]?
    let borderWidth: Double?
    let borderColor: [Double]?
    let opacity: Double?
    /// Whether this node keeps its children inside its own frame — the
    /// resolved `overflow`. Absent means false: the core leaves the key out
    /// rather than send it on every node. See `anClips`.
    let clip: Bool?
    let disabled: Bool?
    let testId: String?

    /// The six of the contract, already translated by Rust.
    /// `accessibilityTraits` are `AccessibilityTraits` names —`isButton`,
    /// `isSelected`— and not the contract's role: the table lives in
    /// `snapshot.rs`, which is also what says what has no shape in SwiftUI.
    /// See `AnAccessibility`.
    let accessibilityLabel: String?
    let accessibilityHint: String?
    let accessibilityValue: String?
    let accessibilityTraits: [String]?
    let accessible: Bool?

    let text: String?
    let fontSize: Double?
    let fontWeight: Int?
    let italic: Bool?
    let fontFamily: String?
    let letterSpacing: Double?
    let textAlign: String?
    let textDecoration: String?
    let maxLines: Int?

    let on: Bool?
    let value: Double?
    let minimum: Double?
    let maximum: Double?
    let step: Double?
    let progress: Double?
    let animating: Bool?
    let field: String?
    let placeholder: String?
    let secure: Bool?
    let keyboard: String?
    let items: [String]?
    let selectedIndex: Int?
    let dateMode: String?
    let symbol: String?
    let symbolSize: Double?
    let symbolWeight: Int?
    let source: String?
    let resizeMode: String?

    let visible: Bool?
    let title: String?
    let message: String?
    let buttons: [String]?
    let presentation: String?
    let transition: String?

    let contentWidth: Double?
    let contentHeight: Double?

    /// What the template listens for on this node. The shell only hooks up what
    /// is here: one recogniser too many would eat the gestures of the
    /// `ScrollView` underneath, and on the watch the system also highlights
    /// whatever it believes to be tappable.
    let listens: [String]?

    let children: [AnNode]?

    func listens(to event: String) -> Bool {
        listens?.contains(event) == true
    }
}
