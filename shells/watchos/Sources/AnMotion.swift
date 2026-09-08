import SwiftUI

/// What moves: the transform a node carries, and the animation that takes it to
/// the next one.
///
/// SwiftUI is a better fit for this than UIKit, not a worse one. `.offset`,
/// `.scaleEffect` and `.rotationEffect` are one modifier each, and
/// `.animation(_:value:)` needs no starting state handed to it the way
/// `UIView.animate` does — the value it came from is still in the view tree.
/// What was missing was never the drawing: nothing carried the numbers across.
///
/// What an `[animate]` covers is the frame, the opacity and the transform. That
/// is not a decision taken here: it is the three things `an-ios` puts inside an
/// animation block, and a colour or a label changing is a jump on every host.
extension AnNode {
    /// The animation this node asked for, and `nil` when it asked for none.
    var anAnimation: Animation? {
        // Rust does not send a duration of zero, so a value that arrives here
        // is one somebody wants.
        guard let milliseconds = animate, milliseconds > 0 else { return nil }
        let seconds = milliseconds / 1000
        let curve: Animation =
            switch animateEasing {
            case "linear": .linear(duration: seconds)
            case "ease-in": .easeIn(duration: seconds)
            case "ease-in-out": .easeInOut(duration: seconds)
            // `ease-out` is the default, and it is the one wanted nearly
            // always: away fast and braking on arrival. It is the default
            // `an-ios` picks too, and the two have to agree or the same
            // template moves differently on the phone.
            default: .easeOut(duration: seconds)
            }
        guard let delay = animateDelay, delay > 0 else { return curve }
        return curve.delay(delay / 1000)
    }

    /// Everything `[animate]` covers, in the one `Equatable` value
    /// `.animation(_:value:)` compares between frames.
    ///
    /// It is deliberately not the node itself: with `AnNode` as the value, a
    /// label being edited or a colour changing would start the animation and
    /// the frame would slide for it.
    var anMotion: AnMotion {
        AnMotion(
            x: x,
            y: y,
            width: width,
            height: height,
            opacity: opacity ?? 1,
            translateX: translateX ?? 0,
            translateY: translateY ?? 0,
            scaleX: scaleX ?? 1,
            scaleY: scaleY ?? 1,
            rotate: rotate ?? 0
        )
    }
}

/// The frame, the opacity and the transform: a change to one of these is
/// animated, and a change to anything else is not.
struct AnMotion: Equatable {
    let x: Double
    let y: Double
    let width: Double
    let height: Double
    let opacity: Double
    let translateX: Double
    let translateY: Double
    let scaleX: Double
    let scaleY: Double
    let rotate: Double
}

/// The transform, applied to the frame taffy gave and before the node is
/// placed.
///
/// Before `.position` and not after, and that is not a preference: `.position`
/// hands back a view the size of the whole container with the content placed
/// inside it, so a `.scaleEffect` after it would scale that container about
/// *its* centre and the node would travel across the screen instead of growing
/// where it stands.
///
/// The order is scale, then rotate, then translate, which is the order
/// `an-ios` multiplies its matrix in. The other way round the rotation turns
/// the translation with it, and dragging something tilted goes off diagonally
/// instead of following the finger.
///
/// It is attached to every node, carrying a transform or not. A conditional
/// modifier is a different view type and SwiftUI resets the identity of the
/// subtree when the branch flips, so a template animating `translateX` away
/// from zero would lose the animation on the very frame it starts.
private struct AnTransform: ViewModifier {
    let node: AnNode

    func body(content: Content) -> some View {
        content
            .scaleEffect(x: node.scaleX ?? 1, y: node.scaleY ?? 1)
            .rotationEffect(.radians(node.rotate ?? 0))
            .offset(x: node.translateX ?? 0, y: node.translateY ?? 0)
    }
}

extension View {
    /// Moves, scales and turns the node, without any of it reaching the
    /// layout: the node goes on occupying the box taffy measured for it, which
    /// is what the contract promises and what makes a transform cheap.
    func anTransform(_ node: AnNode) -> some View {
        modifier(AnTransform(node: node))
    }
}
