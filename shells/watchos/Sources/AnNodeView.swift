import SwiftUI

/// Paints a node and, recursively, its own.
///
/// The rule that governs this whole file: **the layout is already done**. taffy
/// computed every node's frame in Rust, relative to its parent. Here it is only
/// placed. That is why there is not one `VStack`, one `HStack` or one
/// `.padding` in the whole shell: if there were, there would be two layout
/// engines deciding the same thing, and the result would be neither one's.
///
/// The placement pattern is always the same:
///
///     .frame(width:height:)      the size taffy gave
///     .position(x:y:)            the centre, inside the parent's ZStack
///
/// `.position` is used and not `.offset` because `.offset` moves from where
/// SwiftUI would have put the view —the container's centre—, and that would mean
/// compensating for the centring on every node. `.position` is absolute with
/// respect to the container, which is exactly what taffy gives.
///
/// The controls are SwiftUI's, not drawings that resemble them: a `Toggle`, a
/// `Slider`, a `Picker`. They bring what the system brings —the haptics, the
/// highlight, the crown rotation— and that is not imitated.
struct AnNodeView: View {
    let node: AnNode
    /// The state the finger moves and the app has not confirmed yet.
    let controls: AnControls
    /// The way events go out towards Rust.
    let dispatch: (UInt32, String, [String: Any]) -> Void
    /// Who holds the crown right now. It is passed by hand down the whole tree
    /// because a `FocusState` does not travel through SwiftUI's environment.
    let crownFocus: FocusState<UInt32?>.Binding

    var body: some View {
        content
            .frame(width: node.width, height: node.height)
            // The outline goes here, on the frame taffy gave and before the
            // node is placed, for the same reason accessibility does: a border
            // belongs to every node and not to a kind. `AnBorder` is what says
            // why there is one width and not four.
            .anBorder(node)
            .position(x: node.x + node.width / 2, y: node.y + node.height / 2)
            // Accessibility goes here and not in every `case` of `content`
            // for the same reason the frame does: it belongs to every node,
            // not to a type. A `Toggle` already ships with the system's label
            // and trait, and what is here only goes on top of that if the
            // template really set it; `AnAccessibility` takes care of that.
            .anAccessibility(node)
    }

    @ViewBuilder
    private var content: some View {
        if node.unsupported != nil {
            // The gap of the size the layout gave. The why has already been
            // said by the host through the log, once and with the SDK's reason:
            // painting a labelled box here would mean inventing a control.
            Color.clear
        } else {
            switch node.kind {
            case "Text": textView
            case "Button": buttonView
            case "ScrollView": scrollView
            case "Image": imageView
            case "Icon": iconView
            case "Switch": toggleView
            case "Slider": sliderView
            case "Stepper": stepperView
            case "ProgressBar": progressView
            case "ActivityIndicator": spinnerView
            case "TextInput": fieldView
            case "Picker": pickerView
            case "DatePicker": dateView
            case "StackView": stackView
            default: container
            }
        }
    }

    // -------------------------------------------------------------- containers

    /// Container: background, corners and children placed by frame.
    ///
    /// The `ZStack` with `topLeading` is the parent's coordinate system. Without
    /// an explicit `alignment` SwiftUI would centre, and taffy's frames are from
    /// the top left corner.
    private var container: some View {
        children
            .frame(width: node.width, height: node.height, alignment: .topLeading)
            .background(background)
            .anClip(node)
            .opacity(node.opacity ?? 1)
            .anGestures(node, dispatch: dispatch)
            .anCrown(node, controls: controls, dispatch: dispatch, focus: crownFocus)
    }

    private var children: some View {
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
    }

    /// A stack of screens: only the top one is visible, and it comes in or goes
    /// out according to the direction the template gives.
    ///
    /// The core has already given the stack the parent's size and stacked its
    /// children one on top of another; the only thing the shell decides is which
    /// one shows and how it comes in.
    private var stackView: some View {
        ZStack(alignment: .topLeading) {
            if let top = node.children?.last {
                AnNodeView(
                    node: top,
                    controls: controls,
                    dispatch: dispatch,
                    crownFocus: crownFocus
                )
                .transition(slide)
            }
        }
        .frame(width: node.width, height: node.height, alignment: .topLeading)
        .background(background)
        // A stack that does not clip lets the screen coming in paint outside
        // it while it slides, which is what UIKit's host does too: there the
        // animation moves the child's frame and only `clipsToBounds` stops it.
        // A stack meant to contain its transition asks for `overflow: hidden`.
        .anClip(node)
        .animation(.easeOut(duration: 0.25), value: node.children?.last?.id)
    }

