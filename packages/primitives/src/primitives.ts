import { DestroyRef, Directive, effect, ElementRef, inject, input, Renderer2 } from '@angular/core'
import { outputFromObservable } from '@angular/core/rxjs-interop'
import { map, Observable } from 'rxjs'

/**
 * The payload of a `(press)`. The coordinates are in points and relative to the
 * view that took the tap.
 *
 */
export interface NativePressEvent {
  x: number
  y: number
}

/**
 * Where in the gesture the event arrives.
 *
 * `cancel` is not a failure: the system takes the gesture away when another one
 * wins —dragging inside a list that starts scrolling, for instance—. Whoever is
 * moving something with a finger has to put it back where it was, not leave it
 * halfway.
 */
export type NativeGestureState = 'begin' | 'move' | 'end' | 'cancel'

/**
 * A drag.
 *
 * `translation` is measured from where the finger started, not from the previous
 * event: that way adding it to the starting position is enough, with nothing to
 * accumulate and no rounding error dragged along from every step.
 */
export interface NativePanEvent {
  x: number
  y: number
  translationX: number
  translationY: number
  /** Points per second. Useful for coasting on after the finger lifts. */
  velocityX: number
  velocityY: number
  state: NativeGestureState
}

/** A pinch. `scale` is relative to the start of the gesture, not absolute. */
export interface NativePinchEvent {
  scale: number
  velocity: number
  state: NativeGestureState
}

/** A two-finger rotation, in radians since the gesture began. */
export interface NativeRotateEvent {
  rotation: number
  velocity: number
  state: NativeGestureState
}

/** An event carrying nothing but the chosen position. */
export interface NativeIndexEvent {
  index: number
}

/** An event carrying nothing but a number. */
export interface NativeValueEvent {
  value: number
}

/** An event carrying nothing but text. */
export interface NativeTextEvent {
  value: string
}

/**
 * A turn of the watch's digital crown.
 *
 * `delta` is how much it has turned since the last notification, which is what
 * is nearly always wanted: SwiftUI only hands over the running total, and
 * subtracting in every template would mean repeating the same sum in all of
 * them.
 */
export interface NativeCrownEvent {
  delta: number
  /** The running total since the view took focus. */
  offset: number
  /** Turns per second. Useful for coasting on after the finger lifts. */
  velocity: number
}

/** A view's resolved frame, relative to its parent and in points. */
export interface NativeLayoutEvent {
  x: number
  y: number
  width: number
  height: number
}

/** The margins the system keeps for itself: notch, status bar, home bar. */
export interface NativeSafeAreaInsets {
  top: number
  right: number
  bottom: number
  left: number
}

/** A `ScrollView`'s current scroll offset, in points. */
export interface NativeScrollEvent {
  x: number
  y: number
}

/**
 * The pointer enters or leaves a view.
 *
 * It is one output with a boolean and not two —`hoverIn` and `hoverOut`—
 * because what sits underneath is one thing too: an `NSTrackingArea` delivers
 * entry and exit down the same road, and splitting it into two outputs would
 * force two subscriptions for something that nearly always ends up in the same
 * signal.
 *
 * The coordinates are the pointer's inside the view, in points. On the way out
 * they are those of the last point it passed through, which is the edge it left
 * by.
 */
export interface NativeHoverEvent {
  hovered: boolean
  x: number
  y: number
}

/**
 * Which pointer the system shows over this view.
 *
 * The names are CSS's and not AppKit's because they are the ones anybody who
 * has written an interface already knows, and because the vocabulary has to be
 * able to mean the same thing on another desktop. Each one is a system cursor
 * —an `NSCursor`— and not a drawing of ours: `pointer` is macOS's hand, looking
 * however it looks on that version.
 *
 * `null` means "whichever is right", which is not the same as `default`: with
 * nothing set, a text field goes on showing the text cursor it puts up by
 * itself, and `default` is asking for the arrow *on top of* whatever the control
 * would do.
 *
 * The resize ones are missing. The ones macOS has always had
 * —`resizeLeftRightCursor` and friends— are marked deprecated, and the ones that
 * replace them arrived in macOS 15, which is later than the minimum this host
 * compiles for. Adding them would mean choosing between a deprecation warning on
 * every build and a method that does not exist in the version we claim to
 * support.
 */
export type NativeCursor =
  | 'default'
  | 'pointer'
  | 'text'
  | 'crosshair'
  | 'grab'
  | 'grabbing'
  | 'not-allowed'

/**
 * The keys a control accepts in `[ios]` or in `[android]`.
 *
 * The list is declared even though the object's type already says it, and that
 * is not redundant: the compiler checks the type against what it can see, and it
 * cannot see an object assembled in pieces or one that comes from outside. The
 * list is what survives at runtime, and it is also what `check-wrapper.sh` reads
 * in order to demand that that platform's host —and only that one— looks at it.
 */
interface PlatformKeys {
  readonly primitive: string
  readonly platform: 'ios' | 'android'
  readonly keys: ReadonlySet<string>
}

function platformKeys(
  primitive: string,
  platform: 'ios' | 'android',
  keys: readonly string[]
): PlatformKeys {
  return { primitive, platform, keys: new Set(keys) }
}

/**
 * Warns once per key that nobody is going to look at.
 *
 * Same treatment as `warnUnknownStyle()` in the renderer and for the same
 * reason: a prop that travels, that nobody recognises and that raises no error
 * is a bug that looks like "this does nothing" and gets hunted in the wrong
 * place. Once per key, because the object is evaluated again on every change
 * detection pass.
 */
const warnedPlatformProps = new Set<string>()

function warnUnknownPlatformProp(where: PlatformKeys, key: string): void {
  const seen = `${where.primitive}.${where.platform}.${key}`
  if (warnedPlatformProps.has(seen)) return
  warnedPlatformProps.add(seen)
  console.warn(
    `[angular-native] <${where.primitive}> has no "${key}" in [${where.platform}], ` +
      `so it will do nothing. It accepts: ${[...where.keys].sort().join(', ')}. ` +
      'If it exists on both platforms it is an ordinary input and does not belong here.'
  )
}

/**
 * Native primitives as directives.
 *
 * The alternative was `CUSTOM_ELEMENTS_SCHEMA`, which besides demanding a hyphen
 * in the name switches off property checking: a mistyped `[bakcgroundColor]`
 * would sail past the compiler and fail silently on the device.
 *
 * With directives, every prop is a declared input: the template compiler checks
 * it, the editor completes it, and the directive is the natural place to convert
 * the value before sending it to the core.
 */
@Directive()
export abstract class NativeVisual {
  protected readonly node = inject(ElementRef).nativeElement
  protected readonly renderer = inject(Renderer2)

  protected set(name: string, value: unknown): void {
    this.renderer.setProperty(this.node, name, value ?? null)
  }

