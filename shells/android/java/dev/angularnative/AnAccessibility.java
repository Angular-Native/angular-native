package dev.angularnative;

import android.os.Bundle;
import android.view.View;
import android.view.ViewGroup;
import android.view.accessibility.AccessibilityEvent;
import androidx.core.view.AccessibilityDelegateCompat;
import androidx.core.view.ViewCompat;
import androidx.core.view.accessibility.AccessibilityNodeInfoCompat;
import androidx.core.view.accessibility.AccessibilityNodeInfoCompat.AccessibilityActionCompat;
import androidx.core.view.accessibility.AccessibilityNodeProviderCompat;

/**
 * The six accessibility props, put where Android keeps them.
 *
 * On iOS half a dozen accessibility props are half a dozen properties of
 * `UIView`. Not here: only two of them — the name, and whether this counts as
 * one element — are properties of the view. The role, the value and the state
 * do not exist as fields; they exist in the `AccessibilityNodeInfo` that a view
 * fills in every time a screen reader asks about it, and the only way to put
 * something in there is to intercept that moment with an
 * `AccessibilityDelegate`.
 *
 * Hence the shape of this file: one state per node holding whatever the
 * template said, and a delegate that pours it onto the node the system has
 * just built.
 *
 * <h2>Only what the template actually set gets overwritten</h2>
 *
 * A `MaterialButton` already announces itself as a button and a
 * `MaterialSwitch` already says whether it is on: writing our own default role
 * and state over them would make worse what the library already gets right.
 * So every field of the state is null until someone sets it, the delegate
 * first calls the one underneath — which is what puts the widget's own answers
 * in — and only then writes the fields that did arrive. A button with no
 * `[accessibilityRole]` keeps announcing itself the way Material announces it.
 *
 * And for the same reason the delegate is not installed on every node, but on
 * the first one to receive a prop that needs it. An app that uses none of this
 * does not pay for a single object.
 */
final class AnAccessibility {

    private static final String TAG = "angular-native";

    /** The template's `checked`: Android only has two of the three. */
    static final int UNCHECKED = 0;
    static final int CHECKED = 1;
    static final int MIXED = 2;

    private AnAccessibility() {}

    /**
     * What the template has said about a node.
     *
     * Everything null by default, and that is half the design: null means "the
     * template has not spoken about this", which is not the same as "the
     * template said no". `selected = false` turns off the "selected" the
     * widget had put there; `selected = null` leaves it as it was.
     */
    static final class State {
        String label;
        /** Used as the name only when there is no `accessibilityLabel`. */
        String testID;
        String hint;
        String role;
        String value;
        Boolean disabled;
        Boolean selected;
        /** `null`, or one of the three constants above. */
        Integer checked;
        Boolean expanded;
        Boolean busy;
        Boolean accessible;
        /** The delegate is installed once and never removed. */
        boolean delegated;

        /** Whether any of this can only be told through the accessibility node. */
        boolean needsDelegate() {
            return hint != null
                    || role != null
                    || value != null
                    || disabled != null
                    || selected != null
                    || checked != null
                    || expanded != null
                    || busy != null;
        }
    }

    // ------------------------------------------------------------------ roles

    /**
     * A role from the template, translated into something Android understands.
     *
     * Android has no "role" field. What a screen reader uses to say "button"
     * is the class name of the node, which by default is that of the real
     * view. Changing it is what turns an `an-view` into a button for someone
     * who cannot see it, and it is also the only part of all this that can be
     * seen from outside with `uiautomator dump`.
     *
     * The three roles Android has no widget for — link, search and summary —
     * go through `setRoleDescription`, which is text a reader speaks verbatim.
     * That is why it comes from `strings.xml` and not from a constant: spoken
     * verbatim means that on a Spanish phone it has to be in Spanish, and a
     * string in the code would be English forever.
     */
    private static final class Role {
        final String className;
        final int description;
        final boolean checkable;
        final boolean heading;

        Role(String className, int description, boolean checkable, boolean heading) {
            this.className = className;
            this.description = description;
            this.checkable = checkable;
            this.heading = heading;
        }
    }

    private static Role roleOf(String name) {
        switch (name) {
            case "button":
                return new Role("android.widget.Button", 0, false, false);
            case "image":
                return new Role("android.widget.ImageView", 0, false, false);
            case "text":
                return new Role("android.widget.TextView", 0, false, false);
            case "checkbox":
                return new Role("android.widget.CheckBox", 0, true, false);
            case "radio":
                return new Role("android.widget.RadioButton", 0, true, false);
            case "switch":
                return new Role("android.widget.Switch", 0, true, false);
            case "slider":
                return new Role("android.widget.SeekBar", 0, false, false);
            case "header":
                // There is no heading class in Android: there is a flag, and it
                // is the one TalkBack uses to navigate by headings.
                return new Role(null, 0, false, true);
            case "link":
                return new Role(null, R.string.an_role_link, false, false);
            case "search":
                return new Role(null, R.string.an_role_search, false, false);
            case "summary":
                return new Role(null, R.string.an_role_summary, false, false);
            case "none":
                // Taking a role away is as explicit as giving one:
                // `android.view.View` is the class that means nothing, and with
                // it TalkBack stops announcing the "button" the widget
                // underneath was carrying.
                return new Role("android.view.View", 0, false, false);
            default:
                return null;
        }
    }