    /// Which way a screen comes in and goes out.
    ///
    /// The direction is given by `[transition]`, set by whoever navigates: it is
    /// the only one that knows whether this is going forwards or back. With
    /// `none` nothing is animated, which is what is needed when mounting the
    /// first screen.
    private var slide: AnyTransition {
        switch node.transition {
        case "pop": .asymmetric(insertion: .move(edge: .leading), removal: .move(edge: .trailing))
        case "none": .identity
        default: .asymmetric(insertion: .move(edge: .trailing), removal: .move(edge: .leading))
        }
    }

    /// The `ScrollView` really is SwiftUI's: scrolling with the digital crown
    /// cannot be imitated, and it is the only way for it to feel like the rest of
    /// the watch.
    ///
    /// Inside, taffy is still in charge: the content is a `ZStack` of the size
    /// `contentSize` gave, with the children in their frames.
    private var scrollView: some View {
        // One axis, never both: the core clamps the content to this view's own
        // size across the axis it was not asked for, so a second one would have
        // nothing to travel over and would only take the crown away from the
        // one that does.
        ScrollView(node.horizontal == true ? .horizontal : .vertical) {
            children
                .frame(
                    width: node.contentWidth ?? node.width,
                    height: node.contentHeight ?? node.height,
                    alignment: .topLeading
                )
        }
        .frame(width: node.width, height: node.height)
        .background(background)
        .anClip(node)
    }

    // ------------------------------------------------------------------- text

    /// Text already measured by UIKit in Rust: the frame it comes with is
    /// exactly the room it takes. `fixedSize` stops SwiftUI deciding to truncate
    /// it on its own after having been given that frame.
    ///
    /// Both alignments are needed and they are not the same thing:
    /// `multilineTextAlignment` arranges the lines *inside* the block of text,
    /// and the `frame`'s places that block inside the frame taffy gave. With only
    /// the first, a centred text in a box wider than itself ends up flush left
    /// with its lines centred against each other.
    private var textView: some View {
        Text(node.text ?? "")
            .font(font)
            .tracking(node.letterSpacing ?? 0)
            .underline(node.textDecoration == "underline")
            .strikethrough(node.textDecoration == "lineThrough")
            .foregroundStyle(foreground)
            .multilineTextAlignment(alignment)
            .lineLimit(node.maxLines)
            .frame(width: node.width, height: node.height, alignment: blockAlignment)
            .fixedSize(horizontal: false, vertical: true)
            .opacity(node.opacity ?? 1)
            .anGestures(node, dispatch: dispatch)
    }

    /// The system button. SwiftUI's `Button` is used and not a `View` with a
    /// gesture so it brings what it brings on watchOS: the highlight on touch and
    /// the haptic feedback. The `buttonStyle(.plain)` removes the capsule
    /// background it would put there by default, because the background is the
    /// app's decision.
    private var buttonView: some View {
        Button {
            dispatch(node.id, "press", [:])
        } label: {
            Text(node.text ?? "")
                .font(font)
                .foregroundStyle(foreground)
                .frame(width: node.width, height: node.height)
        }
        .buttonStyle(.plain)
        .disabled(node.disabled == true)
        .background(background)
        .anClip(node)
        .opacity(node.opacity ?? 1)
        .anGestures(node, dispatch: dispatch, ownsTheTap: true)
    }

    // ----------------------------------------------------------------- images

    /// An image from the bundle or from the network.
    ///
    /// The natural size is returned through `(load)` as soon as it is known: the
    /// layout cannot place something whose size it does not know, and only the
    /// image knows it. It is the same contract as on iOS, where the directive
    /// registers that listener even when the template does not listen for it.
    private var imageView: some View {
        AnImageView(node: node, dispatch: dispatch)
            .frame(width: node.width, height: node.height)
            .anClip(node)
            .opacity(node.opacity ?? 1)
            .anGestures(node, dispatch: dispatch)
    }

