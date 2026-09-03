import AppKit

/// The screenshot the app takes of itself.
///
/// It exists because on the desktop the proof that a host works is a picture of
/// the window, and `screencapture` is no good for automating that: asking for
/// the screen requires the recording permission, which is granted by hand and
/// per application. A view, on the other hand, knows how to draw itself into a
/// bitmap without asking anyone's permission: `cacheDisplay(in:to:)` walks the
/// hierarchy and paints the AppKit controls exactly as they are, which is
/// precisely what has to be checked.
///
/// It is turned on with `AN_SCREENSHOT=<path>` in the environment. Without that
/// variable none of this is in the frame path.
enum Screenshot {
    /// Frames let through from the first mount until the shutter fires.
    ///
    /// It is not decoration: when the core sends the first heights, the views
    /// are already in place but AppKit has not drawn them yet, and the system
    /// controls —the `NSSwitch`, the `NSProgressIndicator`— animate their
    /// initial state for a few tenths of a second. Firing on the mounting frame
    /// gives a picture of half-painted controls.
    private static var graceFrames: Int {
        Int(ProcessInfo.processInfo.environment["AN_SCREENSHOT_FRAMES"] ?? "") ?? 40
    }

    static var destination: String? {
        ProcessInfo.processInfo.environment["AN_SCREENSHOT"]
    }

    /// Where to put the pointer before firing, in points of the root view.
    ///
    /// Without this there is no way to photograph a `(hover)`: the highlight is
    /// produced by the system when the mouse really enters the tracking area,
    /// and in a check there is nobody moving the mouse.
    /// `CGWarpMouseCursorPosition` moves it without asking for any permission
    /// —it is not `CGEventPost`, which does require accessibility—, so what
    /// enters the area is the real pointer and what comes out in the picture is
    /// the real `NSTrackingArea` doing its job, not a state set by hand.
    static var hover: CGPoint? {
        guard let raw = ProcessInfo.processInfo.environment["AN_SCREENSHOT_HOVER"] else {
            return nil
        }
        let parts = raw.split(separator: ",").compactMap { Double($0) }
        guard parts.count == 2 else {
            NSLog("angular-native: AN_SCREENSHOT_HOVER is written x,y (and they are window points)")
            return nil
        }
        return CGPoint(x: parts[0], y: parts[1])
    }

    /// A button to press before firing, as `x,y`.
    ///
    /// There are states of an app that do not exist at startup and that are
    /// exactly the ones worth photographing: a video playing, for instance,
    /// starts when somebody hits play. This presses the system control under the
    /// point, through its own path —`performClick:`—, not by simulating the
    /// mouse.
    static var press: CGPoint? {
        guard let raw = ProcessInfo.processInfo.environment["AN_SCREENSHOT_PRESS"] else {
            return nil
        }
        let parts = raw.split(separator: ",").compactMap { Double($0) }
        guard parts.count == 2 else {
            NSLog("angular-native: AN_SCREENSHOT_PRESS is written x,y")
            return nil
        }
        return CGPoint(x: parts[0], y: parts[1])
    }

    /// A swipe, as `x,y,deltaX,deltaY`.
    ///
    /// It is the only thing about this host that cannot really be provoked
    /// without a trackpad and a hand on it: the gesture is recognised by the
    /// system, not by the app, and there is no way to ask it to recognise one.
    /// What can be done is to come in through the same door it comes in through
    /// —`swipeWithEvent:` on the view under the point— with the same deltas it
    /// sends, and see whether everything from there on works: the view takes it
    /// or passes it to its parent, the event crosses into the engine and the
    /// template is recomposed.
    ///
    /// What this does **not** check is that a two-finger gesture ends up in a
    /// `swipeWithEvent:`. That is the system's decision and has to be looked at
    /// by hand. What it does check is everything else, which is what can be
    /// broken by editing this repository.
    static var swipe: (point: CGPoint, dx: Double, dy: Double)? {
        guard let raw = ProcessInfo.processInfo.environment["AN_SCREENSHOT_SWIPE"] else {
            return nil
        }
        let parts = raw.split(separator: ",").compactMap { Double($0) }
        guard parts.count == 4 else {
            NSLog("angular-native: AN_SCREENSHOT_SWIPE is written x,y,deltaX,deltaY")
            return nil
        }
        return (CGPoint(x: parts[0], y: parts[1]), parts[2], parts[3])
    }

