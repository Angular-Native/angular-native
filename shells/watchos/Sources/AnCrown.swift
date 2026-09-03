import SwiftUI

/// The digital crown as a source of events.
///
/// It is the watch's own gesture and the only one with no equivalent on the
/// phone: an analogue dial, with inertia and haptics, used without covering the
/// screen with a finger. Until now it only moved the `ScrollView`, which is what
/// SwiftUI does on its own; with this a template can ask for it:
///
/// ```html
/// <an-view (crown)="turn($event)"> … </an-view>
/// ```
///
/// The event carries what `DigitalCrownEvent` gives plus the running total:
///
/// | key | what it is |
/// |---|---|
/// | `delta` | how far it has turned since the previous event |
/// | `offset` | the total since the view took the crown |
/// | `velocity` | revolutions per second, signed |
///
/// `delta` is not given by SwiftUI: it is worked out here by subtracting,
/// because what is almost always wanted is "raise the value by however far it
/// turned", and doing that subtraction in each template would mean repeating it
/// in each template.
///
/// **Only one view can hold the crown at a time.** It is not a limitation of the
/// shell: on watchOS the crown goes to whatever has the focus, and there is one
/// focus. When there are several views with `(crown)`, the first in drawing
/// order keeps it; touching a `Slider`, a `Stepper` or a `Picker` hands the
/// focus to that control, which is what anybody using a watch expects, and on
/// coming back the view has to be touched to get it again.
struct AnCrown: ViewModifier {
    let node: AnNode
    let controls: AnControls
    let dispatch: (UInt32, String, [String: Any]) -> Void
    let focus: FocusState<UInt32?>.Binding

    /// How far it had turned the previous time, so the step can be sent.
    @State private var previous: Double = 0

    func body(content: Content) -> some View {
        if node.listens(to: "crown") {
            content
                // Without `focusable` the crown does not arrive: watchOS sends
                // it to whatever has the focus, and an ordinary view cannot take
                // it.
                .focusable(true)
                .focused(focus, equals: node.id)
                // The starting focus. On the watch the crown goes with the
                // focus, and a view that asks for it without holding the focus
                // receives absolutely nothing; `defaultFocus` is the way of
                // saying "if nobody has it, let it be this one" without taking it
                // from whoever does. Assigning the `FocusState` by hand will not
                // do: SwiftUI swallows it silently if the view is not on screen
                // yet.
                .defaultFocus(focus, node.id)
                .digitalCrownRotation(
                    controls.crown(node.id),
                    onChange: { event in
                        let delta = event.offset - previous
                        previous = event.offset
                        dispatch(
                            node.id,
                            "crown",
                            ["delta": delta, "offset": event.offset, "velocity": event.velocity]
                        )
                    },
                    onIdle: {
                        // It stopped. It goes as its own event and not as a
                        // `crown` with a zero delta, because "it has stopped
                        // turning" is a different thing from "it has turned
                        // nothing".
                        dispatch(node.id, "crownIdle", [:])
                    }
                )
        } else {
            content
        }
    }
}

extension View {
    func anCrown(
        _ node: AnNode,
        controls: AnControls,
        dispatch: @escaping (UInt32, String, [String: Any]) -> Void,
        focus: FocusState<UInt32?>.Binding
    ) -> some View {
        modifier(AnCrown(node: node, controls: controls, dispatch: dispatch, focus: focus))
    }
}