    /// An SF Symbol, asked for by name. No icon set is bundled: it is drawn by
    /// the system, with whatever stroke that version of watchOS gives it.
    private var iconView: some View {
        Image(systemName: node.symbol ?? "questionmark")
            .font(.system(size: node.symbolSize ?? 24, weight: swiftWeight(node.symbolWeight)))
            .foregroundStyle(foreground)
            .frame(width: node.width, height: node.height)
            .opacity(node.opacity ?? 1)
            .anGestures(node, dispatch: dispatch)
    }

    // --------------------------------------------------------------- controls

    /// A `Toggle` with the switch style, which is the system's. With no label:
    /// the label is put alongside by the template, because what decides the
    /// layout is taffy.
    private var toggleView: some View {
        Toggle("", isOn: controls.flag(node))
            .labelsHidden()
            .tint(ownColor ?? .green)
            .disabled(node.disabled == true)
            .frame(width: node.width, height: node.height, alignment: .trailing)
            .opacity(node.opacity ?? 1)
    }

    private var sliderView: some View {
        Slider(
            value: controls.number(node),
            in: (node.minimum ?? 0)...max(node.maximum ?? 1, (node.minimum ?? 0) + 0.000_001)
        )
        .tint(ownColor)
        .disabled(node.disabled == true)
        .frame(width: node.width, height: node.height)
        .opacity(node.opacity ?? 1)
    }

    /// The system `Stepper`: the two buttons with the plus and the minus, and the
    /// crown rotation when it has the focus, which on the watch is how it is
    /// really used.
    private var stepperView: some View {
        Stepper(
            value: controls.number(node),
            in: (node.minimum ?? 0)...max(node.maximum ?? 100, node.minimum ?? 0),
            step: node.step ?? 1
        ) {
            EmptyView()
        }
        .disabled(node.disabled == true)
        .frame(width: node.width, height: node.height)
        .opacity(node.opacity ?? 1)
    }

    private var progressView: some View {
        ProgressView(value: node.progress ?? 0)
            .tint(ownColor)
            .frame(width: node.width, height: node.height)
            .opacity(node.opacity ?? 1)
    }

    /// The spinner. `animating` set to `false` hides it, just like
    /// `hidesWhenStopped` on iOS: a stopped indicator says nothing.
    @ViewBuilder
    private var spinnerView: some View {
        if node.animating == false {
            Color.clear
        } else {
            ProgressView()
                .tint(ownColor)
                .frame(width: node.width, height: node.height)
                .opacity(node.opacity ?? 1)
        }
    }

    /// The minimum height of a field on the watch.
    ///
    /// It is not a preference: the watchOS `TextField` is always drawn inside its
    /// own rounded container —it is not a field with a cursor, it is a button
    /// that opens the dictation screen— and that container measures this even
    /// when given a shorter frame. `.textFieldStyle(.plain)` does not remove it:
    /// that was tried, and on watchOS 26 it changes nothing.
    private static let fieldHeight: Double = 40

    /// Text field.
    ///
    /// On the watch a `TextField` is not typed into where it sits: on being
    /// tapped the system opens its own screen —dictation, scribble or keyboard—
    /// and hands the text back. That is exactly what this control does, and that
    /// is why it is the system's and not a box with a drawn cursor.
    ///
    /// And that is why its frame cannot be decided by the text. taffy measures an
    /// `an-text-input` the way it measures a `<Text>` —one line— because on iOS a
    /// borderless `UITextField` takes exactly that; here the system container is
    /// taller and would eat the row below. It has to be given `[style.height]`,
    /// and if it is not, it is said.
    private var fieldView: some View {
        field
            .font(font)
            .foregroundStyle(foreground)
            .multilineTextAlignment(alignment)
            .disabled(node.disabled == true)
            .frame(width: node.width, height: node.height, alignment: blockAlignment)
            .opacity(node.opacity ?? 1)
            .onSubmit { dispatch(node.id, "submit", ["value": controls.text(node).wrappedValue]) }
            .onAppear {
                guard node.height < Self.fieldHeight else { return }
                AnWarnings.once("short-field-\(node.id)") {
                    NSLog(
                        """
                        angular-native: <an-text-input> is \(Int(node.height)) points tall and \
                        the watch field draws \(Int(Self.fieldHeight)); give it [style.height] or \
                        it will overlap what is below it
                        """
                    )
                }
            }
    }