  /**
   * Pushes this directive's inputs to the core.
   *
   * A signal input has no moment at which it "gets assigned": it is read, and
   * whoever reads it decides when. Here an effect reads it.
   *
   * One per directive and not one per input. An `<an-text>` declares eleven
   * props and hardly any template uses more than three: with an effect per prop,
   * every `<an-text>` in a list of five thousand rows would be carrying eleven
   * reactive nodes nobody is ever going to wake. Reading eleven signals when one
   * of them changes is cheaper than keeping eleven effects waiting.
   *
   * Only what changed travels. And on the first pass the nulls keep quiet too,
   * since that is what an input nobody has set is worth: sending them would be
   * asking the host to erase something it never wrote.
   */
  protected push(props: Record<string, () => unknown>): void {
    const entries = Object.entries(props)
    const sent = new Map<string, unknown>()
    let first = true
    effect(() => {
      for (const [name, read] of entries) {
        const value = read()
        if (sent.has(name) && Object.is(sent.get(name), value)) continue
        sent.set(name, value)
        if (first && (value === null || value === undefined)) continue
        this.set(name, value)
      }
      first = false
    })
  }

  /** The same for a platform's object, which is sent taken apart. */
  protected pushPlatform(
    where: PlatformKeys,
    value: () => Record<string, unknown> | null
  ): void {
    effect(() => this.platform(where, value()))
  }

  constructor() {
    this.push({
      backgroundColor: this.backgroundColor,
      animate: this.animate,
      animateDelay: this.animateDelay,
      animateEasing: this.animateEasing,
      translateX: this.translateX,
      translateY: this.translateY,
      scale: this.scale,
      scaleX: this.scaleX,
      scaleY: this.scaleY,
      rotate: this.rotate,
      borderRadius: this.borderRadius,
      borderTopLeftRadius: this.borderTopLeftRadius,
      borderTopRightRadius: this.borderTopRightRadius,
      borderBottomRightRadius: this.borderBottomRightRadius,
      borderBottomLeftRadius: this.borderBottomLeftRadius,
      borderWidth: this.borderWidth,
      borderColor: this.borderColor,
      opacity: this.opacity,
      cursor: this.cursor,
      accessibilityLabel: this.accessibilityLabel,
      accessibilityHint: this.accessibilityHint,
      accessibilityRole: this.accessibilityRole,
      accessibilityValue: this.accessibilityValue,
      // The state is an object and the protocol carries no objects, so it
      // travels as JSON, just like the lists of `an-tab-bar`. `null` stays
      // `null` on purpose: sending `"{}"` would say "no state", which is not the
      // same thing as "nothing has been said about the state".
      accessibilityState: () => {
        const state = this.accessibilityState()
        return state === null ? null : JSON.stringify(state)
      },
      accessible: this.accessible,
      testID: this.testID
    })
  }

  /** What was sent last time in each platform object. */
  private readonly platformSent = new Map<string, Set<string>>()

  /**
   * Takes `[ios]` or `[android]` apart into loose props with their prefix.
   *
   * The prefix does two things: it lets the other platform's host discard the
   * prop without knowing what it is, and it keeps the name greppable
   * —`"ios:subtitle"` has to appear in the iOS host and not in the Android one,
   * and a script checks that—.
   */
  protected platform(where: PlatformKeys, value: Record<string, unknown> | null): void {
    const previous = this.platformSent.get(where.platform)
    const sent = new Set<string>()
    for (const [key, raw] of Object.entries(value ?? {})) {
      if (!where.keys.has(key)) {
        warnUnknownPlatformProp(where, key)
        continue
      }
      sent.add(key)
      // The wire's value type is a number, a boolean or a string, so a list
      // travels as JSON and the host takes it apart — the same treatment
      // `[items]` and `[buttons]` get, and for the same reason. Doing it here
      // and not in the directive keeps every platform key going through one
      // path: `set()` would otherwise turn the array into whatever
      // `String(value)` felt like, which is a comma-joined string that happens
      // to work until a value contains a comma.
      this.set(`${where.platform}:${key}`, Array.isArray(raw) ? JSON.stringify(raw) : raw)
    }
    // A key that was set and is no longer there has to go back to its factory
    // value: the control does not work out on its own that it has been taken
    // away.
    if (previous) {
      for (const key of previous) {
        if (!sent.has(key)) this.set(`${where.platform}:${key}`, null)
      }
    }
    this.platformSent.set(where.platform, sent)
  }

  /**
   * A gesture that only exists if the template asks for it.
   *
   * The observable is cold: the `UIGestureRecognizer` is hooked up on subscribe
   * and let go when the view is destroyed. Angular subscribes an output only
   * when there is a `(press)` bound, so a view nobody is listening to pays
   * nothing. Declaring it as an element event would have cost the same, but
   * `$event` would be an `Event` and every template would have to cast.
   */
  protected nativeEvent<T>(name: string): Observable<T> {
    return new Observable<T>((subscriber) => {
      const unlisten = this.renderer.listen(this.node, name, (payload) => {
        subscriber.next(payload as T)
      })
      return () => unlisten()
    })
  }

  readonly press = outputFromObservable(this.nativeEvent<NativePressEvent>('press'))
  readonly doublePress = outputFromObservable(this.nativeEvent<NativePressEvent>('doublePress'))

  /**
   * Press and hold. It arrives once and once only, when the system decides the
   * gesture counts: each platform has its own time threshold, and respecting it
   * is what makes the app feel like it belongs to that platform.
   */
  readonly longPress = outputFromObservable(this.nativeEvent<NativePressEvent>('longPress'))

  readonly pan = outputFromObservable(this.nativeEvent<NativePanEvent>('pan'))
  readonly pinch = outputFromObservable(this.nativeEvent<NativePinchEvent>('pinch'))
  /**
   * Two-finger rotation.
   *
   * It is called `rotation` and not `rotate` because `[rotate]` is already the
   * transform, and a class cannot have two members with the same name. It also
   * makes it clearer which is which: `[rotate]` commands, `(rotation)` reports.
   */
  readonly rotation = outputFromObservable(this.nativeEvent<NativeRotateEvent>('rotate'))

  // Swiping. Each direction is its own output because each one hooks up its own
  // recogniser: listening to `swipeLeft` alone does not cost the other three.
  readonly swipeLeft = outputFromObservable(this.nativeEvent<NativePressEvent>('swipeLeft'))
  readonly swipeRight = outputFromObservable(this.nativeEvent<NativePressEvent>('swipeRight'))
  readonly swipeUp = outputFromObservable(this.nativeEvent<NativePressEvent>('swipeUp'))
  readonly swipeDown = outputFromObservable(this.nativeEvent<NativePressEvent>('swipeDown'))

  /**
   * The pointer enters or leaves this view.
   *
   * On a desktop this is not decoration: a control that does not change as the
   * mouse passes over it looks disabled, and that is the only hint somebody with
   * a mouse has that there is something there to click. On a phone it does not
   * exist —there is no pointer to respond to— which is why that platform's host
   * never delivers it.
   */
  readonly hover = outputFromObservable(this.nativeEvent<NativeHoverEvent>('hover'))

  /**
   * The frame layout assigned it, every time it changes.
   *
   * No platform produces it: the core emits it as the commit finishes, because
   * the core is what computes the frame. It comes free on both iOS and Android.
   */
  readonly layout = outputFromObservable(this.nativeEvent<NativeLayoutEvent>('layout'))

