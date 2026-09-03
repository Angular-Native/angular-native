import Foundation
import Observation
import SwiftUI

/// What a control holds inside while it is being touched.
enum AnControlValue: Equatable {
    case number(Double)
    case text(String)
    case flag(Bool)
    case index(Int)
}

/// The controls' state, which is the only thing the shell keeps on its own.
///
/// It is needed because both sides carry the same datum and do not run at the
/// same rhythm. A SwiftUI `Slider` needs a `Binding` it can write to on the spot
/// —the finger is on it— while the real value lives in an Angular signal, on the
/// other side of QuickJS, and does not come back until the next frame. Without
/// this middle layer the slider would jump backwards on every drag, because
/// every snapshot would return it to the old value.
///
/// There is a single reconciliation rule and it is what keeps the shell and the
/// app from fighting:
///
/// > if the value arriving from Rust **differs from the one that arrived last
/// > time**, the app changed it, and the app wins. If it is the same, whatever
/// > the finger touched wins.
///
/// That way a signal that changes the value from code shows straight away, and a
/// drag is not trampled by the echo of its own event.
///
/// And for the same reason the state survives a hot reload: `an dev` keeps the
/// tree mounted and the ids, so nothing is touched here; what disappears from
/// the tree is cleaned up in `reconcile`, which is also what empties this when a
/// cold reload throws the whole tree away.
@MainActor
@Observable
final class AnControls {
    /// What is being touched right now.
    private var locals: [UInt32: AnControlValue] = [:]
    /// The last thing Rust said, so it can be told whether a new value comes
    /// from the app.
    private var mirrored: [UInt32: AnControlValue] = [:]
    /// Presentations the system has already closed and whose template has not
    /// found out yet.
    ///
    /// Without this a dialog would reopen on its own: on pressing a button
    /// SwiftUI closes it, but `[visible]` is still `true` until JS reacts to the
    /// `(select)`, and the frame in between would present it again.
    private var closed: Set<UInt32> = []
    /// How far the crown has turned on each node.
    ///
    /// It is kept apart from `locals` rather than sharing a slot with the
    /// controls' values because one node can be both things: an `an-slider` with
    /// a `(crown)` on it exists, and with a single dictionary the slider's value
    /// would run over the accumulated rotation as soon as the app changed it.
    private var turns: [UInt32: Double] = [:]

    /// The way changes go out towards JS.
    @ObservationIgnored var dispatch: (UInt32, String, [String: Any]) -> Void = { _, _, _ in }

    /// Reconciles the state with the snapshot that has just arrived and forgets
    /// the nodes that are no longer there.
    func reconcile(root: AnNode?, overlays: [AnNode]) {
        var alive: Set<UInt32> = []
        if let root {
            walk(root, &alive)
        }
        for overlay in overlays {
            walk(overlay, &alive)
        }
        locals = locals.filter { alive.contains($0.key) }
        mirrored = mirrored.filter { alive.contains($0.key) }
        closed = closed.filter { alive.contains($0) }
        turns = turns.filter { alive.contains($0.key) }
    }

    private func walk(_ node: AnNode, _ alive: inout Set<UInt32>) {
        alive.insert(node.id)
        // The template has found out it was closed: next time it opens it, it
        // has to be allowed to.
        if node.visible == false {
            closed.remove(node.id)
        }
        if let incoming = Self.declared(node) {
            // A value different from the previous one can only come from the
            // app: the echo of the finger already arrived in the last snapshot.
            if mirrored[node.id] != incoming {
                locals[node.id] = incoming
            }
            mirrored[node.id] = incoming
        }
        for child in node.children ?? [] {
            walk(child, &alive)
        }
    }

    /// The value the template declares for this node, if it is a control.
    private static func declared(_ node: AnNode) -> AnControlValue? {
        switch node.kind {
        case "Switch": return .flag(node.on ?? false)
        case "Slider", "Stepper", "DatePicker": return .number(node.value ?? 0)
        case "TextInput": return .text(node.field ?? "")
        case "Picker": return .index(node.selectedIndex ?? 0)
        default: return nil
        }
    }

    private func current(_ node: AnNode) -> AnControlValue? {
        locals[node.id] ?? Self.declared(node)
    }

    // ------------------------------------------------------------- bindings

    /// A number: `an-slider`, `an-stepper`.
    ///
    /// The event goes out under the same key as on iOS —`change` with `value`—
    /// because what receives it is the directive's own `outputFromObservable`,
    /// not any watch-specific code.
    func number(_ node: AnNode) -> Binding<Double> {
        Binding(
            get: {
                if case .number(let value)? = self.current(node) { return value }
                return node.value ?? 0
            },
            set: { value in
                self.locals[node.id] = .number(value)
                self.dispatch(node.id, "change", ["value": value])
            }
        )
    }

    /// A date. It travels as milliseconds since 1970 in both directions, which
    /// is what `Date` gives and takes in JS: formatting it depends on the
    /// device's language and time zone, and the system already takes care of that
    /// when painting it.
    func date(_ node: AnNode) -> Binding<Date> {
        let millis = number(node)
        return Binding(
            get: { Date(timeIntervalSince1970: millis.wrappedValue / 1000.0) },
            set: { millis.wrappedValue = $0.timeIntervalSince1970 * 1000.0 }
        )
    }

    func flag(_ node: AnNode) -> Binding<Bool> {
        Binding(
            get: {
                if case .flag(let value)? = self.current(node) { return value }
                return node.on ?? false
            },
            set: { value in
                self.locals[node.id] = .flag(value)
                self.dispatch(node.id, "change", ["value": value])
            }
        )
    }

    func text(_ node: AnNode) -> Binding<String> {
        Binding(
            get: {
                if case .text(let value)? = self.current(node) { return value }
                return node.field ?? ""
            },
            set: { value in
                self.locals[node.id] = .text(value)
                self.dispatch(node.id, "change", ["value": value])
            }
        )
    }

    /// The option selected in an `an-select`. The event carries `index`, not
    /// `value`: it is what `NativeIndexEvent` expects in the directive.
    func index(_ node: AnNode) -> Binding<Int> {
        Binding(
            get: {
                if case .index(let value)? = self.current(node) { return value }
                return node.selectedIndex ?? 0
            },
            set: { value in
                self.locals[node.id] = .index(value)
                self.dispatch(node.id, "change", ["index": value])
            }
        )
    }

    // ------------------------------------------------- what gets presented

    /// Whether an `an-alert` or an `an-modal` is on screen.
    ///
    /// Writing `false` is the system saying it has closed it —a button, the
    /// gesture of pulling the sheet down—, and that has to be reported:
    /// otherwise the signal that opened it goes on saying it is still open and
    /// setting it back to `true` does nothing.
    func presented(_ node: AnNode) -> Binding<Bool> {
        Binding(
            get: { node.visible == true && !self.closed.contains(node.id) },
            set: { open in
                guard !open else { return }
                self.closed.insert(node.id)
                self.dispatch(node.id, "dismiss", [:])
            }
        )
    }

    // --------------------------------------------------------- the crown

    /// How far the crown has turned on this node.
    ///
    /// SwiftUI's crown is asked for with a `Binding` over a number it moves; the
    /// event that matters —how far and how fast— arrives separately, through
    /// `onChange`. This accumulator exists because without somewhere to keep it
    /// the rotation would reset on every snapshot, and with that the crown would
    /// drag the value back to where it was.
    func crown(_ id: UInt32) -> Binding<Double> {
        Binding(
            get: { self.turns[id] ?? 0 },
            set: { self.turns[id] = $0 }
        )
    }
}
