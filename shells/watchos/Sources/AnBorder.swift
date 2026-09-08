import SwiftUI

/// The outline of a node: the shape its corners give it, and the line the
/// template asked for along that shape.
///
/// Why the two live in one file: a border that did not follow the same corners
/// as the clip shows straight away — a straight line cutting the corner off a
/// rounded box. So the shape is worked out once, from the node, and
/// `AnNodeView` both clips with it and strokes with it.
extension AnNode {
    /// The node's outline, in SwiftUI's names.
    ///
    /// `UnevenRoundedRectangle` is used even for the four-the-same case rather
    /// than switching to `RoundedRectangle`: with four equal radii it is the
    /// same drawing, and one shape is one shape — a clip and a stroke that pick
    /// their own cannot end up disagreeing about which is in use. It is
    /// watchOS 9 and up and this shell's minimum is 11, so there is no
    /// `@available` to write.
    ///
    /// Rust sends the four clockwise from the top left, and only when they
    /// differ; with one radius for all four it sends `borderRadius` alone,
    /// which is what nearly every node has. Left maps to leading and right to
    /// trailing: the props are named left and right, UIKit's masked corners are
    /// left and right too, and mirroring them here under an RTL locale would
    /// make the watch the one host that rounds a different corner.
    var anCornerShape: UnevenRoundedRectangle {
        let all = borderRadius ?? 0
        let corners = borderRadii?.count == 4 ? borderRadii! : [all, all, all, all]
        return UnevenRoundedRectangle(
            topLeadingRadius: corners[0],
            bottomLeadingRadius: corners[3],
            bottomTrailingRadius: corners[2],
            topTrailingRadius: corners[1]
        )
    }

    /// Whether the node cuts its children off at its own edge.
    ///
    /// The same rule UIKit's host applies in `set_clip`: `clip || rounded`. The
    /// first half is the template's resolved `overflow`; the second is not a
    /// choice, because a rounded background cannot be drawn without cutting the
    /// corners off, so a radius clips whether or not anybody asked for it.
    ///
    /// Without the first half this shell clipped every node unconditionally,
    /// which made `overflow: visible` —the default— do nothing here and work on
    /// the other three hosts.
    var anClips: Bool {
        if clip == true { return true }
        let all = borderRadius ?? 0
        let corners = borderRadii?.count == 4 ? borderRadii! : [all, all, all, all]
        return corners.contains { $0 > 0 }
    }
}

/// The clip, applied only to the nodes that ask for one. See `AnNode.anClips`.
private struct AnClip: ViewModifier {
    let node: AnNode

    @ViewBuilder
    func body(content: Content) -> some View {
        if node.anClips {
            // `.clipShape` and not `.clipped()`: with four zero radii it is the
            // very same rectangle, and it is the only thing that cuts a child
            // at a rounded corner instead of squaring it off.
            content.clipShape(node.anCornerShape)
        } else {
            content
        }
    }
}

/// The line around a node.
///
/// It hangs off `AnNodeView.body` rather than off each `case` for the same
/// reason accessibility does: an outline belongs to every node, not to a kind.
/// A node with no `[borderWidth]` gets an empty overlay, which SwiftUI drops.
///
/// There is no per-side border here, and it is not an omission: it is the
/// contract. `borderWidth` is one number; `borderTopWidth` and its three
/// siblings are layout styles — they are resolved by taffy, they inset the
/// children and they never reach any host as something to draw. Four rectangles
/// butted together would be this shell inventing a drawing the other three
/// hosts do not make. The core warns once per name so that a template asking
/// for one is not left wondering, and `scripts/check-border.sh` fails if any of
/// the four hosts grows one on its own.
private struct AnBorder: ViewModifier {
    let node: AnNode

    func body(content: Content) -> some View {
        content.overlay(stroke)
    }

    @ViewBuilder
    private var stroke: some View {
        if let width = node.borderWidth, width > 0 {
            node.anCornerShape
                // `.strokeBorder` and not `.stroke`: `.stroke` centres the line
                // on the path, so half of it would fall outside the frame taffy
                // gave and paint over whatever sits next to it. UIKit's
                // `layer.borderWidth` draws inside the bounds as well, which is
                // what makes the two hosts agree.
                .strokeBorder(AnNodeView.color(node.borderColor) ?? .black, lineWidth: width)
                // The line fades with the node. On UIKit the border lives on the
                // content's own layer and `alpha` takes it along; here it is a
                // sibling overlay and has to be told.
                .opacity(node.opacity ?? 1)
        }
    }
}

extension View {
    /// Cuts the children off at the node's edge, when the node clips.
    func anClip(_ node: AnNode) -> some View {
        modifier(AnClip(node: node))
    }

    /// Black when the template gave a width and no `[borderColor]`, because
    /// that is `CALayer`'s default and the phone draws it black too. On the
    /// watch's black background that line is invisible, which looks like a bug
    /// and is not one: it is the same border iOS paints.
    func anBorder(_ node: AnNode) -> some View {
        modifier(AnBorder(node: node))
    }
}