  /**
   * The margins the system keeps for itself, and every time they change: on
   * rotation, when the keyboard comes up, when going into split screen.
   */
  readonly safeArea = outputFromObservable(this.nativeEvent<NativeSafeAreaInsets>('safeArea'))

  /**
   * Focus came into or left this view.
   *
   * These used to be on `an-text-input` only, because on a phone focus belongs
   * to the keyboard. On a TV it is the whole platform: the remote walks the
   * focusable views and there is no other way to highlight the one under the
   * cursor. `value` only comes along when the view is a text field.
   */
  readonly focus = outputFromObservable(this.nativeEvent<{ value?: string }>('focus'))
  readonly blur = outputFromObservable(this.nativeEvent<{ value?: string }>('blur'))

  /**
   * The watch's digital crown, while it turns.
   *
   * It lives on the base class and not on a control because the crown goes to
   * **whichever view has focus**, whatever it is: on the watch it is the
   * equivalent of rolling the mouse wheel over something.
   */
  readonly crown = outputFromObservable(this.nativeEvent<NativeCrownEvent>('crown'))

  /** The crown stopped turning. Without it there is no way to know when to
   * stop. */
  readonly crownIdle = outputFromObservable(this.nativeEvent<void>('crownIdle'))

  readonly backgroundColor = input<string | null>(null)

  /**
   * How many milliseconds this view takes to reach its new values.
   *
   * With this set, moving, scaling, changing the opacity or relocating the view
   * stops being a jump: the platform animates it, on its drawing thread, without
   * coming back through JavaScript on every frame. That is why an animation
   * stays smooth even when the engine thread is busy.
   *
   * What gets animated is the change, not one particular value: it is set once
   * and holds for every change that comes after. Zero or `null` turns it off.
   */
  readonly animate = input<number | null>(null)

  readonly animateDelay = input<number | null>(null)

  /** `ease-out` by default: it leaves fast and brakes on arrival. */
  readonly animateEasing = input<'linear' | 'ease-in' | 'ease-out' | 'ease-in-out' | null>(null)

  /**
   * Translating, scaling and rotating.
   *
   * They deliberately take no part in layout: a view that has been moved or
   * scaled still occupies the same place it occupied. That is why they are cheap
   * —there is nothing to recompute— and that is why they are the ones to use to
   * follow a finger. To move something *and* have its neighbour get out of the
   * way, the layout has to change, not this.
   */
  readonly translateX = input<number | null>(null)

  readonly translateY = input<number | null>(null)

  readonly scale = input<number | null>(null)

  readonly scaleX = input<number | null>(null)

  readonly scaleY = input<number | null>(null)

  /** In radians, like what the rotate gesture sends. */
  readonly rotate = input<number | null>(null)

  readonly borderRadius = input<number | null>(null)

  // Per-corner radii. UIKit only knows about a single radius, so when they
  // differ the host draws the outline and uses it as a mask; Android settles it
  // with `setCornerRadii`.
  readonly borderTopLeftRadius = input<number | null>(null)

  readonly borderTopRightRadius = input<number | null>(null)

  readonly borderBottomRightRadius = input<number | null>(null)

  readonly borderBottomLeftRadius = input<number | null>(null)

  readonly borderWidth = input<number | null>(null)

  readonly borderColor = input<string | null>(null)

  readonly opacity = input<number | null>(null)

  /**
   * The system pointer over this view.
   *
   * It goes with `(hover)` and for the same reason: where there is a mouse, the
   * cursor's shape is half the response. Where there is not, it means nothing,
   * so the only host that looks at it is the desktop one; the other two have no
   * pointer to give a shape to. It is in the pointer prop list in
   * `scripts/check-wrapper.sh`, which is what demands that macOS does look at
   * it.
   */
  readonly cursor = input<NativeCursor | null>(null)

  /**
   * The name a screen reader announces.
   *
   * Without this, VoiceOver and TalkBack read whatever they find inside —a
   * child's text, an image's filename— or they read nothing at all. An `an-view`
   * acting as a button is, with no label, an element with no name: you can focus
   * it and you cannot tell what it does.
   *
   * System controls come with theirs from the factory, and it only needs setting
   * when the factory one does not say the right thing.
   */
  readonly accessibilityLabel = input<string | null>(null)

  /** What happens on activating it, if the name is not enough. It is read after
   * the name. */
  readonly accessibilityHint = input<string | null>(null)

  /** What it is. See `NativeRole`. */
  readonly accessibilityRole = input<NativeRole | null>(null)

  /** What it is worth right now: "35 %", "three of seven". */
  readonly accessibilityValue = input<string | null>(null)

  /** What state it is in. See `NativeAccessibilityState`. */
  readonly accessibilityState = input<NativeAccessibilityState | null>(null)

  /**
   * Whether this is **one** element as far as the reader is concerned, rather
   * than a container you navigate into.
   *
   * It is what turns a whole row —icon, title and subtitle— into a single stop
   * read out in one go, instead of three separate stops. `false` does the
   * opposite: it hides the view and whatever is inside it, which is what purely
   * decorative things need.
   */
  readonly accessible = input<boolean | null>(null)

  /** An identifier for UI tests; it ends up as accessibilityIdentifier. */
  readonly testID = input<string | null>(null)
}

@Directive({ selector: 'an-view' })
export class View extends NativeVisual {}

/**
 * What this is, for somebody who cannot see it.
 *
 * The vocabulary is deliberately short. Each platform has its own —traits in
 * UIKit, `NSAccessibility` roles in AppKit, `AccessibilityNodeInfo` on Android—
 * and they resemble each other neither in number nor in name. What they do share
 * is this handful, which is also what a screen reader announces differently:
 * "button", "heading", "link", "image", "checkbox", "picker".
 *
 * A role that only one platform had would go in its `[ios]` or `[android]`
 * object, like anything else that exists in one place only.
 */
export type NativeRole =
  | 'button'
  | 'link'
  | 'header'
  | 'image'
  | 'text'
  | 'checkbox'
  | 'radio'
  | 'switch'
  | 'slider'
  | 'search'
  | 'summary'
  | 'none'

/**
 * What state it is in, for somebody who cannot see it.
 *
 * It is kept apart from the role because it changes over time and the role does
 * not: a screen reader announces "selected" again when this changes, without the
 * view being rebuilt.
 */
export interface NativeAccessibilityState {
  disabled?: boolean
  selected?: boolean
  /** Checked, for checkboxes and switches. `mixed` is the in-between state. */
  checked?: boolean | 'mixed'
  expanded?: boolean
  busy?: boolean
}

/**
 * A system control: something you touch and that can be switched off.
 *
 * `enabled` lives here rather than being repeated in each of them because it
 * means the same thing in all eight and on both platforms —`UIControl.isEnabled`
 * and `View.setEnabled`—, including the grey-out and the fact that it stops
 * responding to touch, which the system does and not us.
 *
 * Text fields do not belong here: they already have `editable`, the same idea
 * under the name a field uses.
 */
@Directive()
export abstract class NativeControl extends NativeVisual {
  constructor() {
    super()
    this.push({
      enabled: this.enabled
    })
  }

