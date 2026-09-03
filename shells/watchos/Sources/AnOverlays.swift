import SwiftUI

/// What the system presents on top: `an-alert` and `an-modal`.
///
/// They are not views to be placed. In SwiftUI they are modifiers on the root
/// —`.alert`, `.sheet`, `.fullScreenCover`— and what decides where they go and
/// how they come in is the system, not taffy. That is why Rust takes them out of
/// the tree and sends them apart, in `overlays`, and why this modifier hangs off
/// the root view and not off a node.
///
/// It is the same thing iOS does by presenting a real `UIAlertController`
/// instead of drawing a lookalike layer: the system knows there is something
/// modal in front, so VoiceOver stops reading what is behind and it does not
/// compete in draw order with the watch's own dialogs.
struct AnOverlays: ViewModifier {
    let overlays: [AnNode]
    let controls: AnControls
    let dispatch: (UInt32, String, [String: Any]) -> Void
    let crownFocus: FocusState<UInt32?>.Binding

    func body(content: Content) -> some View {
        // One at a time and in the order they appear in the tree: stacking
        // several `.alert`s on the same view is legal and the system queues them.
        overlays.reduce(AnyView(content)) { view, overlay in
            switch overlay.kind {
            case "Alert":
                AnyView(
                    view.modifier(
                        AnAlert(node: overlay, controls: controls, dispatch: dispatch)
                    )
                )
            case "Modal":
                AnyView(
                    view.modifier(
                        AnModal(
                            node: overlay,
                            controls: controls,
                            dispatch: dispatch,
                            crownFocus: crownFocus
                        )
                    )
                )
            default: view
            }
        }
    }
}

/// The system dialog.
///
/// The `isPresented` `Binding` is also written when the user closes it —by
/// tapping outside, or with the watch's back button—: were that not reported,
/// the signal that opened it would go on saying `true` and setting it again
/// would do nothing. It is the same reason `an-modal` has `(dismiss)`.
private struct AnAlert: ViewModifier {
    let node: AnNode
    let controls: AnControls
    let dispatch: (UInt32, String, [String: Any]) -> Void

    func body(content: Content) -> some View {
        content.alert(
            node.title ?? "",
            isPresented: controls.presented(node),
            actions: {
                // With no buttons an "OK" comes out, as on iOS: a dialog there
                // is no way out of is not a dialog.
                let labels = node.buttons?.isEmpty == false ? node.buttons! : ["OK"]
                ForEach(Array(labels.enumerated()), id: \.offset) { index, label in
                    Button(label) { dispatch(node.id, "select", ["index": index]) }
                }
            },
            message: {
                if let message = node.message, !message.isEmpty {
                    Text(message)
                }
            }
        )
    }
}

/// Content presented on top of everything.
///
/// On the watch there is no visible difference between `sheet` and `fullScreen`
/// —a sheet takes up the screen just the same—, but there is in how it is
/// closed: the sheet can be pulled down with a finger and the full-screen one
/// cannot. Whatever `[presentation]` says is honoured rather than picking one
/// for both.
private struct AnModal: ViewModifier {
    let node: AnNode
    let controls: AnControls
    let dispatch: (UInt32, String, [String: Any]) -> Void
    let crownFocus: FocusState<UInt32?>.Binding

    func body(content: Content) -> some View {
        if node.presentation == "fullScreen" {
            content.fullScreenCover(isPresented: isPresented) { inside }
        } else {
            content.sheet(isPresented: isPresented) { inside }
        }
    }

    private var isPresented: Binding<Bool> {
        controls.presented(node)
    }

    /// Inside, taffy is still in charge: the `an-modal`'s children carry their
    /// frame relative to it, so they are placed as in any other container.
    private var inside: some View {
        ZStack(alignment: .topLeading) {
            ForEach(node.children ?? []) { child in
                AnNodeView(
                    node: child,
                    controls: controls,
                    dispatch: dispatch,
                    crownFocus: crownFocus
                )
            }
        }
        .frame(width: node.width, height: node.height, alignment: .topLeading)
        .ignoresSafeArea()
    }
}

extension View {
    func anOverlays(
        _ overlays: [AnNode],
        controls: AnControls,
        dispatch: @escaping (UInt32, String, [String: Any]) -> Void,
        crownFocus: FocusState<UInt32?>.Binding
    ) -> some View {
        modifier(
            AnOverlays(
                overlays: overlays,
                controls: controls,
                dispatch: dispatch,
                crownFocus: crownFocus
            )
        )
    }
}