    @ViewBuilder
    private var field: some View {
        if node.secure == true {
            SecureField(node.placeholder ?? "", text: controls.text(node))
        } else {
            TextField(node.placeholder ?? "", text: controls.text(node))
        }
    }

    /// Choosing an option. On the watch the `Picker` is the wheel that turns
    /// with the crown; there is no dropdown, and building one would mean
    /// imitating a control the system does not have.
    private var pickerView: some View {
        Picker("", selection: controls.index(node)) {
            ForEach(Array((node.items ?? []).enumerated()), id: \.offset) { index, label in
                Text(label).tag(index)
            }
        }
        .labelsHidden()
        .disabled(node.disabled == true)
        .frame(width: node.width, height: node.height)
        .opacity(node.opacity ?? 1)
    }

    /// Date and time. On being tapped, the watch opens its own picker —the dial
    /// one— and hands the result back.
    private var dateView: some View {
        DatePicker("", selection: controls.date(node), displayedComponents: components)
            .labelsHidden()
            .disabled(node.disabled == true)
            .frame(width: node.width, height: node.height)
            .opacity(node.opacity ?? 1)
    }

    private var components: DatePickerComponents {
        switch node.dateMode {
        case "time": [.hourAndMinute]
        case "dateAndTime": [.date, .hourAndMinute]
        default: [.date]
        }
    }

    // ------------------------------------------------------------------ style

    private var background: Color {
        AnNodeView.color(node.background) ?? .clear
    }

    /// With no explicit `color`, white wins: on the watch the system background
    /// is black and inheriting SwiftUI's primary colour would come out the same,
    /// but said explicitly it does not depend on what SwiftUI does tomorrow.
    private var foreground: Color {
        AnNodeView.color(node.color) ?? .white
    }

    /// The template's colour, with no fallback: a control with no `[color]`
    /// keeps its own, which is the system's.
    private var ownColor: Color? {
        AnNodeView.color(node.color)
    }

    /// The font of a text or of a button's label.
    ///
    /// With `[fontFamily]` that one is asked for by name; without it, the
    /// system's. It is the same decision `WatchMeasurer` took when measuring, and
    /// it has to be: if a different one were painted here, the frame would not
    /// fit it.
    private var font: Font {
        let points = node.fontSize ?? 16
        var base: Font
        if let family = node.fontFamily, !family.isEmpty {
            base = .custom(family, size: points)
        } else {
            base = .system(size: points, weight: swiftWeight(node.fontWeight))
        }
        if node.italic == true {
            base = base.italic()
        }
        return base
    }

    private var alignment: TextAlignment {
        switch node.textAlign {
        case "center": .center
        case "right": .trailing
        default: .leading
        }
    }

    /// Where the block of text goes inside its frame.
    ///
    /// Vertically always centred, which is what a `UILabel` does and therefore
    /// what is seen on iOS: when the frame is taller than the text —a
    /// fixed-height row, a button— pinning it to the top shows and is not what
    /// anybody expects.
    private var blockAlignment: Alignment {
        switch node.textAlign {
        case "center": .center
        case "right": .trailing
        default: .leading
        }
    }

    /// The CSS 100..900 scale to SwiftUI's weights. It is the same table Rust
    /// measured the text with: were they to diverge, the frame would not fit the
    /// font that ends up being drawn.
    private func swiftWeight(_ weight: Int?) -> Font.Weight {
        switch weight ?? 400 {
        case ..<200: .ultraLight
        case ..<300: .thin
        case ..<400: .light
        case ..<500: .regular
        case ..<600: .medium
        case ..<700: .semibold
        case ..<800: .bold
        case ..<900: .heavy
        default: .black
        }
    }

    /// The channels arrive already resolved to 0..1 from Rust: no `#rrggbb` is
    /// parsed here, so there are not two colour parsers to keep in agreement.
    static func color(_ channels: [Double]?) -> Color? {
        guard let c = channels, c.count == 4 else { return nil }
        return Color(.sRGB, red: c[0], green: c[1], blue: c[2], opacity: c[3])
    }
}