  readonly enabled = input<boolean | null>(true)
}

/**
 * A stack of screens.
 *
 * Its children overlap and fill everything —the core imposes that, not the
 * styling— and the host animates the way in and the way out according to
 * `transition`. It is rarely used bare: the usual thing is `NativeStack`, which
 * wires it to the router.
 */
@Directive({ selector: 'an-stack-view' })
export class StackView extends NativeVisual {
  constructor() {
    super()
    this.push({
      transition: this.transition
    })
  }

  /**
   * The direction of the next transition. Whoever navigates decides it, being
   * the only one who knows whether this is going forward or back.
   */
  readonly transition = input<'push' | 'pop' | 'none' | null>('none')

  /** The edge gesture on iOS, the physical button on Android. */
  readonly back = outputFromObservable(this.nativeEvent<void>('back'))
}

/** What `UIScrollView` has and Android's does not. */
export type IosScrollViewProps = {
  /**
   * Scrolling comes to rest at multiples of the view's size.
   * `UIScrollView.isPagingEnabled`. Android does not ship it: its answer is
   * `ViewPager2`, which is another view with its own adapter, not a prop.
   */
  pagingEnabled?: boolean
  /**
   * What the keyboard does while scrolling. `keyboardDismissMode`. On Android
   * the keyboard does not hide on scroll and there is nothing to ask it for.
   */
  keyboardDismissMode?: 'none' | 'onDrag' | 'interactive'
}

const SCROLL_VIEW_IOS = platformKeys('an-scroll-view', 'ios', [
  'pagingEnabled',
  'keyboardDismissMode'
])

@Directive({ selector: 'an-scroll-view' })
export class ScrollView extends NativeVisual {
  constructor() {
    super()
    this.push({
      showsScrollIndicator: this.showsScrollIndicator,
      scrollEnabled: this.scrollEnabled,
      bounces: this.bounces,
      refreshing: this.refreshing
    })
    this.pushPlatform(SCROLL_VIEW_IOS, this.ios)
  }

  readonly showsScrollIndicator = input<boolean | null>(null)

  /**
   * Whether the finger moves the content.
   *
   * Switched off, the view goes on clipping and the content can still be
   * scrolled from code: what is taken away is the gesture.
   */
  readonly scrollEnabled = input<boolean | null>(true)

  readonly ios = input<IosScrollViewProps | null>(null)

  /** iOS's bounce on reaching the end. */
  readonly bounces = input<boolean | null>(null)

  /**
   * Whether it is refreshing. Setting it to `false` closes the spinner; the
   * gesture itself opens it, not this prop.
   */
  readonly refreshing = input<boolean | null>(false)

  /**
   * Pull to refresh.
   *
   * On iOS the system draws it with a `UIRefreshControl`. Android ships none in
   * the platform —`SwipeRefreshLayout` lives in AndroidX— so the same arc the
   * system draws is drawn by hand.
   */
  readonly refresh = outputFromObservable(this.nativeEvent<void>('refresh'))

  /**
   * Emitted on every frame of the scroll. Layout computes the `contentSize` by
   * itself: it is the size the children take up, and the core sends it to the
   * `UIScrollView` whenever it changes.
   */
  readonly scroll = outputFromObservable(this.nativeEvent<NativeScrollEvent>('scroll'))
}

/** The real size of an image once loaded, in points. */
export interface NativeImageLoadEvent {
  width: number
  height: number
}

@Directive({ selector: 'an-image' })
export class Image extends NativeVisual {
  constructor() {
    super()
    // This listener is not optional: layout cannot place something whose size
    // it does not know, and only the image knows how big it is. It is always
    // registered, even when the template is not listening for `load`.
    const unlisten = this.renderer.listen(this.node, 'load', (payload) => {
      const size = payload as NativeImageLoadEvent
      this.set('intrinsicWidth', size.width)
      this.set('intrinsicHeight', size.height)
    })
    inject(DestroyRef).onDestroy(unlisten)
    this.push({
      source: this.source,
      resizeMode: this.resizeMode,
      intrinsicWidth: this.intrinsicWidth,
      intrinsicHeight: this.intrinsicHeight
    })
  }

  /**
   * The image's path. With no scheme it is a resource in the app's bundle; with
   * `http` or `https` it is fetched over the network and appears when it lands.
   */
  readonly source = input<string | null>(null)

  /** `contain` by default; also `cover`, `stretch` and `center`. */
  readonly resizeMode = input<'contain' | 'cover' | 'stretch' | 'center' | null>(null)

  /**
   * The intrinsic size. It fills itself in when the image loads; setting it by
   * hand reserves the space before the image arrives and avoids the jump.
   */
  readonly intrinsicWidth = input<number | null>(null)

  readonly intrinsicHeight = input<number | null>(null)

  readonly load = outputFromObservable(this.nativeEvent<NativeImageLoadEvent>('load'))
}

/** What Android's `TextView` has and iOS's `UILabel` does not. */
export type AndroidTextProps = {
  /**
   * Lets the text be selected and copied.
   *
   * `UILabel` does not do it: on iOS a selectable text is a disabled
   * `UITextView`, which is another view and another measurement, so it is not
   * imitated here.
   */
  selectable?: boolean
}

const TEXT_ANDROID = platformKeys('an-text', 'android', ['selectable'])

@Directive({ selector: 'an-text' })
export class Text extends NativeVisual {
  constructor() {
    super()
    this.push({
      color: this.color,
      fontSize: this.fontSize,
      fontWeight: this.fontWeight,
      fontStyle: this.fontStyle,
      fontFamily: this.fontFamily,
      letterSpacing: this.letterSpacing,
      lineHeight: this.lineHeight,
      textAlign: this.textAlign,
      numberOfLines: this.numberOfLines,
      textDecoration: this.textDecoration
    })
    this.pushPlatform(TEXT_ANDROID, this.android)
  }

  readonly color = input<string | null>(null)

  readonly fontSize = input<number | null>(null)

  /** `'bold'`, `'normal'` or CSS's numeric scale (100..900). */
  readonly fontWeight = input<string | number | null>(null)

  readonly fontStyle = input<'normal' | 'italic' | null>(null)

  readonly fontFamily = input<string | null>(null)

  readonly letterSpacing = input<number | null>(null)

  readonly lineHeight = input<number | null>(null)

  readonly textAlign = input<'left' | 'center' | 'right' | 'justify' | null>(null)

  /** 0 or null = no limit. */
  readonly numberOfLines = input<number | null>(null)

  /** Underline or strikethrough. A plain single line, which is what anybody
   * ever asks for. */
  readonly textDecoration = input<'none' | 'underline' | 'lineThrough' | null>('none')

  readonly android = input<AndroidTextProps | null>(null)
}

/** What UIKit's field has and Android's does not. */
export type IosTextInputProps = {
  /**
   * The little cross that empties the field. `UITextField.clearButtonMode`.
   * Android has none: over there the convention is to delete with the keyboard.
   */
  clearButtonMode?: 'never' | 'whileEditing' | 'always'
  /**
   * The frame UIKit draws around the field. `UITextField.borderStyle`. On
   * Android an `EditText`'s background comes from the theme, and here it is
   * deliberately taken away so that the template supplies the frame.
   */
  borderStyle?: 'none' | 'line' | 'bezel' | 'roundedRect'
}

