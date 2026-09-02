import SwiftUI

/// The six accessibility props of the contract, on SwiftUI.
///
/// This host is the only declarative one of Apple's four, and that changes
/// where they are applied. In UIKit and in AppKit there is a live view and a
/// setter to call when the prop arrives; here there is a view built whole out
/// of every snapshot, so accessibility is not "set": it is declared while
/// building it, which is why this is a `ViewModifier` hanging off
/// `AnNodeView.body` and not any equivalent of `set_prop`.
///
/// **The decision is not here.** Which role is which trait, and which part of
/// the contract has no shape in SwiftUI, is settled by Rust in
/// `snapshot::accessibility_traits_of`, which is also what says it through the
/// log. What arrives here is already translated: a list of trait names and
/// three strings. That way the table lives in one place and not in two, same
/// as with the icons.
///
/// The only thing this file decides is how each piece is written in SwiftUI,
/// and a trait name it does not recognise leaves through the log instead of
/// getting lost: that is the only way a change in Rust that was not followed
/// here shows up, rather than leaving a mute view.
struct AnAccessibility: ViewModifier {
    let node: AnNode

    func body(content: Content) -> some View {
        content
            // First, whether this is **one** element or a container. It goes
            // before the rest because it decides what the label sticks to:
            // with `.combine` the whole row is a single stop and the label is
            // the row's.
            .modifier(Grouped(accessible: node.accessible))
            .modifier(Phrase(text: node.accessibilityLabel, which: .label))
            .modifier(Phrase(text: node.accessibilityHint, which: .hint))
            .modifier(Phrase(text: node.accessibilityValue, which: .value))
            // An empty trait set does nothing, so this one can always be
            // attached: with no role it does not change the view.
            .accessibilityAddTraits(traits)
            // And hiding the branch, which is the opposite of grouping:
            // `false` in the contract means "this is decorative, do not read
            // me or mine".
            .accessibilityHidden(node.accessible == false)
    }

    /// The traits Rust asked for, already unioned.
    private var traits: AccessibilityTraits {
        var all = AccessibilityTraits()
        for name in node.accessibilityTraits ?? [] {
            guard let trait = Self.trait(name) else {
                AnAccessibility.warnOnce(
                    "trait:\(name)",
                    "angular-native: Rust sent the AccessibilityTraits \"\(name)\" and the watch "
                        + "shell does not know it; that trait was not applied"
                )
                continue
            }
            all.formUnion(trait)
        }
        return all
    }

    /// The `AccessibilityTraits` that goes by that name.
    ///
    /// It is the only table on this side, and it is deliberately a dumb
    /// translation: the names are SwiftUI's as they stand, so whoever reads
    /// `swiftui_trait()` in Rust is already reading the constant's name.
    private static func trait(_ name: String) -> AccessibilityTraits? {
        switch name {
        case "isButton": return .isButton
        case "isLink": return .isLink
        case "isHeader": return .isHeader
        case "isImage": return .isImage
        case "isStaticText": return .isStaticText
        case "isSearchField": return .isSearchField
        case "isSummaryElement": return .isSummaryElement
        case "isSelected": return .isSelected
        case "isToggle": return .isToggle
        default: return nil
        }
    }

    /// Turns the view into a single stop for the reader, or leaves it as the
    /// container it was.
    ///
    /// It sits in a `ViewModifier` of its own, and not in an `if` inside the
    /// one above, so that the type coming out of `AnAccessibility` does not
    /// depend on whether the template set `[accessible]`: with the branch
    /// boxed in here, `body` has one type and not two.
    private struct Grouped: ViewModifier {
        let accessible: Bool?

        @ViewBuilder
        func body(content: Content) -> some View {
            if accessible == true {
                // `.combine` and not `.ignore`: what is needed is for the row
                // —icon, title and subtitle— to be read in one go, not for it
                // to go mute.
                content.accessibilityElement(children: .combine)
            } else {
                content
            }
        }
    }

    /// One of the three strings, attached only if it came.
    ///
    /// All three share a modifier because they share a problem: SwiftUI has no
    /// value meaning "do not set one". `.accessibilityLabel("")` is not
    /// leaving the label alone, it is setting an empty one, and on top of a
    /// `Toggle` —which ships with the system's— that leaves the control
    /// nameless. Empty ones never arrive: Rust filters them out before they go
    /// into the snapshot.
    private struct Phrase: ViewModifier {
        enum Which {
            case label
            case hint
            case value
        }

        let text: String?
        let which: Which

        @ViewBuilder
        func body(content: Content) -> some View {
            if let text {
                switch which {
                case .label: content.accessibilityLabel(Text(verbatim: text))
                case .hint: content.accessibilityHint(Text(verbatim: text))
                case .value: content.accessibilityValue(Text(verbatim: text))
                }
            } else {
                content
            }
        }
    }

    /// What has already been said. Same as in Rust: one warning per frame at
    /// 30 Hz is a log nobody reads.
    ///
    /// `@MainActor` because SwiftUI builds views there, and because global
    /// mutable state with no owner does not compile under Swift 6.
    @MainActor private static var said = Set<String>()

    @MainActor
    private static func warnOnce(_ key: String, _ message: String) {
        guard !said.contains(key) else { return }
        said.insert(key)
        NSLog("%@", message)
    }
}

extension View {
    func anAccessibility(_ node: AnNode) -> some View {
        modifier(AnAccessibility(node: node))
    }
}