    // --------------------------------------------------------------- mounting

    /**
     * The part that really is a property of the view: the name, and whether
     * this counts as one element.
     *
     * It is applied whenever something changes, and not from the delegate,
     * because the system reads these from the view and not from the node, and
     * because `importantForAccessibility` decides whether the node gets to
     * exist at all.
     */
    static void apply(View view, State state) {
        // The order of these two matters, and the dump is what said so.
        //
        // `setContentDescription` does not only set the name: if the view was
        // on `AUTO`, it promotes it to `YES`. Without that, an `an-view` with a
        // name and nothing else — no touch, no role, no state — is still a
        // container the system finds uninteresting, and the name it was given
        // is read by nobody: the node does not even show up in the tree a
        // screen reader walks. Writing the importance afterwards undid that
        // promotion and left the label mute, which is exactly the kind of
        // failure that cannot be seen by reading the code.
        if (state.accessible == null) {
            view.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_AUTO);
            ViewCompat.setScreenReaderFocusable(view, false);
        } else if (state.accessible) {
            // One stop, not three. `setScreenReaderFocusable` is what puts the
            // icon, the title and the subtitle of a row into a single place
            // where the reader stops; `IMPORTANT_..._YES` is what stops the
            // system from discarding the container for having nothing of its
            // own.
            view.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_YES);
            ViewCompat.setScreenReaderFocusable(view, true);
        } else {
            // Plain `NO` would hide the view and leave its children inside the
            // tree, hanging off the grandparent. Decoration is hidden whole.
            view.setImportantForAccessibility(
                    View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS);
            ViewCompat.setScreenReaderFocusable(view, false);
        }

        // The name wins over the test identifier. Both end up in
        // `contentDescription` because on Android there is no second place:
        // UIKit's `accessibilityIdentifier` does not exist here, and the
        // `resource-id` that would be its equivalent only takes integers from
        // the app's `R`. Writing both would let whichever arrived last win,
        // and that order is not one the template controls.
        String name = state.label != null ? state.label : state.testID;
        view.setContentDescription(name);

        if (state.needsDelegate() && !state.delegated) {
            install(view, state);
            state.delegated = true;
        }

        // A state that changes has to be announced again: the reader kept the
        // node from last time. The view checks by itself whether any service
        // is listening, so with no reader running this costs nothing.
        view.sendAccessibilityEvent(AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED);
    }

    /**
     * Hooks the delegate up without throwing away the one that was there.
     *
     * Material's `Slider` has its own — an `ExploreByTouchHelper` that invents
     * a node per thumb — and putting another one on top of it would leave the
     * slider without them: it would become a mute rectangle. So ours replaces
     * nobody, it goes in front and hands over everything that is not its
     * business.
     *
     * Widgets that use no delegate but override
     * `onInitializeAccessibilityNodeInfo` instead — `CompoundButton`,
     * `Button`, `ScrollView` — still come in through the `super`, which is
     * what ends up calling them.
     */
    private static void install(View view, State state) {
        AccessibilityDelegateCompat previous = ViewCompat.getAccessibilityDelegate(view);
        ViewCompat.setAccessibilityDelegate(view, new Delegate(previous, state));
    }

    private static final class Delegate extends AccessibilityDelegateCompat {
        private final AccessibilityDelegateCompat previous;
        private final State state;

        Delegate(AccessibilityDelegateCompat previous, State state) {
            this.previous = previous;
            this.state = state;
        }

        @Override
        public void onInitializeAccessibilityNodeInfo(
                View host, AccessibilityNodeInfoCompat info) {
            if (previous != null) {
                previous.onInitializeAccessibilityNodeInfo(host, info);
            } else {
                super.onInitializeAccessibilityNodeInfo(host, info);
            }
            decorate(host, info, state);
        }

        // The rest is not ours and goes straight to whoever was there. Without
        // this, a `Slider` loses its virtual node provider and its actions.

        @Override
        public AccessibilityNodeProviderCompat getAccessibilityNodeProvider(View host) {
            return previous != null
                    ? previous.getAccessibilityNodeProvider(host)
                    : super.getAccessibilityNodeProvider(host);
        }

        @Override
        public boolean performAccessibilityAction(View host, int action, Bundle args) {
            return previous != null
                    ? previous.performAccessibilityAction(host, action, args)
                    : super.performAccessibilityAction(host, action, args);
        }

        @Override
        public void sendAccessibilityEvent(View host, int eventType) {
            if (previous != null) {
                previous.sendAccessibilityEvent(host, eventType);
            } else {
                super.sendAccessibilityEvent(host, eventType);
            }
        }

        @Override
        public void sendAccessibilityEventUnchecked(View host, AccessibilityEvent event) {
            if (previous != null) {
                previous.sendAccessibilityEventUnchecked(host, event);
            } else {
                super.sendAccessibilityEventUnchecked(host, event);
            }
        }

        @Override
        public boolean dispatchPopulateAccessibilityEvent(View host, AccessibilityEvent event) {
            return previous != null
                    ? previous.dispatchPopulateAccessibilityEvent(host, event)
                    : super.dispatchPopulateAccessibilityEvent(host, event);
        }

        @Override
        public void onPopulateAccessibilityEvent(View host, AccessibilityEvent event) {
            if (previous != null) {
                previous.onPopulateAccessibilityEvent(host, event);
            } else {
                super.onPopulateAccessibilityEvent(host, event);
            }
        }

        @Override
        public void onInitializeAccessibilityEvent(View host, AccessibilityEvent event) {
            if (previous != null) {
                previous.onInitializeAccessibilityEvent(host, event);
            } else {
                super.onInitializeAccessibilityEvent(host, event);
            }
        }

        @Override
        public boolean onRequestSendAccessibilityEvent(
                ViewGroup host, View child, AccessibilityEvent event) {
            return previous != null
                    ? previous.onRequestSendAccessibilityEvent(host, child, event)
                    : super.onRequestSendAccessibilityEvent(host, child, event);
        }
    }

    // ---------------------------------------------------------------- the pour

    private static void decorate(View host, AccessibilityNodeInfoCompat info, State state) {
        if (state.role != null) {
            Role role = roleOf(state.role);
            if (role == null) {
                // Angular's compiler already rejects a role that is not in
                // `NativeRole`, so getting here means the two lists have drifted
                // apart. It gets said: the warning is cheaper than a role that
                // is never announced and nobody knows why.
                android.util.Log.e(
                        TAG,
                        "accessibilityRole=\""
                                + state.role
                                + "\" is unknown to the Android host; the node keeps whatever"
                                + " role its view brings");
            } else {
                if (role.className != null) {
                    info.setClassName(role.className);
                }
                if (role.description != 0) {
                    info.setRoleDescription(host.getContext().getString(role.description));
                }
                if (role.checkable) {
                    info.setCheckable(true);
                }
                if (role.heading) {
                    info.setHeading(true);
                }
            }
        }

        if (state.hint != null) {
            // Android has a single hint slot, and a reader speaks it on text
            // fields. On something you press, what gets read is the label of
            // the click action — "double tap to save the draft" — so it goes
            // there too.
            info.setHintText(state.hint);
            if (host.isClickable()) {
                info.addAction(
                        new AccessibilityActionCompat(
                                AccessibilityNodeInfoCompat.ACTION_CLICK, state.hint));
            }
        }

        if (state.disabled != null) {
            // The node only. Turning the real view off is what `[enabled]`
            // does, and it also stops it responding to touch: if
            // `accessibilityState` did that too, setting the state would change
            // behaviour without anyone having asked for it.
            info.setEnabled(!state.disabled);
        }
        if (state.selected != null) {
            info.setSelected(state.selected);
        }
        if (state.checked != null) {
            info.setCheckable(true);
            info.setChecked(state.checked == CHECKED);
        }
        if (state.expanded != null) {
            if (host.isClickable()) {
                info.addAction(
                        state.expanded
                                ? AccessibilityActionCompat.ACTION_COLLAPSE
                                : AccessibilityActionCompat.ACTION_EXPAND);
            } else {
                // With nothing to press, "expanded" would be an announcement
                // that something can be expanded which cannot be expanded.
                android.util.Log.e(
                        TAG,
                        "accessibilityState.expanded on a view that does not respond to touch:"
                                + " on Android expanding is an action, so without a (press) the"
                                + " reader would announce something that cannot be done. Not set.");
            }
        }

        String stateText = stateDescription(host, state);
        if (stateText != null) {
            info.setStateDescription(stateText);
        }
    }

    /**
     * The value, and whatever else Android cannot say any other way, on a
     * single line.
     *
     * `stateDescription` is the slot Android 11 opened for exactly this: "what
     * it is worth right now", which is what a reader announces after the name
     * and the role. Three things end up here rather than one, in this order:
     *
     * 1. `accessibilityValue`, the one the template wrote;
     * 2. the "partially checked" of `checked: 'mixed'`, because on Android
     *    `setChecked` is a boolean and the third state does not fit in it;
     * 3. the "busy" of `busy`, which on Android has no field at all.
     *
     * They are joined instead of one winning: the template asked for all
     * three, and dropping two to keep one would be losing them silently.
     */
    private static String stateDescription(View host, State state) {
        StringBuilder text = new StringBuilder();
        if (state.value != null) {
            text.append(state.value);
        }
        if (state.checked != null && state.checked == MIXED) {
            append(text, host.getContext().getString(R.string.an_state_partially_checked));
        }
        if (state.busy != null && state.busy) {
            append(text, host.getContext().getString(R.string.an_state_busy));
        }
        return text.length() == 0 ? null : text.toString();
    }

    private static void append(StringBuilder text, String piece) {
        if (text.length() > 0) {
            text.append(", ");
        }
        text.append(piece);
    }
}