/** What Android's field has and UIKit's does not. */
export type AndroidTextInputProps = {
  /** On taking focus, the whole text ends up selected. */
  selectAllOnFocus?: boolean
  /** Hides the caret. `EditText.setCursorVisible`. */
  cursorVisible?: boolean
}

const TEXT_INPUT_IOS = platformKeys('an-text-input', 'ios', ['clearButtonMode', 'borderStyle'])
const TEXT_INPUT_ANDROID = platformKeys('an-text-input', 'android', [
  'selectAllOnFocus',
  'cursorVisible'
])

@Directive({ selector: 'an-text-input' })
export class TextInput extends NativeVisual {
  constructor() {
    super()
    this.push({
      placeholder: this.placeholder,
      value: this.value,
      secureTextEntry: this.secureTextEntry,
      editable: this.editable,
      color: this.color,
      fontSize: this.fontSize,
      fontWeight: this.fontWeight,
      fontFamily: this.fontFamily,
      textAlign: this.textAlign,
      keyboardType: this.keyboardType,
      returnKeyType: this.returnKeyType,
      autoCapitalize: this.autoCapitalize,
      autoCorrect: this.autoCorrect,
      placeholderColor: this.placeholderColor
    })
    this.pushPlatform(TEXT_INPUT_IOS, this.ios)
    this.pushPlatform(TEXT_INPUT_ANDROID, this.android)
  }

  readonly placeholder = input<string | null>(null)

  /**
   * The host only writes into the field when the text really does differ:
   * assigning on every keystroke would send the caret to the end.
   */
  readonly value = input<string | null>(null)

  readonly secureTextEntry = input<boolean | null>(null)

  readonly editable = input<boolean | null>(null)

  readonly color = input<string | null>(null)

  readonly fontSize = input<number | null>(null)

  /** `'bold'`, `'normal'` or CSS's numeric scale (100..900). */
  readonly fontWeight = input<string | number | null>(null)

  readonly fontFamily = input<string | null>(null)

  readonly textAlign = input<'left' | 'center' | 'right' | null>(null)

  /**
   * Which keyboard comes up.
   *
   * This is not decoration: an email field with the text keyboard makes people
   * hunt for the at sign, and a phone field with letters lets them type things
   * that are not a phone number. On iOS it is `keyboardType`; on Android the
   * `inputType`, which also changes what the field will accept.
   */
  readonly keyboardType = input<'default' | 'numeric' | 'decimal' | 'email' | 'phone' | 'url' | null>('default')

  /**
   * What the return key says. It changes the label and, with it, what the person
   * expects to happen when they press it.
   */
  readonly returnKeyType = input<'default' | 'done' | 'go' | 'next' | 'search' | 'send' | null>('default')

  readonly autoCapitalize = input<'none' | 'sentences' | 'words' | 'characters' | null>('sentences')

  /** The system's autocorrect. Turning it off is the usual thing for a username
   * or a code. */
  readonly autoCorrect = input<boolean | null>(true)

  /** The placeholder's colour, which does not have to be the text's. */
  readonly placeholderColor = input<string | null>(null)

  readonly ios = input<IosTextInputProps | null>(null)

  readonly android = input<AndroidTextInputProps | null>(null)

  /** Paired with `value`, it enables `[(value)]` in the template. */
  readonly valueChange = outputFromObservable(
    this.nativeEvent<{ value: string }>('change').pipe(map((event) => event.value))
  )

  /** The keyboard's return key. */
  readonly submit = outputFromObservable(
    this.nativeEvent<{ value: string }>('submit').pipe(map((event) => event.value))
  )
}

/** The selected tab. */
export interface NativeTabSelectEvent {
  index: number
}

/** What iOS's bar has and Material's does not. */
export type IosTabBarProps = {
  /**
   * Whether what goes past behind it shows through. `UITabBar.isTranslucent`.
   * Material's bar is opaque by design and has no switch for this.
   */
  translucent?: boolean
}

const TAB_BAR_IOS = platformKeys('an-tab-bar', 'ios', ['translucent'])

/**
 * The system tab bar.
 *
 * It is the real bar —`UITabBar` on iOS— and not a row of views imitating one:
 * it inherits its typeface, its translucent background and its behaviour with
 * large accessibility text.
 */
@Directive({ selector: 'an-tab-bar' })
export class TabBar extends NativeVisual {
  constructor() {
    super()
    this.push({
      items: () => JSON.stringify(this.items() ?? []),
      icons: () => JSON.stringify(this.icons() ?? []),
      selectedIndex: this.selectedIndex,
      color: this.color,
      unselectedColor: this.unselectedColor
    })
    this.pushPlatform(TAB_BAR_IOS, this.ios)
  }

  /** Titles, in order. */
  // The protocol carries no lists and a tab bar is not enough reason to add
  // them: they travel as JSON.
  readonly items = input<readonly string[] | null>(null)

  /**
   * Icons, in the same order as the titles.
   *
   * The names are `<an-icon>`'s, so the common ones work —`home`, `search`,
   * `settings`— and so do each platform's native ones. A tab bar with no icons
   * is legal, but it is not what anybody expects.
   */
  readonly icons = input<readonly string[] | null>(null)

  readonly selectedIndex = input<number | null>(0)

  /** The active tab's colour. */
  readonly color = input<string | null>(null)

  /**
   * The colour of the rest.
   *
   * Without this the active one came out dimmed, which on a light bar can end up
   * almost the colour of the background: the labels are there and cannot be
   * read.
   */
  readonly unselectedColor = input<string | null>(null)

  readonly ios = input<IosTabBarProps | null>(null)

  readonly select = outputFromObservable(
    this.nativeEvent<NativeTabSelectEvent>('select').pipe(map((event) => event.index))
  )
}

/** What Material's switch has and UIKit's does not. */
export type AndroidSwitchProps = {
  /**
   * The track's colour when the switch is off.
   *
   * `UISwitch` does not expose it: what goes around is putting a
   * `backgroundColor` and a corner radius on a system control so its background
   * shows through from behind, and that breaks the moment Apple changes the
   * control's height. On iOS it keeps the system's colour.
   */
  trackColor?: string
}

const SWITCH_ANDROID = platformKeys('an-switch', 'android', ['trackColor'])

/** The system switch. */
@Directive({ selector: 'an-switch' })
export class Switch extends NativeControl {
  constructor() {
    super()
    this.push({
      on: this.on,
      color: this.color,
      thumbColor: this.thumbColor
    })
    this.pushPlatform(SWITCH_ANDROID, this.android)
  }

  readonly on = input<boolean | null>(false)

  /** The colour when it is on. */
  readonly color = input<string | null>(null)

  /** The thumb's colour, the part that moves. */
  readonly thumbColor = input<string | null>(null)

  readonly android = input<AndroidSwitchProps | null>(null)

