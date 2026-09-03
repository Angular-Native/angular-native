import SwiftUI

/// The gestures the watch does have, hooked up only where the template asked for
/// them.
///
/// They are hooked up by list and not always, for two reasons that show on
/// screen: one recogniser too many eats the drag of the `ScrollView` underneath,
/// and watchOS highlights whatever it believes to be tappable, so wrapping
/// everything in a gesture would make half the screen flicker when brushed.
///
/// What is not here is not on the watch, and it is said from Rust —once per
/// gesture— rather than hooked up to something that never responds:
///
/// - `(pinch)` and `(rotation)`: `MagnifyGesture` and `RotateGesture` are marked
///   `@available(watchOS, unavailable)`. A 40 mm screen does not take two
///   fingers.
/// - `(back)`: outside a `NavigationStack` the watch does not give the drag from
///   the edge, and setting one up would put SwiftUI's navigation —with its bar
///   and its layout— inside a tree taffy lays out.
/// - `(scroll)` and `(refresh)`: SwiftUI's `ScrollView` publishes its offset
///   nowhere that works on watchOS 11, which is the shell's minimum.
struct AnGestures: ViewModifier {
    let node: AnNode
    let dispatch: (UInt32, String, [String: Any]) -> Void
    /// `true` when the control itself already sends the `(press)` —a SwiftUI
    /// `Button` does, with its highlight and its haptic— and hooking a tap onto
    /// it as well would send it twice.
    let ownsTheTap: Bool

    /// Where the finger was last time.
    ///
    /// SwiftUI gives the position neither in `onTapGesture` nor in
    /// `onLongPressGesture`, and the iOS `(press)` carries it. It is taken from a
    /// zero-distance drag, which is a real gesture and not an invented
    /// coordinate: if there has been no contact, nothing is sent.
    @State private var point: CGPoint?
    /// Whether the drag under way has already announced that it was starting.
    @State private var dragging = false

    /// How far it has to travel to count as a swipe. `UIKit` uses a similar
    /// threshold in its swipe recogniser; here it is written out because in
    /// SwiftUI there is no swipe recogniser and the threshold is set by whoever
    /// builds one.
    private static let swipeThreshold: CGFloat = 24

    func body(content: Content) -> some View {
        var view = AnyView(content)
        if wantsTouch {
            view = AnyView(view.contentShape(Rectangle()))
        }
        if (wantsPress || node.listens(to: "longPress")) && !wantsDrag {
            // Zero distance: it moves nothing, it only notes where the finger is.
            // It is only added when there is no real drag: two `DragGesture`s on
            // the same view fight over the finger and one of the two wins, so
            // when there is one, the position comes from it.
            view = AnyView(
                view.simultaneousGesture(
                    DragGesture(minimumDistance: 0).onChanged { point = $0.location }
                )
            )
        }
        if wantsPress {
            view = AnyView(view.onTapGesture { send("press", position()) })
        }
        if node.listens(to: "doublePress") {
            view = AnyView(view.onTapGesture(count: 2) { send("doublePress", position()) })
        }
        if node.listens(to: "longPress") {
            // `simultaneousGesture` and not `.onLongPressGesture`: the latter is
            // installed as an ordinary gesture and the drag below eats it, so on
            // a view that also listens for `(pan)` or `(swipe*)` it never
            // arrived.
            view = AnyView(
                view.simultaneousGesture(
                    LongPressGesture(minimumDuration: 0.5).onEnded { _ in
                        send("longPress", position())
                    }
                )
            )
        }
        if wantsDrag {
            view = AnyView(view.gesture(drag))
        }
        return view
    }

    private var wantsPress: Bool {
        node.listens(to: "press") && !ownsTheTap
    }

    private var wantsTouch: Bool {
        wantsPress || node.listens(to: "doublePress")
            || node.listens(to: "longPress") || wantsDrag
    }

    private var wantsDrag: Bool {
        node.listens(to: "pan") || wantsSwipe
    }

    private var wantsSwipe: Bool {
        node.listens(to: "swipeLeft") || node.listens(to: "swipeRight")
            || node.listens(to: "swipeUp") || node.listens(to: "swipeDown")
    }

    /// The drag, which feeds both `(pan)` and the four `(swipe*)` at once.
    ///
    /// A single recogniser for both because they are the same gesture looked at
    /// two ways: `pan` counts the travel as it happens and `swipe` looks at the
    /// balance on release. Two `DragGesture`s on the same view would fight.
    private var drag: some Gesture {
        DragGesture(minimumDistance: 1)
            .onChanged { value in
                // The position is always noted, whoever listens for it: it is the
                // one a long press on this same view carries.
                point = value.location
                guard node.listens(to: "pan") else { return }
                let phase = dragging ? "move" : "begin"
                dragging = true
                send("pan", payload(value, phase))
            }
            .onEnded { value in
                if node.listens(to: "pan") {
                    send("pan", payload(value, "end"))
                }
                dragging = false
                swipe(value)
            }
    }

    private func swipe(_ value: DragGesture.Value) {
        guard wantsSwipe else { return }
        let dx = value.translation.width
        let dy = value.translation.height
        // The dominant axis wins: a diagonal drag is a swipe in one direction,
        // not in two.
        let name: String
        if abs(dx) >= abs(dy) {
            guard abs(dx) >= Self.swipeThreshold else { return }
            name = dx < 0 ? "swipeLeft" : "swipeRight"
        } else {
            guard abs(dy) >= Self.swipeThreshold else { return }
            name = dy < 0 ? "swipeUp" : "swipeDown"
        }
        guard node.listens(to: name) else { return }
        send(name, ["x": value.location.x, "y": value.location.y])
    }

    private func payload(_ value: DragGesture.Value, _ phase: String) -> [String: Any] {
        [
            "x": value.location.x,
            "y": value.location.y,
            "translationX": value.translation.width,
            "translationY": value.translation.height,
            "velocityX": value.velocity.width,
            "velocityY": value.velocity.height,
            "state": phase
        ]
    }

    /// The position of the last contact, if there was one.
    ///
    /// With no contact, nothing is sent rather than a zero: a zero is a specific
    /// corner of the view and whoever received it would believe it.
    private func position() -> [String: Any] {
        guard let point else { return [:] }
        return ["x": point.x, "y": point.y]
    }

    private func send(_ name: String, _ payload: [String: Any]) {
        dispatch(node.id, name, payload)
    }
}

extension View {
    func anGestures(
        _ node: AnNode,
        dispatch: @escaping (UInt32, String, [String: Any]) -> Void,
        ownsTheTap: Bool = false
    ) -> some View {
        modifier(AnGestures(node: node, dispatch: dispatch, ownsTheTap: ownsTheTap))
    }
}