    /// Photograph the whole window, title bar included.
    ///
    /// The normal screenshot is of the content, which is what the core mounts.
    /// But on macOS there is one thing in the tree that is **not** in the
    /// content: the `[title]` of an `<an-navigation-bar>`, which ends up in the
    /// window's title bar. To see it the topmost view has to be drawn, which is
    /// the one AppKit uses for the window frame.
    static var withFrame: Bool {
        ProcessInfo.processInfo.environment["AN_SCREENSHOT_WINDOW"] == "1"
    }

    /// How many hot reloads to wait for before firing.
    ///
    /// It exists because of one specific bug that was already made once: a hot
    /// reload unmounted the views and left the window black. That shows up in no
    /// log —the app is still alive, the frames keep going by and nobody returns
    /// an error—; it can only be seen by looking at the screen. With
    /// `AN_SCREENSHOT_RELOADS=1` the screenshot is not the one from startup but
    /// the one after reloading, which is the one that shows whether the tree is
    /// still standing.
    static var reloadsToWaitFor: Int {
        Int(ProcessInfo.processInfo.environment["AN_SCREENSHOT_RELOADS"] ?? "") ?? 0
    }

    /// Writes the view out as a PNG. Returns `false` and says why if it cannot:
    /// a screenshot that is not saved and does not report it leaves a check
    /// green while looking at a file from the previous run.
    static func write(_ root: NSView, to path: String) -> Bool {
        // The window frame is the ancestor of the content view. There is no need
        // to ask anybody for the screen: it is still an `NSView` that knows how
        // to draw itself into a bitmap.
        let view = withFrame ? (root.window?.contentView?.superview ?? root) : root
        guard let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds) else {
            NSLog("angular-native: the view could not produce a bitmap")
            return false
        }
        view.cacheDisplay(in: view.bounds, to: rep)
        guard let png = rep.representation(using: .png, properties: [:]) else {
            NSLog("angular-native: the bitmap could not be encoded as PNG")
            return false
        }
        do {
            try png.write(to: URL(fileURLWithPath: path))
        } catch {
            NSLog("angular-native: could not write \(path): \(error)")
            return false
        }
        // The colour count goes on the same line because it is what turns the
        // screenshot into an automatic check: a PNG file of the right size and
        // completely black weighs its share and passes any test that looks at
        // the size. If this comes out as 1, the window painted nothing and the
        // script can say so without opening the image.
        NSLog(
            "angular-native: screenshot written to \(path) (\(rep.pixelsWide)x\(rep.pixelsHigh), \(colours(rep)) colours)"
        )
        return true
    }

    /// Takes the real pointer to the point `AN_SCREENSHOT_HOVER` asked for.
    ///
    /// There are two things to do and both are needed. One is moving the
    /// pointer, which is what makes it enter the `NSTrackingArea`. The other is
    /// bringing the app to the front: this host's areas are `ActiveInActiveApp`,
    /// because on a Mac controls only light up on hover when the app is active,
    /// and a check cannot ask the host to behave differently from the rest of
    /// the desktop. It is the only screenshot that steals the focus, and only
    /// whoever wants to photograph the pointer asks for it.
    ///
    /// What happens afterwards —AppKit noticing the new area and delivering the
    /// `mouseEntered:`— is done by the app's normal event loop, which keeps
    /// running. That is why this is called halfway through the wait and not just
    /// before firing.
    ///
    /// The app brings itself to the front at startup and not here: see
    /// `AppDelegate`.
    static func placePointer(in view: NSView) {
        guard let point = hover, let window = view.window else { return }
        // The app already activated at startup (see `AppDelegate`); this is in
        // case something took the front away in the meantime.
        NSApp.activate(ignoringOtherApps: true)
        // And say whether it worked, because it does not always. With a
        // simulator open, macOS gives the front to whoever asked last and the
        // pointer ends up over another window: there is then no `(hover)` to
        // receive, and without this line the check reports it as a failure of
        // the code. The two messages share no substring on purpose — one
        // containing the other is how the first version of this check fooled
        // itself.
        NSLog("angular-native: frontmost=%@", NSApp.isActive ? "yes" : "no")
        // Where the mouse of whoever launched the check was. It is given back as
        // soon as the picture is taken: moving somebody's pointer and leaving it
        // wherever suited you is the same as breaking whatever they were doing.
        pointerWasAt = NSEvent.mouseLocation

        let inWindow = view.convert(point, to: nil)
        let onScreen = window.convertPoint(toScreen: inWindow)
        // `CGWarpMouseCursorPosition` works in screen coordinates with the
        // origin at the top; AppKit gives them with the origin at the bottom.
        let height = NSScreen.screens.first?.frame.height ?? 0
        let target = CGPoint(x: onScreen.x, y: height - onScreen.y)
        // Out first, then in. A tracking area reports an entry when the
        // pointer *moves* into it, not because of where it happens to be, so
        // warping straight to a point the pointer is already on produces
        // nothing at all. On its own this check passed, because the pointer
        // came from wherever the person left it; inside the full run an
        // earlier step had already parked it inside the view, and the entry
        // that never happened looked exactly like a broken `(hover)`.
        CGWarpMouseCursorPosition(CGPoint(x: 0, y: 0))
        CGAssociateMouseAndMouseCursorPosition(1)
        CGWarpMouseCursorPosition(target)
        // A warp leaves an interval during which the system does not associate
        // the mouse with the pointer. Without closing it, the movement is not
        // delivered and the area never learns that somebody is on top of it.
        CGAssociateMouseAndMouseCursorPosition(1)
    }

    /// Where the pointer was before this moved it.
    private static var pointerWasAt: CGPoint?

    /// Gives the mouse back to whoever had it.
    static func restorePointer() {
        guard let point = pointerWasAt else { return }
        pointerWasAt = nil
        let height = NSScreen.screens.first?.frame.height ?? 0
        CGWarpMouseCursorPosition(CGPoint(x: point.x, y: height - point.y))
        CGAssociateMouseAndMouseCursorPosition(1)
    }

    /// Presses the system control under the point.
    ///
    /// It walks up the parents until it finds an `NSControl` because what sits
    /// right under the point is usually the cell or the label inside the button,
    /// not the button. An `<an-view>` with `(press)` does not come through here:
    /// that one goes through a gesture recogniser, and this only knows about
    /// controls.
    static func pressControl(in view: NSView) {
        guard let point = press else { return }
        var target = view.hitTest(view.convert(point, to: view.superview))
        while let current = target, !(current is NSControl) {
            target = current.superview
        }
        guard let control = target as? NSControl else {
            NSLog("angular-native: there is no system control at %@", "\(point)")
            return
        }
        control.performClick(nil)
    }

    /// Sends a swipe to the view under the point.
    ///
    /// The event is built from a scroll wheel `CGEvent` because that is the only
    /// way to get an `NSEvent` with the deltas set; what makes it a swipe is not
    /// the type, it is which method it is delivered to. And it is delivered to
    /// the result of `hitTest:`, which is what the system does: if that view
    /// does not handle it, it goes up to its parent along the responder chain.
    static func sendSwipe(in view: NSView) {
        guard let (point, dx, dy) = swipe else { return }
        guard let cg = CGEvent(source: nil) else {
            NSLog("angular-native: could not build the swipe event")
            return
        }
        cg.type = .scrollWheel
        cg.setDoubleValueField(.scrollWheelEventDeltaAxis1, value: dy)
        cg.setDoubleValueField(.scrollWheelEventDeltaAxis2, value: dx)
        guard let event = NSEvent(cgEvent: cg) else {
            NSLog("angular-native: the swipe event could not be converted")
            return
        }
        // `hitTest:` wants the point in the coordinates of the view's parent.
        let target = view.hitTest(view.convert(point, to: view.superview)) ?? view
        target.swipe(with: event)
    }

    /// How many distinct colours there are in the bitmap.
    ///
    /// One pixel in four is sampled by rows and by columns: that is sixteen
    /// times less work, and any system control takes up rather more than a
    /// pixel, so none of them escapes the sampling.
    private static func colours(_ rep: NSBitmapImageRep) -> Int {
        guard let base = rep.bitmapData else { return 0 }
        let perPixel = rep.bitsPerPixel / 8
        guard perPixel >= 3 else { return 0 }
        var seen = Set<UInt32>()
        for y in stride(from: 0, to: rep.pixelsHigh, by: 4) {
            let row = base + y * rep.bytesPerRow
            for x in stride(from: 0, to: rep.pixelsWide, by: 4) {
                let p = row + x * perPixel
                seen.insert(UInt32(p[0]) << 16 | UInt32(p[1]) << 8 | UInt32(p[2]))
            }
        }
        return seen.count
    }

    /// Keeps the frame count since the first mount.
    ///
    /// It is kept apart from the controller so that the normal frame path —the
    /// one that runs sixty times a second in any app— is a comparison against
    /// `nil` and nothing else.
    final class Trigger {
        /// Frames to wait for the core to mount something before giving up. Ten
        /// seconds at 60 Hz. Without this cap, an app that does not start leaves
        /// the window black and the script waiting forever, which is the most
        /// expensive way of failing silently.
        private static let framesToWait = 600

        private let path: String
        private var mounted = false
        private var frames = 0
        /// Reloads still to be seen. Until it reaches zero, the trigger watches
        /// the frames go by and counts none of them.
        private var reloadsPending: Int

        init(path: String) {
            self.path = path
            self.reloadsPending = Screenshot.reloadsToWaitFor
        }

        /// The shell has just stitched a new bundle over the one that was
        /// running.
        ///
        /// The count is rearmed from zero: what is worth capturing is what is on
        /// screen *after* the reload, and the views that survive a hot reload are
        /// not mounted again, so the wait from here on cannot demand that any
        /// height arrive.
        func reloaded() {
            guard reloadsPending > 0 else { return }
            reloadsPending -= 1
            frames = 0
            mounted = reloadsPending == 0
        }

        /// `applied` is what `an_runtime_frame` returned. When its turn comes, it
        /// ends the process: the exit code tells apart the three things that
        /// matter to whoever called it from a script —the app mounted and the
        /// screenshot was saved, the screenshot failed, or nothing was ever
        /// mounted—.
        func frame(applied: Int32, view: NSView) {
            // With reloads pending there is nothing to count: the screenshot is
            // fired by `reloaded()`, not by startup.
            guard reloadsPending == 0 else { return }
            frames += 1
            if !mounted {
                guard applied > 0 else {
                    if frames >= Self.framesToWait {
                        NSLog("angular-native: %d frames without mounting anything; there is nothing to capture", frames)
                        exit(2)
                    }
                    return
                }
                mounted = true
                frames = 0
            }
            // The pointer is placed halfway through the wait, not just before
            // firing. Between it entering the tracking area and the highlight
            // being on screen there are three steps —AppKit delivers the
            // `mouseEntered:`, the event crosses into the engine, Angular
            // recomposes—, and all three happen on their own by letting the
            // frames run. Forcing the event loop from here would be reentering
            // this method.
            if frames == max(1, Screenshot.graceFrames / 4) {
                Screenshot.pressControl(in: view)
            }
            if frames == max(1, Screenshot.graceFrames / 3) {
                Screenshot.sendSwipe(in: view)
            }
            if frames == max(1, Screenshot.graceFrames / 2) {
                Screenshot.placePointer(in: view)
            }
            guard frames >= Screenshot.graceFrames else { return }
            let written = Screenshot.write(view, to: path)
            Screenshot.restorePointer()
            exit(written ? 0 : 1)
        }
    }
}