  /** Paired with `on`, it enables `[(on)]` in the template. */
  readonly onChange = outputFromObservable(
    this.nativeEvent<{ value: boolean }>('change').pipe(map((event) => event.value))
  )
}

/** What UIKit's slider has and Material's does not. */
export type IosSliderProps = {
  /**
   * Whether it reports while being dragged or only on release.
   * `UISlider.isContinuous`. Material's always reports while being dragged and
   * that cannot be changed.
   */
  continuous?: boolean
}

/** What Material's slider has and UIKit's does not. */
export type AndroidSliderProps = {
  /**
   * The jump between values. `Slider.setStepSize`.
   *
   * It is not a common prop because `UISlider` is continuous and has no steps.
   * Rounding the value in the host is possible, but then the finger goes one way
   * and the value another: Material's snaps to the steps, and promising "steps"
   * while giving two different behaviours is worse than saying only Android has
   * it.
   *
   * It has to divide the range exactly or Material complains; if it does not,
   * the host says so in the log and leaves the slider continuous.
   */
  stepSize?: number
}

const SLIDER_IOS = platformKeys('an-slider', 'ios', ['continuous'])
const SLIDER_ANDROID = platformKeys('an-slider', 'android', ['stepSize'])

/** The system slider. */
@Directive({ selector: 'an-slider' })
export class Slider extends NativeControl {
  constructor() {
    super()
    this.push({
      value: this.value,
      minimumValue: this.minimumValue,
      maximumValue: this.maximumValue,
      color: this.color,
      minimumTrackColor: this.minimumTrackColor,
      maximumTrackColor: this.maximumTrackColor,
      thumbColor: this.thumbColor
    })
    this.pushPlatform(SLIDER_IOS, this.ios)
    this.pushPlatform(SLIDER_ANDROID, this.android)
  }

  readonly value = input<number | null>(0)

  readonly minimumValue = input<number | null>(0)

  readonly maximumValue = input<number | null>(1)

  readonly color = input<string | null>(null)

  /** The stretch already covered, from the left to the thumb. */
  readonly minimumTrackColor = input<string | null>(null)

  /** The stretch still to go. */
  readonly maximumTrackColor = input<string | null>(null)

  readonly thumbColor = input<string | null>(null)

  readonly ios = input<IosSliderProps | null>(null)

  readonly android = input<AndroidSliderProps | null>(null)

  readonly valueChange = outputFromObservable(
    this.nativeEvent<{ value: number }>('change').pipe(map((event) => event.value))
  )
}

/** A loading spinner. It hides itself when it stops. */
@Directive({ selector: 'an-activity-indicator' })
export class ActivityIndicator extends NativeVisual {
  constructor() {
    super()
    this.push({
      animating: this.animating,
      color: this.color
    })
  }

  readonly animating = input<boolean | null>(true)

  readonly color = input<string | null>(null)
}

/** A determinate progress bar. `progress` runs from 0 to 1. */
@Directive({ selector: 'an-progress-bar' })
export class ProgressBar extends NativeVisual {
  constructor() {
    super()
    this.push({
      progress: this.progress,
      color: this.color
    })
  }

  readonly progress = input<number | null>(0)

  readonly color = input<string | null>(null)
}

/**
 * What iOS's button has and Android's does not.
 *
 * It is a type alias and not an interface on purpose: an interface cannot be
 * passed as a `Record<string, unknown>` —TypeScript gives it no index
 * signature— and the key walk `platform()` does needs one. An alias can.
 */
export type IosButtonProps = {
  /**
   * A second, smaller line beneath the label.
   *
   * `UIButtonConfiguration.subtitle`. Material has no equivalent: a two-line
   * button is not a Material button, so it is not imitated.
   */
  subtitle?: string
}

/** What Material's button has and UIKit's does not. */
export type AndroidButtonProps = {
  /** The colour of the ripple under the finger. `MaterialButton.setRippleColor`. */
  rippleColor?: string
  /**
   * An upper-case label. It was the norm in Material 2 and stopped being so in
   * Material 3, but it is still there and there are brands that ask for it. On
   * iOS a button has never had its label in upper case.
   */
  allCaps?: boolean
}

const BUTTON_IOS = platformKeys('an-button', 'ios', ['subtitle'])
const BUTTON_ANDROID = platformKeys('an-button', 'android', ['rippleColor', 'allCaps'])

/** The system button, with its own typeface and its own response to touch. */
@Directive({ selector: 'an-button' })
export class Button extends NativeControl {
  constructor() {
    super()
    this.push({
      title: this.title,
      color: this.color,
      variant: this.variant,
      icon: this.icon,
      iconPosition: this.iconPosition,
      fontSize: this.fontSize,
      fontWeight: this.fontWeight
    })
    this.pushPlatform(BUTTON_IOS, this.ios)
    this.pushPlatform(BUTTON_ANDROID, this.android)
  }

  readonly title = input<string | null>('')

  readonly color = input<string | null>(null)

  /**
   * How it looks: the label alone, filled, with a faint background of the same
   * colour, or outlined and empty inside.
   *
   * `text` by default, which is what a plain button does on iOS. The others are
   * drawn by the platform —`UIButtonConfiguration` on iOS— except on Android,
   * where Material 3's buttons are not in the platform and the pill is drawn by
   * hand over a real `Button`.
   *
   * There is no `elevated`: Material has it and UIKit has nothing like it, so it
   * would be a variant that only does something on half the platforms. Whoever
   * wants it can go through `[android]`.
   */
  readonly variant = input<'text' | 'filled' | 'tonal' | 'outlined' | null>('text')

  /**
   * An icon beside the label, by name, just like `<an-icon>`.
   *
   * An SF Symbol on iOS and a Material Symbol on Android, so the same template
   * gives each platform the icon that belongs to it.
   */
  readonly icon = input<string | null>(null)

  /** Which side of the label. `leading` by default. */
  readonly iconPosition = input<'leading' | 'trailing' | null>('leading')

  readonly fontSize = input<number | null>(null)

  /** `'bold'`, `'normal'` or CSS's numeric scale (100..900). */
  readonly fontWeight = input<string | number | null>(null)

  readonly ios = input<IosButtonProps | null>(null)

  readonly android = input<AndroidButtonProps | null>(null)
}

/**
 * Picking one of several options, all of them on screen.
 *
 * `UISegmentedControl` on iOS. Android ships no platform equivalent —Material's
 * lives in a separate library— so it is drawn out of system views, like the tab
 * bar.
 */
@Directive({ selector: 'an-segmented-control' })
export class SegmentedControl extends NativeControl {
  constructor() {
    super()
    this.push({
      items: () => JSON.stringify(this.items() ?? []),
      selectedIndex: this.selectedIndex,
      color: this.color
    })
  }

  readonly items = input<readonly string[] | null>(null)

  readonly selectedIndex = input<number | null>(0)

  readonly color = input<string | null>(null)

  readonly change = outputFromObservable(this.nativeEvent<NativeIndexEvent>('change'))
}

/**
 * Up and down, one at a time.
 *
 * `UIStepper` on iOS. Android has no platform equivalent, so it is put together
 * out of two system buttons.
 */
@Directive({ selector: 'an-stepper' })
export class Stepper extends NativeControl {
  constructor() {
    super()
    this.push({
      value: this.value,
      minimumValue: this.minimumValue,
      maximumValue: this.maximumValue,
      stepValue: this.step
    })
  }

  readonly value = input<number | null>(0)

  readonly minimumValue = input<number | null>(0)

  readonly maximumValue = input<number | null>(100)

  /** How much each tap goes up or down. One by default. */
  readonly step = input<number | null>(1)

  readonly change = outputFromObservable(this.nativeEvent<NativeValueEvent>('change'))
}

/**
 * The system search field, with its magnifier and its clear button.
 *
 * It is a control of its own and not an `<an-text-input>` with an icon beside
 * it: the system gives it the keyboard with the search key, the cancel
 * behaviour and the look people recognise as "this is where you search".
 */
@Directive({ selector: 'an-search-bar' })
export class SearchBar extends NativeControl {
  constructor() {
    super()
    this.push({
      value: this.value,
      placeholder: this.placeholder
    })
  }

  readonly value = input<string | null>('')

  readonly placeholder = input<string | null>(null)

  readonly input = outputFromObservable(this.nativeEvent<NativeTextEvent>('input'))
  readonly submit = outputFromObservable(this.nativeEvent<NativeTextEvent>('submit'))
}

/**
 * Picking one of several options from a list that drops down.
 *
 * On iOS it is a button that opens a `UIMenu`: there is no dropdown control, and
 * `UIPickerView` is the full-screen wheel, which is a different thing and no
 * longer what the system uses for a short list. On Android it is a `Spinner`.
 */
@Directive({ selector: 'an-select' })
export class Select extends NativeControl {
  constructor() {
    super()
    this.push({
      items: () => JSON.stringify(this.items() ?? []),
      selectedIndex: this.selectedIndex
    })
  }

  readonly items = input<readonly string[] | null>(null)

  readonly selectedIndex = input<number | null>(0)

  readonly change = outputFromObservable(this.nativeEvent<NativeIndexEvent>('change'))
}

/**
 * The system date and time picker.
 *
 * The value goes out and comes back in milliseconds since 1970 —what `Date`
 * gives and takes— because a formatted date depends on the device's language
 * and time zone, and each platform settles that for itself.
 */
@Directive({ selector: 'an-date-picker' })
export class DatePicker extends NativeControl {
  constructor() {
    super()
    this.push({
      // A date travels in milliseconds since 1970, which is what `Date` gives
      // and takes: formatting it depends on the device's language and time
      // zone, and each platform settles that for itself.
      value: () => {
        const value = this.value()
        return value instanceof Date ? value.getTime() : (value ?? Date.now())
      },
      mode: this.mode
    })
  }

  readonly value = input<number | Date | null>(null)

  readonly mode = input<'date' | 'time' | 'dateAndTime' | null>('date')

  readonly change = outputFromObservable(this.nativeEvent<NativeValueEvent>('change'))
}

/**
 * A header with a title and a back button.
 *
 * `UINavigationBar` on iOS and `Toolbar` on Android. Outside a
 * `UINavigationController` there is no automatic back button, so one is put
 * there with the same symbol and in the same place; navigating is still the
 * router's business, since the router is what knows where you go back to.
 */
@Directive({ selector: 'an-navigation-bar' })
export class NavigationBar extends NativeVisual {
  constructor() {
    super()
    this.push({
      title: this.title,
      showsBack: this.showsBack,
      backTitle: this.backTitle
    })
  }

  readonly title = input<string | null>('')

  readonly showsBack = input<boolean | null>(false)

  /**
   * The back button's label. iOS only: on Android the toolbar carries nothing
   * but the arrow, which is what any app on that platform does.
   */
  readonly backTitle = input<string | null>(null)

  readonly back = outputFromObservable(this.nativeEvent<void>('back'))
}

/**
 * A multi-line text field.
 *
 * It is a primitive of its own and not a prop on `<an-text-input>` because on
 * iOS they are two separate controls —`UITextField` and `UITextView`— and
 * swapping one for the other with the view already mounted is not possible.
 */
@Directive({ selector: 'an-textarea' })
export class TextArea extends NativeVisual {
  constructor() {
    super()
    this.push({
      value: this.value,
      editable: this.editable,
      color: this.color
    })
  }

  readonly value = input<string | null>('')

  readonly editable = input<boolean | null>(true)

  readonly color = input<string | null>(null)

  readonly change = outputFromObservable(this.nativeEvent<NativeTextEvent>('change'))
}

/**
 * An embedded browser: `WKWebView` on iOS, `WebView` on Android.
 *
 * It takes an address or a loose piece of HTML. It is one more view in the tree:
 * it occupies whatever space layout gives it and it can sit beside any other.
 */
@Directive({ selector: 'an-web-view' })
export class WebView extends NativeVisual {
  constructor() {
    super()
    this.push({
      url: this.url,
      html: this.html
    })
  }

  readonly url = input<string | null>(null)

  readonly html = input<string | null>(null)
}

/**
 * A map.
 *
 * On iOS it is `MKMapView`, the system's own. On Android there is none in the
 * platform —Google's lives in Play Services, which wants an API key and a
 * dependency this build cannot bring in— so over there OpenStreetMap tiles are
 * drawn onto a `Canvas`: it is a native view, but it is not the system's map and
 * it brings neither directions nor search.
 */
@Directive({ selector: 'an-map-view' })
export class MapView extends NativeVisual {
  constructor() {
    super()
    this.push({
      latitude: this.latitude,
      longitude: this.longitude,
      zoom: this.zoom,
      showsUser: this.showsUser
    })
  }

  readonly latitude = input<number | null>(0)

  readonly longitude = input<number | null>(0)

  /**
   * A zoom level in the tile style: 0 is the whole world and each level is twice
   * as close. MapKit does not work that way —it works in how many degrees are on
   * screen— and the host does the conversion, so that the same number means the
   * same thing on both platforms.
   */
  readonly zoom = input<number | null>(12)

  /** The dot showing where you are. iOS only: Android's map does not know. */
  readonly showsUser = input<boolean | null>(false)
}

/**
 * Video.
 *
 * `VideoView` on Android. On iOS there is no video view: there is a layer
 * —`AVPlayerLayer`— that hangs off any view, so the host hangs it there and
 * keeps its frame in step. A layer does not stretch along with its view.
 */
@Directive({ selector: 'an-video-view' })
export class VideoView extends NativeVisual {
  constructor() {
    super()
    this.push({
      url: this.url,
      playing: this.playing,
      muted: this.muted
    })
  }

  readonly url = input<string | null>(null)

  readonly playing = input<boolean | null>(false)

  /** iOS only: `VideoView` does not hand over the player inside it. */
  readonly muted = input<boolean | null>(false)
}

/**
 * A system icon.
 *
 * Nothing is drawn and no icon set is bundled: on iOS it is an SF Symbol and on
 * Android a system drawable, both asked for by name. An icon like that ages
 * along with the platform —it changes when the system changes— instead of
 * staying pinned to the day it was dropped into the project, and it arrives with
 * the weight and the stroke that belong to that version.
 *
 * The common names —`home`, `search`, `settings`, `back`, `close`, `add`,
 * `delete`, `edit`, `share`, `star`, `menu`, `check`…— are translated into each
 * platform's own name, so the same template works on both. For anything
 * specific, the native name is written directly: any SF Symbol
 * (`square.and.arrow.up`) or any Android drawable.
 */
@Directive({ selector: 'an-icon' })
export class Icon extends NativeVisual {
  constructor() {
    super()
    this.push({
      name: this.name,
      iconSize: this.size,
      iconWeight: this.weight,
      color: this.color
    })
    // The size is also the box's size. It goes outside the push because it is
    // not a prop: it is two styles, and layout has to know them so that an
    // `<an-icon>` with no measurements does not end up invisible.
    effect(() => {
      const points = this.size()
      this.renderer.setStyle(this.node, 'width', points)
      this.renderer.setStyle(this.node, 'height', points)
    })
  }

  readonly name = input<string | null>(null)

  /**
   * Points. Besides pinning the view's size down, it picks the symbol's stroke:
   * on iOS a large icon is not the small one scaled up, it is a different
   * drawing.
   *
   * The 24 is the default and it always arrives, even with no `[size]`: a signal
   * input is read whether or not anybody writes it, which is exactly what an
   * unbound setter did not do.
   */
  readonly size = input(24)

  /** The stroke's weight, on the typographic scale: 100..900. */
  readonly weight = input<number | null>(null)

  readonly color = input<string | null>(null)
}

/**
 * One of the heights a sheet is allowed to rest at.
 *
 * The two names are UIKit's own —`.medium()` is about half the screen and
 * `.large()` is the full sheet— and a number is `.custom(resolver:)` in points,
 * measured from the bottom. A height taller than the sheet can be is brought
 * down to that maximum instead of throwing the detent away, which is what UIKit
 * does with a resolver that oversteps.
 */
export type IosSheetDetent = 'medium' | 'large' | number

/** What a sheet on iOS has and a dialog on Android does not. */
export type IosModalProps = {
  /**
   * Where the sheet is allowed to rest, in the order UIKit is given them.
   * `UISheetPresentationController.detents`. Without it, the two the system
   * suggests: medium and large.
   *
   * Android has no equivalent to ask for. Its `Dialog` has no resting heights
   * at all —it is as tall as its content— and the closest thing on the platform
   * is `BottomSheetBehavior`, which lives in Material, not in the framework,
   * and which speaks of a peek height and an expanded state rather than of a
   * list of stops. A common `[detents]` would be a name Android could only
   * pretend to honour.
   */
  detents?: readonly IosSheetDetent[]
}

const MODAL_IOS = platformKeys('an-modal', 'ios', ['detents'])

/**
 * Content presented on top of everything.
 *
 * It really is presented: a `UIViewController` on iOS and a `Dialog` on Android,
 * not a view laid over the others. It looks similar, but the difference matters:
 * the system knows there is something modal in front, so VoiceOver and TalkBack
 * stop reading what is behind, Android's back button closes it, and it does not
 * compete for draw order with the system's own dialogs.
 */
@Directive({ selector: 'an-modal' })
export class Modal extends NativeVisual {
  constructor() {
    super()
    this.push({
      visible: this.visible,
      presentation: this.presentation
    })
    this.pushPlatform(MODAL_IOS, this.ios)
  }

  readonly visible = input<boolean | null>(false)

  /**
   * `fullScreen` covers the screen; `sheet` comes up from the bottom with the
   * system's grabber, resting where `[ios].detents` says.
   */
  readonly presentation = input<'fullScreen' | 'sheet' | null>(null)

  /**
   * Only read when `presentation` is `sheet`: a modal that covers the screen
   * has nowhere to rest.
   */
  readonly ios = input<IosModalProps | null>(null)

  /**
   * It closed.
   *
   * The user can close it without going through the template —dragging the sheet
   * down on iOS, with the back button on Android— so this has to be listened to:
   * otherwise the signal that opened it goes on saying it is still open and
   * setting it back to `true` does nothing.
   */
  readonly dismiss = outputFromObservable(this.nativeEvent<void>('dismiss'))
}

/**
 * A system dialog.
 *
 * It is not a layer drawn by the framework: it is a real `UIAlertController` and
 * a real `AlertDialog`, with their look, their animation and their behaviour
 * with VoiceOver and TalkBack. It takes up no room in the layout.
 */
@Directive({ selector: 'an-alert' })
export class Alert extends NativeVisual {
  constructor() {
    super()
    this.push({
      sheet: this.sheet,
      visible: this.visible,
      title: this.title,
      message: this.message,
      buttons: () => JSON.stringify(this.buttons() ?? [])
    })
  }

  /**
   * An action sheet instead of a centred dialog.
   *
   * It is the way to offer several actions on something that has just been
   * tapped; the centred dialog is for confirming or warning. On iOS it comes up
   * from the bottom, on Android it is a list.
   */
  readonly sheet = input<boolean | null>(false)

  readonly visible = input<boolean | null>(false)

  readonly title = input<string | null>('')

  readonly message = input<string | null>('')

  /** The buttons' titles, in order. With none, an "OK" shows up. */
  readonly buttons = input<readonly string[] | null>(null)

  /** The index of the button that was pressed. */
  readonly select = outputFromObservable(
    this.nativeEvent<NativeTabSelectEvent>('select').pipe(map((event) => event.index))
  )
}

/**
 * A view a plugin brings.
 *
 * Everything else in this file is a control the framework mounts on every host.
 * This one is a hole: the plugin registers a view under a name, and this mounts
 * whatever that name resolves to on the platform being built.
 *
 * ```html
 * <an-custom [view]="'barcode-preview'" [style.height]="'320'" />
 * ```
 *
 * **Give it a size.** A plugin view is a container as far as layout is
 * concerned and is never measured by its content: measuring would mean the core
 * calling into a plugin during layout, on the engine thread, and the measuring
 * path is built the other way round. A view with no size comes out at zero,
 * which looks like a plugin that does not work.
 *
 * A name no plugin registered mounts nothing and says so once, in the log —
 * the same as every other gap here.
 */
@Directive({ selector: 'an-custom' })
export class Custom extends NativeVisual {
  constructor() {
    super()
    // Prefixed, like the platform objects: it is not a prop of a control, it is
    // the question "which view is this", and the prefix keeps it greppable in
    // the hosts that answer it.
    this.push({ 'an:view': this.view })
  }

  /** The name the plugin registered its view under. */
  readonly view = input<string | null>(null)
}

/** For importing them all at once into a standalone component. */
export const NATIVE_PRIMITIVES = [
  View,
  Text,
  Image,
  ScrollView,
  TextInput,
  StackView,
  TabBar,
  Switch,
  Slider,
  ActivityIndicator,
  ProgressBar,
  Button,
  DatePicker,
  Icon,
  Modal,
  MapView,
  NavigationBar,
  TextArea,
  VideoView,
  WebView,
  SearchBar,
  SegmentedControl,
  Select,
  Stepper,
  Alert
,
  Custom] as const
