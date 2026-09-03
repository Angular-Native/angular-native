package dev.angularnative;

import android.content.Context;
import android.graphics.Color;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.text.Editable;
import android.text.Layout;
import android.text.StaticLayout;
import android.text.TextPaint;
import android.text.TextUtils;
import android.text.TextWatcher;
import android.util.SparseArray;
import android.util.TypedValue;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.widget.ImageView;
import android.widget.ScrollView;
import android.widget.TextView;

/**
 * The Android host: it mounts views and measures text.
 *
 * It is the counterpart of `UikitHost`, but the split between languages is
 * different. On iOS, Rust talks to UIKit directly because the Objective-C bridge
 * is cheap and typed. Here every call crosses JNI, so Rust sends coarse orders
 * and the view logic lives in Java, which is where it is natural to write it.
 */
public final class AnHost {

    private static final int KIND_VIEW = 0;
    private static final int KIND_TEXT = 1;
    private static final int KIND_IMAGE = 3;
    private static final int KIND_SCROLL = 4;
    private static final int KIND_INPUT = 5;
    private static final int KIND_STACK = 6;
    private static final int KIND_TABBAR = 7;
    private static final int KIND_SWITCH = 8;
    private static final int KIND_SLIDER = 9;
    private static final int KIND_SPINNER = 10;
    private static final int KIND_PROGRESS = 11;
    private static final int KIND_BUTTON = 12;
    private static final int KIND_MODAL = 13;
    private static final int KIND_ALERT = 14;
    private static final int KIND_ICON = 15;
    private static final int KIND_SEGMENTED = 16;
    private static final int KIND_STEPPER = 17;
    private static final int KIND_SEARCH = 18;
    private static final int KIND_SELECT = 19;
    private static final int KIND_DATE = 20;
    private static final int KIND_NAV = 21;
    private static final int KIND_TEXT_AREA = 22;
    private static final int KIND_WEB = 23;
    private static final int KIND_MAP = 24;
    private static final int KIND_VIDEO = 25;
    /** How long a stack transition lasts. The same as on iOS. */
    private static final long TRANSITION_MS = 300;
    /** Resolution of the slider and the progress bar, which work in integers. */
    private static final int SLIDER_STEPS = 1000;

    private final Context context;
    private final AnViewGroup container;
    private final float density;
    private final SparseArray<View> views = new SparseArray<>();
    /** The content of a ScrollView: Android demands a single child. */
    private final SparseArray<AnViewGroup> scrollContent = new SparseArray<>();
    private final TextPaint measurePaint = new TextPaint(TextPaint.ANTI_ALIAS_FLAG);
    private final SparseArray<TextWatcher> watchers = new SparseArray<>();
    /** Per-corner radii in points: top-left, top-right, bottom-right, bottom-left. */
    private final SparseArray<float[]> corners = new SparseArray<>();
    /**
     * The typography arrives in separate props —family, italic, weight— and
     * `Typeface.create` wants them together: they are kept until they can be
     * applied.
     */
    private final SparseArray<FontState> fontState = new SparseArray<>();

    /**
     * What is known about each field's keyboard.
     *
     * On Android the keyboard, the capitalisation, the autocorrect and the
     * password flag are all flags of the same `inputType`, so applying one alone
     * wipes out the rest: they have to be kept and the whole thing composed
     * every time.
     */
    private final SparseArray<InputState> inputState = new SparseArray<>();

    /** A field's keyboard flags, as they arrive. */
    private static final class InputState {
        String keyboard = "default";
        String capitalize = "sentences";
        boolean correct = true;
        boolean secure;
    }

    /** What is known about a node's font, as it arrives. */
    private static final class FontState {
        String family;
        boolean italic;
        boolean bold;
        /** Letter spacing in points; Android wants it in ems. */
        Float letterSpacing;
        /** Line height in points. */
        Float lineHeight;
    }
    /** Direction of each stack's next transition: `push`, `pop` or nothing. */
    private final SparseArray<String> transitions = new SparseArray<>();
    /** Screens that came in on this frame and have not been animated yet. */
    private final java.util.List<int[]> entering = new java.util.ArrayList<>();
    /** Screens on their way out: they stay mounted until the animation ends. */
    private final java.util.List<Object[]> leaving = new java.util.ArrayList<>();
    private final java.util.Set<Integer> animatingOut = new java.util.HashSet<>();
    /** Stack nodes subscribed to `back`, for the hardware button. */
    private final java.util.List<Integer> backListeners = new java.util.ArrayList<>();
    /** Nodes subscribed to the safe area, with the insets they were already told. */
    private final SparseArray<float[]> safeArea = new SparseArray<>();
    /**
     * What the template has said about each node's accessibility.
     *
     * It only exists for nodes something has been said about. The six props
     * arrive separately and in any order, and three of them — role, value and
     * state — cannot be applied on the spot because they are not properties of
     * the view: they are kept here and `AnAccessibility` pours them out every
     * time a screen reader asks about the node.
     */
    private final SparseArray<AnAccessibility.State> accessibility = new SparseArray<>();
    /** Declared dialogs, with whatever has been set on them. */
    private final SparseArray<AlertState> alerts = new SparseArray<>();
    private final java.util.List<Integer> dirtyAlerts = new java.util.ArrayList<>();

    /** What an `Alert` carries while it is not presented. */
    private static final class AlertState {
        boolean sheet;
        String title = "";
        String message = "";
        String[] buttons = new String[0];
        boolean visible;
        androidx.appcompat.app.AlertDialog presented;
    }

    private AnRuntime runtime;
    /**
     * Default font size, in dp. It has to be the same as `FontSpec::default()`'s
     * in the core: it is the one text is measured with.
     */
    private static final float DEFAULT_FONT_SIZE = 14f;

    /**
     * The dialog theme: no frame and no background, because the content is drawn
     * by the view tree and the system frame would show over it.
     */
    private static final int ANDROID_DIALOG_THEME = android.R.style.Theme_Translucent_NoTitleBar;

    /** Each button's variant and colour, which arrive in separate props. */
    private final SparseArray<String> buttonVariants = new SparseArray<>();

    private final SparseArray<Integer> buttonColors = new SparseArray<>();

    /**
     * The switch track's colours: the on one arrives through `[color]` and the
     * off one through `[android]`, and `ColorStateList` wants them together.
     */
    private final SparseArray<int[]> switchTracks = new SparseArray<>();

    /** The step between each slider's values, if one was asked for. */
    private final SparseArray<Float> sliderSteps = new SparseArray<>();

    /** Each map's centre: the latitude and the longitude arrive separately. */
    private final java.util.HashMap<Integer, float[]> mapCenters = new java.util.HashMap<>();

    /** Each `<Modal>`'s presentation state. */
    private final SparseArray<ModalState> modals = new SparseArray<>();

    private final java.util.ArrayList<Integer> dirtyModals = new java.util.ArrayList<>();

    /** The animation settings of each view that asked for them. */
    private final android.util.SparseArray<Animation> animations = new android.util.SparseArray<>();

    /** The frame animations under way, so they can be cancelled. */
    private final java.util.HashMap<View, android.animation.ValueAnimator> frameAnimations =
            new java.util.HashMap<>();

    /** The active gestures of each view, one per view that has any. */
    private final java.util.HashMap<Integer, Gestures> gestures = new java.util.HashMap<>();

    /** The views that listen to the crown, one per view that asks for it. */
    private final SparseArray<Crown> crowns = new SparseArray<>();

    /**
     * How much silence counts as "it has stopped turning".
     *
     * The crown sends no end event: it sends detents and goes quiet. Two hundred
     * and fifty milliseconds is long for a continuous turn —the emulator sends a
     * full revolution's detents in under a hundred— and short enough that
     * `(crownIdle)` does not seem to arrive late.
     */
    private static final long CROWN_IDLE_MS = 250;

    /**
     * Whether this is a Wear OS watch.
     *
     * The system is asked and not the manifest: the watch APK declares
     * `android.hardware.type.watch`, but the phone one can be installed on a
     * watch —it is the first thing anybody tries— and then the manifest lies and
     * the system does not.
     */
    private final boolean watch;

    /**
     * Whether the screen is round.
     *
     * It is not the same as being a watch: there are square ones, and a
     * television or a car might not be. What matters for the layout is the
     * shape.
     */
    private final boolean round;

    /**
     * How far one has to stay from the edge of a round screen to fit inside the
     * largest square that fits within the circle.
     *
     * It comes from geometry and not from a preference: the side of the
     * inscribed square is `d/√2`, so `d·(1 - 1/√2)` is left over, shared between
     * the two sides. It is the same figure androidx.wear's `BoxInsetLayout` uses,
     * which is the container Google provides for this; it cannot be used as it
     * comes here because it would lay the children out on its own and the layout
     * belongs to taffy.
     *
     * No API says that the corner of a round screen does not exist: the system
     * says the screen is round —`isScreenRound`— and the inset is deduced.
     */
    private static final float ROUND_INSET = (float) ((1 - 1 / Math.sqrt(2)) / 2);

    public AnHost(Context context, AnViewGroup container) {
        this.context = context;
        this.container = container;
        this.density = context.getResources().getDisplayMetrics().density;
        this.watch =
                context.getPackageManager()
                        .hasSystemFeature(android.content.pm.PackageManager.FEATURE_WATCH);
        this.round = context.getResources().getConfiguration().isScreenRound();
    }

    /** The Activity consults it to decide things of its own. */
    public boolean isWatch() {
        return watch;
    }

    public void attachRuntime(AnRuntime runtime) {
        this.runtime = runtime;
    }

    private int px(float dp) {
        return Math.round(dp * density);
    }

    // ------------------------------------------------------------- structure

    /**
     * The primitives that are not mounted on a watch, and why.
     *
     * It is not a list of "this is not there yet": it is a list of things that
     * on Wear OS either do not exist or, existing, do not fit. An `an-tab-bar`
     * can be built with Material on a round 227 point screen —it would come
     * out—, but it comes out badly: the tabs eat half the height and the labels
     * are cut off. Letting it come out badly is exactly what is not wanted.
     *
     * Returns the reason, or `null` if the primitive does belong.
     */
    private static String notOnTheWatch(int kind) {
        switch (kind) {
            case KIND_TABBAR:
                return "Wear OS has no tab bar: you navigate by swiping and with"
                        + " the crown, not with tabs at the bottom";
            case KIND_SEGMENTED:
                return "a segmented control does not fit across a watch face";
            case KIND_NAV:
                return "a watch has no navigation bar: the top belongs to the"
                        + " system clock, and \"back\" is the swipe from the edge";
            case KIND_SEARCH:
                return "on the watch, searching is not a field inside the screen"
                        + " but the system's own input screen —dictation,"
                        + " scribble or keyboard—";
            case KIND_WEB:
                return "Wear OS ships no WebView: there is no package in the"
                        + " system that implements android.webkit";
            case KIND_DATE:
                return "the platform date picker is a phone calendar; on the"
                        + " watch the date is chosen full screen";
            case KIND_SELECT:
                return "a dropdown opens an anchored menu, and on a watch face"
                        + " there is nowhere to anchor it";
            default:
                return null;
        }
    }

    /**
     * The tag each of the ones that do not belong is written with.
     *
     * The tag is returned —`an-tab-bar`— and not the core name —`TabBar`—
     * because whoever reads the error wrote the tag, and that is what they will
     * look for in their template.
     */
    private static String kindName(int kind) {
        switch (kind) {
            case KIND_TABBAR:
                return "an-tab-bar";
            case KIND_SEGMENTED:
                return "an-segmented-control";
            case KIND_NAV:
                return "an-navigation-bar";
            case KIND_SEARCH:
                return "an-search-bar";
            case KIND_WEB:
                return "an-web-view";
            case KIND_DATE:
                return "an-date-picker";
            case KIND_SELECT:
                return "an-select";
            default:
                return "kind " + kind;
        }
    }

    /**
     * The nodes that came out as a "this does not belong here" marker.
     *
     * Without this list the marker cannot be read: it is a `TextView`, so the
     * `[color]` and the `[title]` of the primitive it stands in for are applied
     * to it just the same and it ends up painting the label of the control that
     * does not exist, in its colour, as if it worked. Exactly the opposite of
     * what is needed.
     */
    private final java.util.Set<Integer> unsupported = new java.util.HashSet<>();

    /**
     * What is mounted in place of a primitive that does not belong.
     *
     * An empty gap would make the failure look like a layout error, which is the
     * last thing anybody looks at. The name is painted so it can be seen where
     * it is and the reason is written to the log so it can be read in full. It is
     * the same treatment the watchOS host gives it.
     */
    private View unsupportedMarker(String tag, String reason) {
        android.util.Log.e("angular-native", tag + " is not mounted on the watch: " + reason);
        TextView marker = new TextView(context);
        marker.setText(tag);
        marker.setTextSize(TypedValue.COMPLEX_UNIT_DIP, 11);
        marker.setTextColor(0xFFFF6B6B);
        marker.setGravity(Gravity.CENTER);
        marker.setIncludeFontPadding(false);
        marker.setBackgroundColor(0x33FF6B6B);
        return marker;
    }

    public void createView(int id, int kind) {
        View view;
        if (watch) {
            String reason = notOnTheWatch(kind);
            if (reason != null) {
                view = unsupportedMarker(kindName(kind), reason);
                view.setLayoutParams(new AnViewGroup.Frame());
                views.put(id, view);
                unsupported.add(id);
                return;
            }
        }
        switch (kind) {
            case KIND_TEXT: {
                TextView text = new TextView(context);
                text.setIncludeFontPadding(false);
                text.setPadding(0, 0, 0, 0);
                // The same measurement the core measured with. A TextView's
                // default size depends on the theme, and if it does not match the
                // layout's the text spills out of its box and is clipped by the
                // parent, with no error at all.
                text.setTextSize(
                        android.util.TypedValue.COMPLEX_UNIT_DIP, DEFAULT_FONT_SIZE);
                view = text;
                break;
            }
            case KIND_IMAGE:
                view = new ImageView(context);
                break;
            case KIND_ICON: {
                view = newIconView();
                break;
            }
            case KIND_SEGMENTED: {
                AnSegmentedControl segments = new AnSegmentedControl(context);
                segments.setListener(index -> dispatchIndex(id, index));
                view = segments;
                break;
            }
            case KIND_STEPPER: {
                AnStepper stepper = new AnStepper(context);
                stepper.setListener(value -> dispatchValue(id, value));
                view = stepper;
                break;
            }
            case KIND_SEARCH: {
                // `SearchView` is the platform's search field: it brings its
                // magnifier, its clear button and the keyboard with the search
                // key. An `EditText` with an icon beside it is not the same.
                android.widget.SearchView search = new android.widget.SearchView(context);
                search.setIconifiedByDefault(false);
                search.setOnQueryTextListener(
                        new android.widget.SearchView.OnQueryTextListener() {
                            @Override
                            public boolean onQueryTextSubmit(String query) {
                                dispatchText(id, "submit", query);
                                return true;
                            }

                            @Override
                            public boolean onQueryTextChange(String text) {
                                dispatchText(id, "input", text);
                                return true;
                            }
                        });
                view = search;
                break;
            }
            case KIND_SELECT: {
                android.widget.Spinner spinner = new android.widget.Spinner(context);
                spinner.setOnItemSelectedListener(
                        new android.widget.AdapterView.OnItemSelectedListener() {
                            @Override
                            public void onItemSelected(
                                    android.widget.AdapterView<?> parent,
                                    View selected,
                                    int position,
                                    long rowId) {
                                dispatchIndex(id, position);
                            }

                            @Override
                            public void onNothingSelected(android.widget.AdapterView<?> parent) {}
                        });
                view = spinner;
                break;
            }
            case KIND_NAV: {
                android.widget.Toolbar toolbar = new android.widget.Toolbar(context);
                toolbar.setNavigationOnClickListener(
                        v -> {
                            if (runtime != null) {
                                runtime.dispatchEvent(id, "back", 0f, 0f);
                            }
                        });
                view = toolbar;
                break;
            }
            case KIND_TEXT_AREA: {
                EditText area = new EditText(context);
                // Several lines and no background of its own: the frame is set
                // by the template, just as in the single-line field.
                area.setInputType(
                        android.text.InputType.TYPE_CLASS_TEXT
                                | android.text.InputType.TYPE_TEXT_FLAG_MULTI_LINE);
                area.setGravity(Gravity.TOP | Gravity.START);
                area.setBackground(null);
                area.setPadding(0, 0, 0, 0);
                view = area;
                break;
            }
            case KIND_MAP:
                view = new AnMapView(context);
                break;
            case KIND_VIDEO: {
                // `VideoView` is in the platform, with its controls and its
                // audio focus handling.
                android.widget.VideoView video = new android.widget.VideoView(context);
                video.setOnPreparedListener(player -> player.setLooping(true));
                view = video;
                break;
            }
            case KIND_WEB: {
                android.webkit.WebView web = new android.webkit.WebView(context);
                web.getSettings().setJavaScriptEnabled(true);
                // Without this the links open in the system browser and the
                // view is left blank.
                web.setWebViewClient(new android.webkit.WebViewClient());
                view = web;
                break;
            }
            case KIND_DATE: {
                AnDateField date = new AnDateField(context);
                date.setListener(millis -> dispatchValue(id, millis));
                view = date;
                break;
            }
            case KIND_SCROLL: {
                AnScrollView scroll = new AnScrollView(context);
                // The crown is the normal way of going through a list on the
                // watch; a finger covers the screen being looked at.
                if (watch) {
                    scroll.enableRotary();
                }
                AnViewGroup content = new AnViewGroup(context);
                // The content is measured by the ScrollView, not by us, and a
                // ScrollView is a FrameLayout underneath: it demands its own
                // LayoutParams on the child.
                scroll.addView(
                        content,
                        new FrameLayout.LayoutParams(
                                FrameLayout.LayoutParams.MATCH_PARENT,
                                FrameLayout.LayoutParams.WRAP_CONTENT));
                scrollContent.put(id, content);
                view = scroll;
                break;
            }
            case KIND_INPUT: {
                EditText input = new EditText(context);
                input.setPadding(0, 0, 0, 0);
                input.setBackground(null);
                view = input;
                break;
            }
            case KIND_TABBAR: {
                AnTabBar tabBar = new AnTabBar(context);
                tabBar.setIconResolver(this::iconDrawableFor);
                view = tabBar;
                break;
            }
            case KIND_SWITCH:
                // The Material 3 one, with the thumb that grows and its mark
                // when switched on. `android.widget.Switch` is the framework's
                // and stopped at the look of years ago.
                view = new com.google.android.material.materialswitch.MaterialSwitch(context);
                break;
            case KIND_SLIDER: {
                // The Material 3 slider: a thick track, a bar-shaped thumb and
                // the value label while dragging. It works in floats, so the
                // integer scale `SeekBar` demanded is not needed.
                com.google.android.material.slider.Slider slider =
                        new com.google.android.material.slider.Slider(context);
                slider.setValueFrom(0f);
                slider.setValueTo(1f);
                view = slider;
                break;
            }
            case KIND_SPINNER: {
                com.google.android.material.progressindicator.CircularProgressIndicator spinner =
                        new com.google.android.material.progressindicator.CircularProgressIndicator(
                                context);
                spinner.setIndeterminate(true);
                view = spinner;
                break;
            }
            case KIND_PROGRESS: {
                // The Material 3 progress bar: rounded ends and the gap between
                // what is done and what is left.
                com.google.android.material.progressindicator.LinearProgressIndicator bar =
                        new com.google.android.material.progressindicator.LinearProgressIndicator(
                                context);
                bar.setIndeterminate(false);
                bar.setMax(SLIDER_STEPS);
                view = bar;
                break;
            }
            case KIND_BUTTON: {
                // `MaterialButton` in its text variant, which is what a
                // `UIButton` does on iOS: that way `<Button>` means the same
                // thing on both platforms. `[variant]` asks for the fill, and
                // then the pill is put there by Material, not by us.
                com.google.android.material.button.MaterialButton button =
                        new com.google.android.material.button.MaterialButton(
                                context,
                                null,
                                com.google.android.material.R.attr.materialButtonOutlinedStyle);
                button.setAllCaps(false);
                button.setStrokeWidth(0);
                // A button label does not wrap: if it does not fit, it is
                // truncated. That is what UIKit does, and a button with a word
                // broken in half looks broken.
                button.setMaxLines(1);
                button.setEllipsize(android.text.TextUtils.TruncateAt.END);
                view = button;
                break;
            }
            case KIND_MODAL: {
                AnViewGroup overlay = new AnViewGroup(context);
                overlay.setVisibility(View.GONE);
                modals.put(id, new ModalState());
                view = overlay;
                break;
            }
            case KIND_ALERT: {
                // A dialog has no view of its own: the system presents it. An
                // empty one is mounted so the tree has somewhere to hang it.
                View placeholder = new View(context);
                placeholder.setVisibility(View.GONE);
                alerts.put(id, new AlertState());
                view = placeholder;
                break;
            }
            case KIND_STACK: {
                AnViewGroup stack = new AnViewGroup(context);
                // The screens coming in and going out go outside the frame.
                stack.setClipChildren(true);
                view = stack;
                break;
            }
            case KIND_VIEW:
            default:
                view = new AnViewGroup(context);
                break;
        }
        view.setLayoutParams(new AnViewGroup.Frame());
        views.put(id, view);
    }

    public void destroyView(int id) {
        View view = views.get(id);
        // A screen on its way out stays on screen until the animation ends:
        // removing it now would give a jump.
        if (view != null && !animatingOut.contains(id) && view.getParent() instanceof ViewGroup) {
            ((ViewGroup) view.getParent()).removeView(view);
        }
        views.remove(id);
        unsupported.remove(id);
        accessibility.remove(id);
        animations.remove(id);
        buttonVariants.remove(id);
        buttonColors.remove(id);
        modals.remove(id);
        mapCenters.remove(id);
        gestures.remove(Integer.valueOf(id));
        Crown crown = crowns.get(id);
        if (crown != null) {
            // The "it has stopped" callback is queued on the view: without
            // removing it, it fires on a node that no longer exists.
            crown.detach();
            crowns.remove(id);
        }
        scrollContent.remove(id);
        watchers.remove(id);
        transitions.remove(id);
        backListeners.remove(Integer.valueOf(id));
        corners.remove(id);
        fontState.remove(id);
        inputState.remove(id);
        borderWidths.remove(id);
        borderColors.remove(id);
        sliderRanges.remove(id);
        sliderValues.remove(id);
        sliderSteps.remove(id);
        switchTracks.remove(id);
        safeArea.remove(id);
        AlertState alert = alerts.get(id);
        if (alert != null && alert.presented != null) {
            alert.presented.dismiss();
        }
        alerts.remove(id);
    }

    public void insertView(int parentId, int childId, int index) {
        View child = views.get(childId);
        ViewGroup parent = parentFor(parentId);
        if (child == null || parent == null) {
            return;
        }
        if (isStack(parentId)) {
            // A screen coming in goes above the one going out, even if the
            // tree puts it before.
            parent.addView(child);
            entering.add(new int[] {parentId, childId});
            return;
        }
        parent.addView(child, Math.min(index, parent.getChildCount()));
    }

    public void removeView(int parentId, int childId) {
        View child = views.get(childId);
        ViewGroup parent = parentFor(parentId);
        if (child == null || parent == null) {
            return;
        }
        if (isStack(parentId) && "pop".equals(transitions.get(parentId))) {
            // It stays mounted until it has finished going out.
            animatingOut.add(childId);
            leaving.add(new Object[] {parentId, child});
            return;
        }
        parent.removeView(child);
    }

    /** A ScrollView takes no loose children: they go to its inner container. */
    private ViewGroup parentFor(int id) {
        AnViewGroup content = scrollContent.get(id);
        if (content != null) {
            return content;
        }
        View view = views.get(id);
        return view instanceof ViewGroup ? (ViewGroup) view : null;
    }

    public void setRoot(int id) {
        View view = views.get(id);
        if (view == null || view.getParent() != null) {
            return;
        }
        container.addView(view);
    }

    public void clearAll() {
        container.removeAllViews();
        for (int i = 0; i < crowns.size(); i++) {
            crowns.valueAt(i).detach();
        }
        crowns.clear();
        views.clear();
        scrollContent.clear();
        unsupported.clear();
    }

    // ----------------------------------------------------------------- layout

    public void setLayout(int id, float x, float y, float width, float height) {
        View view = views.get(id);
        if (view == null) {
            return;
        }
        ViewGroup.LayoutParams params = view.getLayoutParams();
        if (!(params instanceof AnViewGroup.Frame)) {
            return;
        }
        AnViewGroup.Frame frame = (AnViewGroup.Frame) params;
        Animation anim = animations.get(id);
        if (anim != null && anim.duration > 0 && frame.width > 0) {
            animateFrame(view, frame, px(x), px(y), px(width), px(height), anim);
        } else {
            frame.left = px(x);
            frame.top = px(y);
            frame.width = px(width);
            frame.height = px(height);
            view.setLayoutParams(frame);
            view.requestLayout();
        }
        if (safeArea.indexOfKey(id) >= 0) {
            reportSafeArea(id);
        }
    }

    public void setContentSize(int id, float width, float height) {
        AnViewGroup content = scrollContent.get(id);
        if (content == null) {
            return;
        }
        FrameLayout.LayoutParams params =
                new FrameLayout.LayoutParams(px(width), px(height));
        content.setLayoutParams(params);
        content.requestLayout();
    }

    /** Called once per frame, once every op has been applied. */
    public void flush() {
        container.requestLayout();
        runStackAnimations();
        syncAlerts();
        syncModals();
    }

    private void markModalDirty(int id) {
        if (!dirtyModals.contains(id)) {
            dirtyModals.add(id);
        }
    }

    /**
     * Presents or withdraws the `<Modal>`s that changed.
     *
     * A real `Dialog`, not a view on top of the others. The difference is not
     * how it looks —that was the same— but that the system knows there is
     * something modal in front: the back button closes it, TalkBack stops
     * reading what is behind, and it does not compete in draw order with the
     * system's own dialogs.
     *
     * It goes after the layout: the dialog takes the view as it stands, and with
     * no frame computed it would present an empty box.
     */
    private void syncModals() {
        if (dirtyModals.isEmpty()) {
            return;
        }
        for (int id : dirtyModals) {
            ModalState state = modals.get(id);
            View content = views.get(id);
            if (state == null || content == null) {
                continue;
            }
            if (!state.visible) {
                if (state.presented != null) {
                    state.presented.dismiss();
                    state.presented = null;
                }
                continue;
            }
            if (state.presented != null) {
                continue;
            }
            android.app.Dialog dialog = new android.app.Dialog(context, ANDROID_DIALOG_THEME);
            if (content.getParent() instanceof ViewGroup) {
                ((ViewGroup) content.getParent()).removeView(content);
            }
            dialog.setContentView(content);
            if (dialog.getWindow() != null) {
                android.view.Window window = dialog.getWindow();
                window.setBackgroundDrawable(
                        new android.graphics.drawable.ColorDrawable(
                                android.graphics.Color.TRANSPARENT));
                if (state.sheet) {
                    // Pinned to the bottom and across, which is what is
                    // expected of a sheet. The height comes from the content.
                    window.setGravity(android.view.Gravity.BOTTOM);
                    window.setLayout(
                            android.view.ViewGroup.LayoutParams.MATCH_PARENT,
                            android.view.ViewGroup.LayoutParams.WRAP_CONTENT);
                } else {
                    window.setLayout(
                            android.view.ViewGroup.LayoutParams.MATCH_PARENT,
                            android.view.ViewGroup.LayoutParams.MATCH_PARENT);
                }
            }
            dialog.setOnDismissListener(
                    d -> {
                        state.presented = null;
                        // Closing it from outside —the back button— also has to
                        // reach the template, or the signal that opened it is
                        // left saying it is still open.
                        if (runtime != null) {
                            runtime.dispatchEvent(id, "dismiss", 0f, 0f);
                        }
                    });
            dialog.show();
            state.presented = dialog;
        }
        dirtyModals.clear();
    }

    /** How a `<Modal>` is presented. */
    private static final class ModalState {
        boolean visible;
        boolean sheet;
        android.app.Dialog presented;
    }

    private void markAlertDirty(int id) {
        if (!dirtyAlerts.contains(id)) {
            dirtyAlerts.add(id);
        }
    }

    /**
     * Presents or withdraws the dialogs that changed.
     *
     * It is done when closing the frame, once all their props have arrived:
     * presenting it as soon as `visible` changes would show a dialog with no
     * title.
     */
    private void syncAlerts() {
        if (dirtyAlerts.isEmpty()) {
            return;
        }
        for (int id : dirtyAlerts) {
            AlertState state = alerts.get(id);
            if (state == null) {
                continue;
            }
            if (!state.visible) {
                if (state.presented != null) {
                    state.presented.dismiss();
                    state.presented = null;
                }
                continue;
            }
            if (state.presented != null) {
                continue;
            }
            String[] buttons = state.buttons.length > 0 ? state.buttons : new String[] {"OK"};
            // The Material 3 one, not the framework's: rounded corners, buttons
            // without forced capitals and the right typography. The one in
            // `android.app` stopped at the look of ten years ago.
            com.google.android.material.dialog.MaterialAlertDialogBuilder builder =
                    new com.google.android.material.dialog.MaterialAlertDialogBuilder(context)
                            .setTitle(state.title)
                            .setCancelable(false);
            if (state.sheet) {
                // An action sheet on Android is a list of options, not buttons
                // at the foot: there is no separate control for this.
                builder.setItems(buttons, (dialog, which) -> emitAlertSelection(id, which));
                androidx.appcompat.app.AlertDialog created = builder.create();
                created.setOnDismissListener(d -> state.presented = null);
                created.show();
                state.presented = created;
                continue;
            }
            builder.setMessage(state.message);
            // Android places the buttons by role, not by order: more than three
            // would not fit, so from there on a list is used.
            if (buttons.length <= 3) {
                for (int index = 0; index < buttons.length; index++) {
                    final int position = index;
                    android.content.DialogInterface.OnClickListener listener =
                            (dialog, which) -> emitAlertSelection(id, position);
                    if (index == 0) {
                        builder.setPositiveButton(buttons[index], listener);
                    } else if (index == 1) {
                        builder.setNegativeButton(buttons[index], listener);
                    } else {
                        builder.setNeutralButton(buttons[index], listener);
                    }
                }
            } else {
                builder.setItems(buttons, (dialog, which) -> emitAlertSelection(id, which));
            }
            state.presented = builder.show();
        }
        dirtyAlerts.clear();
    }

    private void emitAlertSelection(int id, int index) {
        AlertState state = alerts.get(id);
        if (state != null) {
            state.presented = null;
        }
        if (runtime != null) {
            runtime.dispatchIndexEvent(id, "select", index);
        }
    }

    private boolean isStack(int id) {
        return views.get(id) instanceof AnViewGroup && transitions.indexOfKey(id) >= 0
                || stackIds.contains(id);
    }

    private final java.util.Set<Integer> stackIds = new java.util.HashSet<>();

    /**
     * Animates the screens that came in or went out on this frame.
     *
     * It is done here and not on insertion because until the layout has run
     * there is no width to animate: a freshly created screen measures zero.
     */
    private void runStackAnimations() {
        if (entering.isEmpty() && leaving.isEmpty()) {
            return;
        }
        for (int[] pair : entering) {
            View stack = views.get(pair[0]);
            View screen = views.get(pair[1]);
            if (stack == null || screen == null || !"push".equals(transitions.get(pair[0]))) {
                continue;
            }
            int width = stack.getWidth();
            if (width <= 0) {
                continue;
            }
            // It comes in from the right; the one underneath moves a third,
            // which is the parallax both platforms do.
            screen.setTranslationX(width);
            screen.animate().translationX(0).setDuration(TRANSITION_MS).start();
            View below = previousSibling((ViewGroup) stack, screen);
            if (below != null) {
                below.animate().translationX(-width / 3f).setDuration(TRANSITION_MS).start();
            }
        }
        entering.clear();

        for (Object[] pair : leaving) {
            int stackId = (Integer) pair[0];
            View screen = (View) pair[1];
            View stack = views.get(stackId);
            ViewGroup parent = stack instanceof ViewGroup ? (ViewGroup) stack : null;
            if (parent == null || parent.getWidth() <= 0) {
                if (parent != null) parent.removeView(screen);
                continue;
            }
            View below = previousSibling(parent, screen);
            if (below != null) {
                below.animate().translationX(0).setDuration(TRANSITION_MS).start();
            }
            screen.animate()
                    .translationX(parent.getWidth())
                    .setDuration(TRANSITION_MS)
                    // The view is removed when it ends; until then it stays mounted.
                    .withEndAction(() -> parent.removeView(screen))
                    .start();
        }
        leaving.clear();
        animatingOut.clear();
    }

    // --- the slider: Android works in integers and the framework in floats
    private final SparseArray<float[]> sliderRanges = new SparseArray<>();
    private final SparseArray<Float> sliderValues = new SparseArray<>();

    private float[] sliderRange(int id) {
        float[] range = sliderRanges.get(id);
        if (range == null) {
            range = new float[] {0f, 1f};
            sliderRanges.put(id, range);
        }
        return range;
    }

    /**
     * Sets the range and the value on the Material slider.
     *
     * All three arrive in separate props and in any order, and `Slider`
     * complains if the value falls outside the range, so they are set together
     * and in order every time.
     */
    private void applySliderValue(int id, com.google.android.material.slider.Slider slider) {
        float[] range = sliderRange(id);
        Float value = sliderValues.get(id);
        float min = range[0];
        float max = range[1] > range[0] ? range[1] : range[0] + 1f;
        slider.setValueFrom(min);
        slider.setValueTo(max);
        Float step = sliderSteps.get(id);
        float applied = 0f;
        if (step != null && step > 0f) {
            float steps = (max - min) / step;
            // Material demands that the step divide the range exactly: if not,
            // it blows up while drawing. Rather than crashing, it is said and
            // left continuous, which is what was there before.
            if (Math.abs(steps - Math.round(steps)) > 1e-4f) {
                android.util.Log.w(
                        "angular-native",
                        "[android].stepSize " + step + " does not divide the range "
                                + (max - min) + ": the slider stays continuous");
            } else {
                applied = step;
            }
        }
        slider.setStepSize(applied);
        if (value != null) {
            float clamped = Math.max(min, Math.min(max, value));
            // With a step, the value has to land on one: Material rejects any
            // other.
            if (applied > 0f) {
                clamped = min + Math.round((clamped - min) / applied) * applied;
                clamped = Math.max(min, Math.min(max, clamped));
            }
            slider.setValue(clamped);
        }
    }

    /**
     * The switch track, with its on colour and its off colour.
     *
     * The two arrive through different props, so they are kept and the whole
     * state list is built: a `ColorStateList` with a single colour paints both
     * positions the same and the switch stops saying whether it is on.
     */
    private void applySwitchTrack(int id, View view) {
        int[] colors = switchTracks.get(id);
        if (colors == null || !(view instanceof androidx.appcompat.widget.SwitchCompat)) {
            return;
        }
        ((androidx.appcompat.widget.SwitchCompat) view)
                .setTrackTintList(
                        new android.content.res.ColorStateList(
                                new int[][] {
                                    new int[] {android.R.attr.state_checked}, new int[0]
                                },
                                new int[] {colors[0], colors[1]}));
    }

    private int[] switchTrackOf(int id) {
        int[] colors = switchTracks.get(id);
        if (colors == null) {
            // With nothing said, the off one is Material's usual grey.
            colors = new int[] {Color.GRAY, Color.argb(60, 120, 120, 120)};
            switchTracks.put(id, colors);
        }
        return colors;
    }

    /** A JSON list of strings: it is how the tab titles travel. */
    private static String[] parseStringList(String raw) {
        if (raw == null || raw.length() < 2) {
            return new String[0];
        }
        java.util.List<String> out = new java.util.ArrayList<>();
        boolean inside = false;
        StringBuilder current = new StringBuilder();
        for (int i = 0; i < raw.length(); i++) {
            char ch = raw.charAt(i);
            if (ch == '"' && (i == 0 || raw.charAt(i - 1) != '\\')) {
                if (inside) {
                    out.add(current.toString());
                    current.setLength(0);
                }
                inside = !inside;
            } else if (inside) {
                current.append(ch);
            }
        }
        return out.toArray(new String[0]);
    }

    private static View previousSibling(ViewGroup parent, View view) {
        int index = parent.indexOfChild(view);
        return index > 0 ? parent.getChildAt(index - 1) : null;
    }

    /** Reports the system insets if they changed since last time. */
    private void reportSafeArea(int id) {
        float[] previous = safeArea.get(id);
        if (previous == null || runtime == null) {
            return;
        }
        android.view.WindowInsets insets = container.getRootWindowInsets();
        float top = 0, right = 0, bottom = 0, left = 0;
        if (insets != null) {
            android.graphics.Insets bars =
                    insets.getInsets(
                            android.view.WindowInsets.Type.systemBars()
                                    | android.view.WindowInsets.Type.displayCutout());
            top = bars.top / density;
            right = bars.right / density;
            bottom = bars.bottom / density;
            left = bars.left / density;
        }
        if (round) {
            // On a round screen the corners do not exist. The system does not
            // count them as insets —`WindowInsets` gives zero— because there is
            // nothing of the system's there: there is simply no screen. Whatever
            // is put in the corner does not come out clipped with a warning, it
            // comes out unpainted and with nobody saying anything, which is
            // worse.
            //
            // It goes down the same path as the notch on purpose: for the app it
            // is the same question —"how far can I paint?"— and `an-safe-area`
            // already answers it on all three platforms. A new event would be a
            // second way of knowing the same thing.
            android.util.DisplayMetrics metrics = context.getResources().getDisplayMetrics();
            float side = Math.min(metrics.widthPixels, metrics.heightPixels) / density;
            float inset = side * ROUND_INSET;
            top = Math.max(top, inset);
            right = Math.max(right, inset);
            bottom = Math.max(bottom, inset);
            left = Math.max(left, inset);
        }
        if (previous[0] == top && previous[1] == right && previous[2] == bottom && previous[3] == left) {
            return;
        }
        safeArea.put(id, new float[] {top, right, bottom, left});
        // Four numbers do not fit in a position event: they travel as JSON,
        // which is the same path the tabs use.
        runtime.dispatchValueEvent(
                id,
                "safeArea",
                "{\"top\":" + top + ",\"right\":" + right
                        + ",\"bottom\":" + bottom + ",\"left\":" + left + "}");
    }

    /** Called by the Activity when the user presses back. */
    public boolean dispatchBack() {
        if (backListeners.isEmpty() || runtime == null) {
            return false;
        }
        runtime.dispatchEvent(backListeners.get(backListeners.size() - 1), "back", 0f, 0f);
        return true;
    }

    // ------------------------------------------------------------------ props

    public void setText(int id, String text) {
        View view = views.get(id);
        if (view instanceof TextView && !unsupported.contains(id)) {
            ((TextView) view).setText(text);
        }
    }

    public void setProp(int id, String key, String value) {
        View view = views.get(id);
        // The "this does not belong here" marker keeps what is its own: if it
        // let the colour and the background of the primitive it stands in for be
        // painted over it, it would disguise itself as the primitive that is not
        // there.
        if (view == null || unsupported.contains(id)) {
            return;
        }
        switch (key) {
            case "backgroundColor":
            case "background-color":
                applyBackground(view, parseColor(value), null);
                break;
            case "borderRadius":
            case "border-radius": {
                Float radius = parseFloat(value);
                float[] all = cornersOf(id);
                java.util.Arrays.fill(all, radius == null ? 0f : radius);
                applyCorners(view, all);
                break;
            }
            case "borderTopLeftRadius":
                setCorner(view, id, 0, parseFloat(value));
                break;
            case "borderTopRightRadius":
                setCorner(view, id, 1, parseFloat(value));
                break;
            case "borderBottomRightRadius":
                setCorner(view, id, 2, parseFloat(value));
                break;
            case "borderBottomLeftRadius":
                setCorner(view, id, 3, parseFloat(value));
                break;
            case "borderWidth":
            case "border-width":
            case "borderColor":
            case "border-color":
                applyBorder(view, id, key, value);
                break;
            case "icons":
                if (view instanceof AnTabBar) {
                    ((AnTabBar) view).setIcons(parseStringList(value));
                }
                break;
            // --- segmented control, dropdown and date
            case "items":
                if (view instanceof AnSegmentedControl) {
                    ((AnSegmentedControl) view).setItems(parseStringList(value));
                } else if (view instanceof android.widget.Spinner) {
                    android.widget.ArrayAdapter<String> adapter =
                            new android.widget.ArrayAdapter<>(
                                    context,
                                    android.R.layout.simple_spinner_item,
                                    parseStringList(value));
                    adapter.setDropDownViewResource(
                            android.R.layout.simple_spinner_dropdown_item);
                    ((android.widget.Spinner) view).setAdapter(adapter);
                } else if (view instanceof AnTabBar) {
                    ((AnTabBar) view).setTitles(parseStringList(value));
                }
                break;
            case "stepValue":
                if (view instanceof AnStepper) {
                    ((AnStepper) view).setStep(number(value, 1f));
                }
                break;
            case "latitude":
            case "longitude":
                if (view instanceof AnMapView) {
                    float[] centre = mapCenters.computeIfAbsent(id, k -> new float[2]);
                    centre["latitude".equals(key) ? 0 : 1] = number(value, 0f);
                    ((AnMapView) view).setCenter(centre[0], centre[1]);
                }
                break;
            case "zoom":
                if (view instanceof AnMapView) {
                    ((AnMapView) view).setZoom(number(value, 12f));
                }
                break;
            case "showsUser":
                // The map here does not know where you are: it is not the system's.
                break;
            case "playing":
                if (view instanceof android.widget.VideoView) {
                    if ("true".equals(value)) {
                        ((android.widget.VideoView) view).start();
                    } else {
                        ((android.widget.VideoView) view).pause();
                    }
                }
                break;
            case "muted":
                // `VideoView` does not expose the volume; one would have to
                // reach the `MediaPlayer` inside, and it does not hand it over.
                break;
            case "url":
                if (view instanceof android.widget.VideoView && value != null) {
                    ((android.widget.VideoView) view).setVideoURI(android.net.Uri.parse(value));
                    break;
                }
                if (view instanceof android.webkit.WebView && value != null) {
                    ((android.webkit.WebView) view).loadUrl(value);
                }
                break;
            case "html":
                if (view instanceof android.webkit.WebView) {
                    ((android.webkit.WebView) view)
                            .loadDataWithBaseURL(
                                    null, value == null ? "" : value, "text/html", "utf-8", null);
                }
                break;
            case "backTitle":
                // Android puts no label on the back button: only the icon, which
                // is what any app on the platform does.
                break;
            case "showsBack":
                if (view instanceof android.widget.Toolbar) {
                    ((android.widget.Toolbar) view)
                            .setNavigationIcon(
                                    "true".equals(value) ? iconDrawableFor("arrow_back") : null);
                }
                break;
            case "mode":
                if (view instanceof AnDateField) {
                    ((AnDateField) view).setMode(value == null ? "date" : value);
                }
                break;
            case "variant":
                if (view instanceof android.widget.Button) {
                    applyButtonVariant((android.widget.Button) view, id, value);
                }
                break;
            case "icon":
                if (view instanceof com.google.android.material.button.MaterialButton) {
                    com.google.android.material.button.MaterialButton material =
                            (com.google.android.material.button.MaterialButton) view;
                    material.setIcon(value == null ? null : iconDrawableFor(value));
                    // The icon is tinted with the label's colour: on a button,
                    // icon and text are the same thing as far as contrast goes.
                    material.setIconTint(
                            android.content.res.ColorStateList.valueOf(
                                    material.getCurrentTextColor()));
                }
                break;
            case "iconPosition":
                if (view instanceof com.google.android.material.button.MaterialButton) {
                    ((com.google.android.material.button.MaterialButton) view)
                            .setIconGravity(
                                    "trailing".equals(value)
                                            ? com.google.android.material.button.MaterialButton
                                                    .ICON_GRAVITY_TEXT_END
                                            : com.google.android.material.button.MaterialButton
                                                    .ICON_GRAVITY_TEXT_START);
                }
                break;
            // Single-platform props. The iOS ones arrive with their prefix and
            // fall into the `default`, which is exactly what they should do here.
            case "android:rippleColor":
                if (view instanceof com.google.android.material.button.MaterialButton) {
                    Integer ripple = parseColor(value);
                    ((com.google.android.material.button.MaterialButton) view)
                            .setRippleColor(
                                    ripple == null
                                            ? null
                                            : android.content.res.ColorStateList.valueOf(ripple));
                }
                break;
            case "android:allCaps":
                if (view instanceof android.widget.Button) {
                    ((android.widget.Button) view).setAllCaps("true".equals(value));
                }
                break;
            case "enabled":
                setEnabledDeep(view, !"false".equals(value));
                break;
            case "name":
                if (view instanceof TextView && isIcon(view)) {
                    ((TextView) view).setText(iconGlyph(value));
                }
                break;
            case "iconSize":
                if (view instanceof TextView && isIcon(view)) {
                    // The glyph's size is the box's: an icon of 24 takes up 24,
                    // without the line gap a text leaves.
                    ((TextView) view)
                            .setTextSize(TypedValue.COMPLEX_UNIT_DIP, number(value, 24f));
                }
                break;
            case "iconWeight":
                if (view instanceof TextView && isIcon(view)) {
                    // Material Symbols is a variable font: the stroke weight is
                    // an axis, not another file.
                    ((TextView) view)
                            .getPaint()
                            .setFontVariationSettings(
                                    "'wght' " + Math.round(number(value, 400f)));
                    view.invalidate();
                }
                break;
            case "opacity":
                visual(view, id).alpha(number(value, 1f));
                break;
            // Animation: it is not a value that shows, it says how the ones that
            // do are reached.
            case "animate":
                animationFor(id).duration = (long) number(value, 0f);
                break;
            case "animateDelay":
                animationFor(id).delay = (long) number(value, 0f);
                break;
            case "animateEasing":
                animationFor(id).easing = value == null ? "ease-out" : value;
                break;
            // Transforms. They do not go through the layout: moving or scaling a
            // view does not change the room it takes up, so nothing has to be
            // recomputed and the finger can be followed for free.
            case "translateX":
                visual(view, id).translationX(number(value, 0f) * density);
                break;
            case "translateY":
                visual(view, id).translationY(number(value, 0f) * density);
                break;
            case "scale":
                visual(view, id).scaleX(number(value, 1f)).scaleY(number(value, 1f));
                break;
            case "scaleX":
                visual(view, id).scaleX(number(value, 1f));
                break;
            case "scaleY":
                visual(view, id).scaleY(number(value, 1f));
                break;
            case "rotate":
                // The API works in radians, like the rotation gesture; Android
                // wants degrees.
                visual(view, id).rotation((float) Math.toDegrees(number(value, 0f)));
                break;
            // --- system controls
            case "on":
                if (view instanceof androidx.appcompat.widget.SwitchCompat) {
                    ((androidx.appcompat.widget.SwitchCompat) view).setChecked("true".equals(value));
                }
                break;
            case "minimumValue":
            case "maximumValue": {
                Float bound = parseFloat(value);
                if (view instanceof AnStepper && bound != null) {
                    if ("minimumValue".equals(key)) {
                        ((AnStepper) view).setMinimum(bound);
                    } else {
                        ((AnStepper) view).setMaximum(bound);
                    }
                    break;
                }
                if (view instanceof com.google.android.material.slider.Slider && bound != null) {
                    float[] range = sliderRange(id);
                    range["minimumValue".equals(key) ? 0 : 1] = bound;
                    applySliderValue(id, (com.google.android.material.slider.Slider) view);
                }
                break;
            }
            case "thumbColor": {
                Integer thumb = parseColor(value);
                if (thumb == null) {
                    break;
                }
                if (view instanceof androidx.appcompat.widget.SwitchCompat) {
                    ((androidx.appcompat.widget.SwitchCompat) view)
                            .setThumbTintList(
                                    android.content.res.ColorStateList.valueOf(thumb));
                } else if (view instanceof com.google.android.material.slider.Slider) {
                    ((com.google.android.material.slider.Slider) view)
                            .setThumbTintList(
                                    android.content.res.ColorStateList.valueOf(thumb));
                }
                break;
            }
            case "minimumTrackColor":
            case "maximumTrackColor": {
                Integer track = parseColor(value);
                if (track == null
                        || !(view instanceof com.google.android.material.slider.Slider)) {
                    break;
                }
                com.google.android.material.slider.Slider bar =
                        (com.google.android.material.slider.Slider) view;
                android.content.res.ColorStateList tint =
                        android.content.res.ColorStateList.valueOf(track);
                if ("minimumTrackColor".equals(key)) {
                    bar.setTrackActiveTintList(tint);
                } else {
                    bar.setTrackInactiveTintList(tint);
                }
                break;
            }
            case "android:trackColor": {
                Integer off = parseColor(value);
                if (off != null) {
                    switchTrackOf(id)[1] = off;
                    applySwitchTrack(id, view);
                }
                break;
            }
            case "android:stepSize":
                if (view instanceof com.google.android.material.slider.Slider) {
                    sliderSteps.put(id, parseFloat(value));
                    applySliderValue(id, (com.google.android.material.slider.Slider) view);
                }
                break;
            case "animating":
                if (view instanceof android.widget.ProgressBar) {
                    view.setVisibility("false".equals(value) ? View.INVISIBLE : View.VISIBLE);
                }
                break;
            case "progress": {
                Float progress = parseFloat(value);
                if (view instanceof android.widget.ProgressBar && progress != null) {
                    ((android.widget.ProgressBar) view)
                            .setProgress(
                                    Math.round(Math.max(0f, Math.min(1f, progress)) * SLIDER_STEPS));
                }
                break;
            }
            case "title":
                if (view instanceof android.widget.Toolbar) {
                    ((android.widget.Toolbar) view).setTitle(value);
                    break;
                }
                if (alerts.get(id) != null) {
                    alerts.get(id).title = value == null ? "" : value;
                    markAlertDirty(id);
                    break;
                }
                if (view instanceof android.widget.Button) {
                    ((android.widget.Button) view).setText(value);
                }
                break;
            case "selectedIndex": {
                Float index = parseFloat(value);
                if (index == null) {
                    break;
                }
                if (view instanceof AnTabBar) {
                    ((AnTabBar) view).setSelectedIndex(Math.round(index));
                } else if (view instanceof AnSegmentedControl) {
                    ((AnSegmentedControl) view).setSelectedIndex(Math.round(index));
                } else if (view instanceof android.widget.Spinner) {
                    ((android.widget.Spinner) view).setSelection(Math.round(index));
                }
                break;
            }
            case "visible":
                if (alerts.get(id) != null) {
                    alerts.get(id).visible = "true".equals(value);
                    markAlertDirty(id);
                    break;
                }
                if (modals.get(id) != null) {
                    modals.get(id).visible = "true".equals(value);
                    // Hidden while it is not presented: what shows it is the
                    // dialog. Visible without being presented, it would be drawn
                    // inline over the page.
                    view.setVisibility(modals.get(id).visible ? View.VISIBLE : View.GONE);
                    markModalDirty(id);
                    break;
                }
                view.setVisibility("false".equals(value) ? View.GONE : View.VISIBLE);
                break;
            case "presentation":
                if (modals.get(id) != null) {
                    modals.get(id).sheet = "sheet".equals(value);
                    markModalDirty(id);
                }
                break;
            // --- system dialogs
            case "sheet":
                if (alerts.get(id) != null) {
                    alerts.get(id).sheet = "true".equals(value);
                    markAlertDirty(id);
                }
                break;
            case "message":
            case "buttons":
                if (alerts.get(id) != null) {
                    AlertState state = alerts.get(id);
                    if ("message".equals(key)) {
                        state.message = value == null ? "" : value;
                    } else {
                        state.buttons = parseStringList(value);
                    }
                    markAlertDirty(id);
                }
                break;
            case "transition":
                transitions.put(id, value);
                stackIds.add(id);
                break;
            case "source":
                if (view instanceof ImageView) {
                    loadImage(id, (ImageView) view, value);
                }
                break;
            case "resizeMode":
                if (view instanceof ImageView) {
                    ((ImageView) view).setScaleType(scaleTypeOf(value));
                }
                break;
            // --- accessibility
            //
            // The seven go together because they end up in the same place: the
            // node state, which is the only thing that knows whether the name
            // came from `[accessibilityLabel]` or from `[testID]`, and the only
            // thing that can decide between them without depending on which
            // one arrived first.
            case "accessibilityLabel":
            case "accessibilityHint":
            case "accessibilityRole":
            case "accessibilityValue":
            case "accessibilityState":
            case "accessible":
            case "testID": {
                AnAccessibility.State state = accessibilityOf(id);
                switch (key) {
                    case "accessibilityLabel":
                        state.label = text(value);
                        break;
                    case "accessibilityHint":
                        state.hint = text(value);
                        break;
                    case "accessibilityRole":
                        state.role = text(value);
                        break;
                    case "accessibilityValue":
                        state.value = text(value);
                        break;
                    case "accessible":
                        state.accessible = flag(value);
                        break;
                    case "testID":
                        state.testID = text(value);
                        break;
                    default:
                        readAccessibilityState(state, value);
                        break;
                }
                AnAccessibility.apply(view, state);
                break;
            }
            case "color": {
                Integer color = parseColor(value);
                if (color == null) {
                    break;
                }
                if (view instanceof AnTabBar) {
                    ((AnTabBar) view).setActiveColor(color);
                } else if (view instanceof androidx.appcompat.widget.SwitchCompat) {
                    // Only the track, and only the on one: the thumb is painted
                    // by Material to contrast with it, and tinting both the same
                    // colour left the thumb invisible.
                    switchTrackOf(id)[0] = color;
                    applySwitchTrack(id, view);
                } else if (view instanceof android.widget.ProgressBar) {
                    ((android.widget.ProgressBar) view)
                            .setProgressTintList(android.content.res.ColorStateList.valueOf(color));
                    ((android.widget.ProgressBar) view)
                            .setIndeterminateTintList(
                                    android.content.res.ColorStateList.valueOf(color));
                } else if (view instanceof com.google.android.material.slider.Slider) {
                    ((com.google.android.material.slider.Slider) view)
                            .setTrackActiveTintList(
                                    android.content.res.ColorStateList.valueOf(color));
                    ((com.google.android.material.slider.Slider) view)
                            .setThumbTintList(android.content.res.ColorStateList.valueOf(color));
                } else if (view instanceof android.widget.Button) {
                    // The colour and the variant arrive separately and in any
                    // order: both are kept and the whole button is rebuilt.
                    buttonColors.put(id, color);
                    refreshButton((android.widget.Button) view, id);
                } else if (view instanceof TextView) {
                    ((TextView) view).setTextColor(color);
                }
                break;
            }
            case "fontSize":
                if (view instanceof TextView) {
                    Float size = parseFloat(value);
                    if (size != null) {
                        ((TextView) view)
                                .setTextSize(TypedValue.COMPLEX_UNIT_PX, size * density);
                        // Letter spacing goes in ems: changing the size changes
                        // what an em is worth.
                        applyTextMetrics(id, (TextView) view);
                    }
                }
                break;
            case "fontWeight":
                if (view instanceof TextView) {
                    fontStateOf(id).bold = "bold".equals(value) || weightOf(value) >= 600;
                    applyTypeface(id, (TextView) view);
                }
                break;
            case "fontStyle":
                if (view instanceof TextView) {
                    fontStateOf(id).italic = "italic".equals(value);
                    applyTypeface(id, (TextView) view);
                }
                break;
            case "fontFamily":
                if (view instanceof TextView) {
                    fontStateOf(id).family = value;
                    applyTypeface(id, (TextView) view);
                }
                break;
            case "letterSpacing":
                if (view instanceof TextView) {
                    fontStateOf(id).letterSpacing = parseFloat(value);
                    applyTextMetrics(id, (TextView) view);
                }
                break;
            case "lineHeight":
                if (view instanceof TextView) {
                    fontStateOf(id).lineHeight = parseFloat(value);
                    applyTextMetrics(id, (TextView) view);
                }
                break;
            case "textAlign":
                if (view instanceof TextView) {
                    ((TextView) view).setGravity(gravityOf(value));
                }
                break;
            case "numberOfLines":
                if (view instanceof TextView) {
                    Float lines = parseFloat(value);
                    TextView text = (TextView) view;
                    if (lines == null || lines < 1) {
                        text.setMaxLines(Integer.MAX_VALUE);
                    } else {
                        text.setMaxLines(Math.round(lines));
                        text.setEllipsize(TextUtils.TruncateAt.END);
                    }
                }
                break;
            case "placeholder":
                if (view instanceof android.widget.SearchView) {
                    ((android.widget.SearchView) view).setQueryHint(value);
                    break;
                }
                if (view instanceof EditText) {
                    ((EditText) view).setHint(value);
                }
                break;
            case "value": {
                Float sliderValue = parseFloat(value);
                if (view instanceof AnStepper && sliderValue != null) {
                    ((AnStepper) view).setValue(sliderValue);
                    break;
                }
                if (view instanceof AnDateField && sliderValue != null) {
                    ((AnDateField) view).setMillis((long) (double) sliderValue);
                    break;
                }
                if (view instanceof android.widget.SearchView) {
                    ((android.widget.SearchView) view).setQuery(value == null ? "" : value, false);
                    break;
                }
                if (view instanceof com.google.android.material.slider.Slider && sliderValue != null) {
                    sliderValues.put(id, sliderValue);
                    applySliderValue(id, (com.google.android.material.slider.Slider) view);
                    break;
                }
                if (view instanceof EditText) {
                    EditText input = (EditText) view;
                    // Writing on every keystroke would move the cursor to the end.
                    if (!input.getText().toString().equals(value)) {
                        input.setText(value);
                    }
                }
                break;
            }
            case "editable":
                if (view instanceof EditText) {
                    ((EditText) view).setEnabled(!"false".equals(value));
                }
                break;
            case "secureTextEntry":
            case "keyboardType":
            case "autoCapitalize":
            case "autoCorrect":
                if (view instanceof EditText) {
                    InputState state = inputStateOf(id);
                    switch (key) {
                        case "secureTextEntry":
                            state.secure = "true".equals(value);
                            break;
                        case "keyboardType":
                            state.keyboard = value == null ? "default" : value;
                            break;
                        case "autoCapitalize":
                            state.capitalize = value == null ? "sentences" : value;
                            break;
                        default:
                            state.correct = !"false".equals(value);
                            break;
                    }
                    applyInputType(id, (EditText) view);
                }
                break;
            case "returnKeyType":
                if (view instanceof EditText) {
                    int action;
                    switch (value == null ? "default" : value) {
                        case "done":
                            action = android.view.inputmethod.EditorInfo.IME_ACTION_DONE;
                            break;
                        case "go":
                            action = android.view.inputmethod.EditorInfo.IME_ACTION_GO;
                            break;
                        case "next":
                            action = android.view.inputmethod.EditorInfo.IME_ACTION_NEXT;
                            break;
                        case "search":
                            action = android.view.inputmethod.EditorInfo.IME_ACTION_SEARCH;
                            break;
                        case "send":
                            action = android.view.inputmethod.EditorInfo.IME_ACTION_SEND;
                            break;
                        default:
                            action = android.view.inputmethod.EditorInfo.IME_ACTION_UNSPECIFIED;
                            break;
                    }
                    ((EditText) view).setImeOptions(action);
                }
                break;
            case "placeholderColor":
                if (view instanceof TextView) {
                    Integer hint = parseColor(value);
                    if (hint != null) {
                        ((TextView) view).setHintTextColor(hint);
                    }
                }
                break;
            case "android:selectAllOnFocus":
                if (view instanceof EditText) {
                    ((EditText) view).setSelectAllOnFocus("true".equals(value));
                }
                break;
            case "android:cursorVisible":
                if (view instanceof EditText) {
                    ((EditText) view).setCursorVisible(!"false".equals(value));
                }
                break;
            case "textDecoration":
                if (view instanceof TextView) {
                    android.graphics.Paint paint = ((TextView) view).getPaint();
                    paint.setUnderlineText("underline".equals(value));
                    paint.setStrikeThruText("lineThrough".equals(value));
                    view.invalidate();
                }
                break;
            case "android:selectable":
                if (view instanceof TextView) {
                    ((TextView) view).setTextIsSelectable("true".equals(value));
                }
                break;
            case "unselectedColor": {
                Integer inactive = parseColor(value);
                if (inactive != null && view instanceof AnTabBar) {
                    ((AnTabBar) view).setInactiveColor(inactive);
                }
                break;
            }
            case "scrollEnabled":
                if (view instanceof AnScrollView) {
                    ((AnScrollView) view).setScrollEnabled(!"false".equals(value));
                }
                break;
            case "showsScrollIndicator":
                if (view instanceof ScrollView) {
                    boolean shown = !"false".equals(value);
                    view.setVerticalScrollBarEnabled(shown);
                    view.setHorizontalScrollBarEnabled(shown);
                }
                break;
            case "bounces":
                if (view instanceof ScrollView) {
                    // The iOS bounce here is the stretch at the end of the
                    // scroll: the same point in the gesture, drawn the way each
                    // platform draws it.
                    view.setOverScrollMode(
                            "false".equals(value)
                                    ? View.OVER_SCROLL_NEVER
                                    : View.OVER_SCROLL_IF_CONTENT_SCROLLS);
                }
                break;
            case "refreshing":
                if (view instanceof AnScrollView) {
                    ((AnScrollView) view).setRefreshing("true".equals(value));
                }
                break;
            default:
                break;
        }
    }

    private AnAccessibility.State accessibilityOf(int id) {
        AnAccessibility.State state = accessibility.get(id);
        if (state == null) {
            state = new AnAccessibility.State();
            accessibility.put(id, state);
        }
        return state;
    }

    /**
     * A text prop the template may have taken away.
     *
     * An input that goes back to `null` arrives here as an empty string — that
     * is what `set_prop` writes in Rust for `PropValue::Null` — and for
     * accessibility the difference matters: an empty label is not a label, it
     * is having no label, and the node has to go back to announcing itself the
     * way it did before anyone said anything.
     */
    private static String text(String value) {
        return value == null || value.isEmpty() ? null : value;
    }

    private static Boolean flag(String value) {
        if (value == null || value.isEmpty()) {
            return null;
        }
        return "true".equals(value);
    }

    /**
     * `accessibilityState` travels as a JSON object, just like the labels of
     * an `an-alert`.
     *
     * It is emptied out before being read because the template sends the whole
     * object: a key that is no longer there means nothing is being said about
     * it any more, and leaving the previous one in place would announce a
     * state the template withdrew.
     */
    private static void readAccessibilityState(AnAccessibility.State state, String value) {
        state.disabled = null;
        state.selected = null;
        state.checked = null;
        state.expanded = null;
        state.busy = null;
        if (value == null || value.isEmpty()) {
            return;
        }
        org.json.JSONObject json;
        try {
            json = new org.json.JSONObject(value);
        } catch (org.json.JSONException error) {
            android.util.Log.e(
                    "angular-native",
                    "accessibilityState is not an object: "
                            + value
                            + " ("
                            + error.getMessage()
                            + ")");
            return;
        }
        if (json.has("disabled")) {
            state.disabled = json.optBoolean("disabled");
        }
        if (json.has("selected")) {
            state.selected = json.optBoolean("selected");
        }
        if (json.has("expanded")) {
            state.expanded = json.optBoolean("expanded");
        }
        if (json.has("busy")) {
            state.busy = json.optBoolean("busy");
        }
        if (json.has("checked")) {
            Object checked = json.opt("checked");
            if (checked instanceof Boolean) {
                state.checked =
                        ((Boolean) checked) ? AnAccessibility.CHECKED : AnAccessibility.UNCHECKED;
            } else if ("mixed".equals(checked)) {
                state.checked = AnAccessibility.MIXED;
            } else {
                android.util.Log.e(
                        "angular-native",
                        "accessibilityState.checked only takes true, false or \"mixed\";"
                                + " got " + checked);
            }
        }
    }

    private GradientDrawable backgroundOf(View view) {
        if (view.getBackground() instanceof GradientDrawable) {
            return (GradientDrawable) view.getBackground();
        }
        GradientDrawable drawable = new GradientDrawable();
        view.setBackground(drawable);
        return drawable;
    }

    private void applyBackground(View view, Integer color, Float unused) {
        if (color != null) {
            backgroundOf(view).setColor(color);
        }
    }

    private float[] cornersOf(int id) {
        float[] radii = corners.get(id);
        if (radii == null) {
            radii = new float[4];
            corners.put(id, radii);
        }
        return radii;
    }

    private void setCorner(View view, int id, int corner, Float radius) {
        float[] radii = cornersOf(id);
        radii[corner] = radius == null ? 0f : radius;
        applyCorners(view, radii);
    }

    /**
     * `setCornerRadii` wants eight values —X and Y radius of each corner— in the
     * order top-left, top-right, bottom-right, bottom-left.
     */
    private void applyCorners(View view, float[] radii) {
        GradientDrawable drawable = backgroundOf(view);
        boolean uniform = radii[0] == radii[1] && radii[1] == radii[2] && radii[2] == radii[3];
        if (uniform) {
            drawable.setCornerRadius(radii[0] * density);
            return;
        }
        drawable.setCornerRadii(
                new float[] {
                    radii[0] * density, radii[0] * density,
                    radii[1] * density, radii[1] * density,
                    radii[2] * density, radii[2] * density,
                    radii[3] * density, radii[3] * density
                });
    }

    /** The border is two props that arrive separately and are applied together. */
    private final SparseArray<float[]> borderWidths = new SparseArray<>();
    private final SparseArray<Integer> borderColors = new SparseArray<>();

    private void applyBorder(View view, int id, String key, String value) {
        if (key.startsWith("borderWidth") || key.startsWith("border-width")) {
            Float width = parseFloat(value);
            borderWidths.put(id, new float[] {width == null ? 0f : width});
        } else {
            Integer color = parseColor(value);
            if (color != null) {
                borderColors.put(id, color);
            }
        }
        float[] width = borderWidths.get(id);
        Integer color = borderColors.get(id);
        if (width == null) {
            return;
        }
        backgroundOf(view)
                .setStroke(Math.round(width[0] * density), color == null ? 0 : color);
    }

    /**
     * Disables a control and everything it carries inside.
     *
     * `setEnabled` on a `ViewGroup` does not reach the children, and three of
     * the controls here —the stepper, the segmented control and the tab bar— are
     * not in the platform and are groups of our own views. Without going down
     * the tree, disabling them left them responding to touch.
     */
    private void setEnabledDeep(View view, boolean enabled) {
        view.setEnabled(enabled);
        if (view instanceof ViewGroup) {
            ViewGroup group = (ViewGroup) view;
            for (int i = 0; i < group.getChildCount(); i++) {
                setEnabledDeep(group.getChildAt(i), enabled);
            }
        }
    }

    private InputState inputStateOf(int id) {
        InputState state = inputState.get(id);
        if (state == null) {
            state = new InputState();
            inputState.put(id, state);
        }
        return state;
    }

    /**
     * Composes the whole `inputType` from what is known about the field.
     *
     * Which keyboard comes up, the automatic capitals, the autocorrect and
     * whether the text shows or is masked are all flags of the same integer:
     * applying one alone would wipe out the other three.
     */
    private void applyInputType(int id, EditText input) {
        InputState state = inputStateOf(id);
        int type;
        switch (state.keyboard) {
            case "numeric":
                type = android.text.InputType.TYPE_CLASS_NUMBER;
                break;
            case "decimal":
                type = android.text.InputType.TYPE_CLASS_NUMBER
                        | android.text.InputType.TYPE_NUMBER_FLAG_DECIMAL;
                break;
            case "phone":
                type = android.text.InputType.TYPE_CLASS_PHONE;
                break;
            case "email":
                type = android.text.InputType.TYPE_CLASS_TEXT
                        | android.text.InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS;
                break;
            case "url":
                type = android.text.InputType.TYPE_CLASS_TEXT
                        | android.text.InputType.TYPE_TEXT_VARIATION_URI;
                break;
            default:
                type = android.text.InputType.TYPE_CLASS_TEXT;
                break;
        }
        boolean numeric = (type & android.text.InputType.TYPE_CLASS_NUMBER) != 0;
        if (state.secure) {
            // The password wins over the variation: a masked field with an
            // email keyboard would show the text.
            type = numeric
                    ? android.text.InputType.TYPE_CLASS_NUMBER
                            | android.text.InputType.TYPE_NUMBER_VARIATION_PASSWORD
                    : android.text.InputType.TYPE_CLASS_TEXT
                            | android.text.InputType.TYPE_TEXT_VARIATION_PASSWORD;
        } else if (!numeric) {
            // Capitals and autocorrect only exist on the text keyboard; on the
            // numeric one there is nothing to capitalise.
            switch (state.capitalize) {
                case "none":
                    break;
                case "words":
                    type |= android.text.InputType.TYPE_TEXT_FLAG_CAP_WORDS;
                    break;
                case "characters":
                    type |= android.text.InputType.TYPE_TEXT_FLAG_CAP_CHARACTERS;
                    break;
                default:
                    type |= android.text.InputType.TYPE_TEXT_FLAG_CAP_SENTENCES;
                    break;
            }
            if (!state.correct) {
                type |= android.text.InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS;
            }
        }
        input.setInputType(type);
        // Changing the type sends the field back to the monospaced password
        // face: its own has to be put back.
        applyTypeface(id, input);
    }

    private FontState fontStateOf(int id) {
        FontState state = fontState.get(id);
        if (state == null) {
            state = new FontState();
            fontState.put(id, state);
        }
        return state;
    }

    /**
     * Family, italic and bold go together or they do not go at all.
     *
     * `setTypeface(null, style)` keeps the family and `Typeface.create` asks for
     * the style, so applying one of the three props alone wipes out the other
     * two. All three are kept and the whole typeface is rebuilt.
     */
    private void applyTypeface(int id, TextView text) {
        FontState state = fontStateOf(id);
        int style = state.bold
                ? (state.italic ? Typeface.BOLD_ITALIC : Typeface.BOLD)
                : (state.italic ? Typeface.ITALIC : Typeface.NORMAL);
        text.setTypeface(
                state.family == null ? null : Typeface.create(state.family, style), style);
    }

    /**
     * Line height and letter spacing.
     *
     * The core already measured with both and the host drew without them: the
     * layout reserved a gap the text did not fill. The spacing goes in ems, so
     * it depends on the font size and has to be redone when that changes.
     */
    private void applyTextMetrics(int id, TextView text) {
        FontState state = fontStateOf(id);
        if (state.letterSpacing != null) {
            float size = text.getTextSize();
            text.setLetterSpacing(size > 0 ? state.letterSpacing * density / size : 0f);
        }
        if (state.lineHeight == null) {
            return;
        }
        int px = Math.round(state.lineHeight * density);
        if (android.os.Build.VERSION.SDK_INT >= 28) {
            text.setLineHeight(px);
            return;
        }
        // Before API 28 there is no line height, only what is added to the one
        // the font already brings: it is subtracted to get to the same place.
        int natural = text.getPaint().getFontMetricsInt(null);
        text.setLineSpacing(Math.max(0, px - natural), 1f);
    }

    // ----------------------------------------------------------------- events

    public void setListener(int id, String event, boolean enabled) {
        View view = views.get(id);
        // A listener on a primitive that never mounted cannot be left waiting in
        // silence: nothing would ever arrive. It was already said on creation.
        if (view == null || unsupported.contains(id)) {
            return;
        }
        if ("refresh".equals(event) && view instanceof AnScrollView) {
            ((AnScrollView) view)
                    .setOnRefresh(
                            enabled
                                    ? () -> {
                                        if (runtime != null) {
                                            runtime.dispatchEvent(id, "refresh", 0f, 0f);
                                        }
                                    }
                                    : null);
            return;
        }
        if ("scroll".equals(event) && view instanceof ScrollView) {
            view.setOnScrollChangeListener(
                    enabled
                            ? (v, x, y, oldX, oldY) -> {
                                if (runtime != null) {
                                    runtime.dispatchEvent(id, "scroll", x / density, y / density);
                                }
                            }
                            : null);
            return;
        }
        if (view instanceof EditText) {
            EditText input = (EditText) view;
            if ("change".equals(event) || "input".equals(event)) {
                // The watcher is kept so it can be removed:
                // `addTextChangedListener` takes no replacement, only add and
                // remove.
                TextWatcher previous = watchers.get(id);
                if (previous != null) {
                    input.removeTextChangedListener(previous);
                    watchers.remove(id);
                }
                if (enabled) {
                    TextWatcher watcher =
                            new TextWatcher() {
                                @Override
                                public void beforeTextChanged(
                                        CharSequence s, int start, int count, int after) {}

                                @Override
                                public void onTextChanged(
                                        CharSequence s, int start, int before, int count) {}

                                @Override
                                public void afterTextChanged(Editable editable) {
                                    if (runtime != null) {
                                        runtime.dispatchValueEvent(
                                                id, "change", editable.toString());
                                    }
                                }
                            };
                    input.addTextChangedListener(watcher);
                    watchers.put(id, watcher);
                }
                return;
            }
            if ("focus".equals(event) || "blur".equals(event)) {
                input.setOnFocusChangeListener(
                        enabled
                                ? (v, hasFocus) -> {
                                    if (runtime != null) {
                                        runtime.dispatchValueEvent(
                                                id,
                                                hasFocus ? "focus" : "blur",
                                                input.getText().toString());
                                    }
                                }
                                : null);
                return;
            }
            if ("submit".equals(event)) {
                input.setOnEditorActionListener(
                        enabled
                                ? (v, actionId, keyEvent) -> {
                                    if (runtime != null) {
                                        runtime.dispatchValueEvent(
                                                id, "submit", input.getText().toString());
                                    }
                                    return false;
                                }
                                : null);
                return;
            }
        }
        if (view instanceof androidx.appcompat.widget.SwitchCompat && "change".equals(event)) {
            ((androidx.appcompat.widget.SwitchCompat) view)
                    .setOnCheckedChangeListener(
                            enabled
                                    ? (button, checked) -> {
                                        if (runtime != null) {
                                            runtime.dispatchValueEvent(
                                                    id, "change", checked ? "true" : "false");
                                        }
                                    }
                                    : null);
            return;
        }
        if (view instanceof com.google.android.material.slider.Slider && "change".equals(event)) {
            com.google.android.material.slider.Slider slider =
                    (com.google.android.material.slider.Slider) view;
            slider.clearOnChangeListeners();
            if (enabled) {
                slider.addOnChangeListener(
                        (control, value, fromUser) -> {
                            if (fromUser && runtime != null) {
                                runtime.dispatchValueEvent(id, "change", String.valueOf(value));
                            }
                        });
            }
            return;
        }
        if (view instanceof AnTabBar && "select".equals(event)) {
            ((AnTabBar) view)
                    .setListener(
                            enabled
                                    ? index -> {
                                        if (runtime != null) {
                                            runtime.dispatchIndexEvent(id, "select", index);
                                        }
                                    }
                                    : null);
            return;
        }
        // The safe area is produced by no gesture: the system knows it, and it
        // changes on rotating or when the navigation bar appears.
        if ("safeArea".equals(event)) {
            if (enabled) {
                safeArea.put(id, new float[] {Float.NaN, Float.NaN, Float.NaN, Float.NaN});
                reportSafeArea(id);
            } else {
                safeArea.remove(id);
            }
            return;
        }
        if ("back".equals(event)) {
            // The hardware back button: the equivalent of the iOS edge gesture.
            // Here it is only reported; undoing the navigation is the router's
            // business.
            backListeners.remove(Integer.valueOf(id));
            if (enabled) {
                backListeners.add(id);
                stackIds.add(id);
            }
            return;
        }
        if ("crown".equals(event) || "crownIdle".equals(event)) {
            setCrown(id, view, event, enabled);
            return;
        }
        Gestures gestures = gestureFor(id, view, event, enabled);
        if (gestures != null) {
            gestures.set(event, enabled);
        }
    }

    /** It has been said once that a phone has no crown; no need to say it again. */
    private boolean crownWarned;

    /**
     * The crown on any old view.
     *
     * Outside a watch this does not exist, and an output that never fires is
     * exactly what is not wanted: it is said on subscribing, which is when there
     * is somebody looking, and not when the event fails to arrive, which is
     * never.
     */
    private void setCrown(int id, View view, String event, boolean enabled) {
        if (!watch) {
            if (enabled && !crownWarned) {
                crownWarned = true;
                android.util.Log.e(
                        "angular-native",
                        "`(" + event + ")` cannot be delivered on this Android: the crown belongs"
                                + " to the watch and there is no wheel to turn here, so this"
                                + " output would never fire");
            }
            return;
        }
        Crown crown = crowns.get(id);
        if (!enabled) {
            if (crown == null) {
                return;
            }
            crown.wants(event, false);
            if (!crown.wanted()) {
                crown.detach();
                crowns.remove(id);
            }
            return;
        }
        if (crown == null) {
            crown = new Crown(id, view);
            crowns.put(id, crown);
            crown.attach();
        }
        crown.wants(event, true);
    }

    /**
     * The Wear OS digital crown on a view that is not a scroll view.
     *
     * `AnScrollView` already listens to it in order to scroll; this is the other
     * half: letting a template ask for it for its own purposes —raising a value,
     * moving between screens— on any `an-view`.
     *
     * What arrives from the system is wheel detents, not points: it is the same
     * axis a mouse moves. They are sent as they come, without converting to
     * pixels the way the scroll view does: nothing is being scrolled here, and
     * converting would mean inventing a scale the template did not ask for.
     *
     * The `offset` counts from when the turn started and not from when the view
     * took the focus, which is what the Apple watch does. The difference is that
     * there the focus is visible —there is a highlight— and here it is not: an
     * Android view that takes the focus does not change its appearance, so
     * "since it took it" would be an origin nobody can see. Since the turn
     * started, one can.
     */
    private final class Crown implements View.OnGenericMotionListener {

        private final int id;
        private final View view;
        /** Whether the template listens to `(crown)`. */
        private boolean turning;
        /** Whether it listens to `(crownIdle)`. */
        private boolean idle;
        /** Detents accumulated since this turn started. */
        private float offset;
        /** When the previous detent of this turn arrived, or 0 if it is the first. */
        private long previous;

        private final Runnable stopped =
                new Runnable() {
                    @Override
                    public void run() {
                        offset = 0;
                        previous = 0;
                        // "It has stopped turning" is a different thing from "it
                        // has turned nothing", so it goes as its own event and not
                        // as a `crown` with a zero delta. Just as on the Apple
                        // watch.
                        if (idle && runtime != null) {
                            runtime.dispatchGesture(id, "crownIdle", "", "", new float[0]);
                        }
                    }
                };

        Crown(int id, View view) {
            this.id = id;
            this.view = view;
        }

        void wants(String event, boolean enabled) {
            if ("crown".equals(event)) {
                turning = enabled;
            } else {
                idle = enabled;
            }
        }

        boolean wanted() {
            return turning || idle;
        }

        void attach() {
            // The crown goes to the view that has the focus, and an ordinary
            // view does not ask for it on its own: without this the system sends
            // the detents to whoever does have it —usually nobody— and the output
            // never fires, with no error at all. It is the same thing
            // `AnScrollView.enableRotary` does.
            view.setFocusable(true);
            view.setFocusableInTouchMode(true);
            view.requestFocus();
            view.setOnGenericMotionListener(this);
        }

        void detach() {
            view.setOnGenericMotionListener(null);
            view.removeCallbacks(stopped);
        }

        @Override
        public boolean onGenericMotion(View v, android.view.MotionEvent event) {
            if (event.getAction() != android.view.MotionEvent.ACTION_SCROLL
                    || !event.isFromSource(android.view.InputDevice.SOURCE_ROTARY_ENCODER)) {
                return false;
            }
            float delta = event.getAxisValue(android.view.MotionEvent.AXIS_SCROLL);
            offset += delta;
            long now = event.getEventTime();
            // Detents per second. The first one of a turn has nothing to compare
            // itself against: zero is the honest answer, and not some huge speed
            // that came out of dividing by the gap before the turn began.
            float velocity = previous == 0 || now <= previous ? 0f : delta * 1000f / (now - previous);
            previous = now;
            if (turning && runtime != null) {
                runtime.dispatchGesture(
                        id,
                        "crown",
                        "",
                        "delta,offset,velocity",
                        new float[] {delta, offset, velocity});
            }
            view.removeCallbacks(stopped);
            view.postDelayed(stopped, CROWN_IDLE_MS);
            // A scroll view uses the crown to scroll, and this listener is
            // consulted before its `onGenericMotionEvent`: keeping the event
            // would stop the list merely by listening to it. The template is told
            // and the event is let through. On any other view there is nobody
            // behind to leave the detent to.
            return !(v instanceof AnScrollView);
        }
    }

    /**
     * Takes a view from its current frame to the new one, interpolating.
     *
     * `ViewPropertyAnimator` is no good here: that animates drawing properties
     * —translation, scale, opacity— and the frame is not one of them, it is the
     * result of the layout. The four numbers have to be interpolated and a
     * layout requested at every step. It is more expensive, and that is why it
     * only happens on the views that asked for it.
     */
    private void animateFrame(
            View view,
            AnViewGroup.Frame frame,
            int left,
            int top,
            int width,
            int height,
            Animation anim) {
        android.animation.ValueAnimator running = frameAnimations.get(view);
        if (running != null) {
            // A new change wins over the one under way: following both at once
            // would send the view to two places.
            running.cancel();
        }
        final int fromLeft = frame.left;
        final int fromTop = frame.top;
        final int fromWidth = frame.width;
        final int fromHeight = frame.height;
        if (fromLeft == left && fromTop == top && fromWidth == width && fromHeight == height) {
            return;
        }
        android.animation.ValueAnimator animator =
                android.animation.ValueAnimator.ofFloat(0f, 1f);
        animator.setDuration(anim.duration);
        animator.setStartDelay(anim.delay);
        animator.setInterpolator(anim.interpolator());
        animator.addUpdateListener(
                a -> {
                    float t = (float) a.getAnimatedValue();
                    frame.left = Math.round(fromLeft + (left - fromLeft) * t);
                    frame.top = Math.round(fromTop + (top - fromTop) * t);
                    frame.width = Math.round(fromWidth + (width - fromWidth) * t);
                    frame.height = Math.round(fromHeight + (height - fromHeight) * t);
                    view.setLayoutParams(frame);
                });
        animator.addListener(
                new android.animation.AnimatorListenerAdapter() {
                    @Override
                    public void onAnimationEnd(android.animation.Animator a) {
                        frameAnimations.remove(view);
                    }
                });
        frameAnimations.put(view, animator);
        animator.start();
    }

    private Animation animationFor(int id) {
        Animation anim = animations.get(id);
        if (anim == null) {
            anim = new Animation();
            animations.put(id, anim);
        }
        return anim;
    }

    /**
     * How to apply a drawing change: directly, or by animating it.
     *
     * Both ways are handled the same —a `ViewPropertyAnimator` with zero
     * duration applies the value and that is that—, so whoever sets the prop
     * does not have to know which of the two applies.
     */
    private android.view.ViewPropertyAnimator visual(View view, int id) {
        Animation anim = animations.get(id);
        android.view.ViewPropertyAnimator animator = view.animate();
        if (anim == null || anim.duration <= 0) {
            return animator.setDuration(0).setStartDelay(0);
        }
        return animator.setDuration(anim.duration)
                .setStartDelay(anim.delay)
                .setInterpolator(anim.interpolator());
    }

    /** How a view animates its changes. */
    private static final class Animation {
        /** Milliseconds. Zero turns the animation off without erasing the rest. */
        long duration;
        long delay;
        String easing = "ease-out";

        android.animation.TimeInterpolator interpolator() {
            switch (easing) {
                case "linear":
                    return new android.view.animation.LinearInterpolator();
                case "ease-in":
                    return new android.view.animation.AccelerateInterpolator();
                case "ease-in-out":
                    return new android.view.animation.AccelerateDecelerateInterpolator();
                default:
                    // It leaves fast and brakes on arrival, as on iOS.
                    return new android.view.animation.DecelerateInterpolator();
            }
        }
    }

    /**
     * The Material icons, inside the app.
     *
     * The set Android ships —`android.R.drawable`— has been frozen since 2011
     * for compatibility: it is Gingerbread's, not Material 3's, and it looks
     * nothing like what people expect today. The current ones live in libraries
     * that are not in the platform, so the app ships the Material Symbols
     * variable font and draws the glyph.
     *
     * They are looked up by codepoint and not by ligature: a ligature that does
     * not exist is drawn as the letters of the name, and an icon that gets it
     * wrong is better missing than spelled out.
     */
    private android.graphics.Typeface iconFont;

    private java.util.HashMap<String, String> iconCodepoints;

    private TextView newIconView() {
        TextView icon = new TextView(context);
        icon.setIncludeFontPadding(false);
        icon.setPadding(0, 0, 0, 0);
        icon.setGravity(Gravity.CENTER);
        icon.setTextSize(TypedValue.COMPLEX_UNIT_DIP, 24f);
        icon.setTag(ICON_TAG);
        android.graphics.Typeface font = iconTypeface();
        if (font != null) {
            icon.setTypeface(font);
        }
        return icon;
    }

    /**
     * A button's background according to its variant.
     *
     * Android does not ship the Material 3 buttons in the platform —they live in
     * the Material library, which is a separate dependency and this build does
     * not use Gradle—, so the pill is drawn here with a `GradientDrawable`. The
     * button is still a real `android.widget.Button`: the only thing of ours is
     * the background.
     */
    private void applyButtonVariant(android.widget.Button button, int id, String variant) {
        buttonVariants.put(id, variant == null ? "text" : variant);
        refreshButton(button, id);
    }

    /**
     * Leaves the button as its variant asks.
     *
     * Everything goes through the `MaterialButton` API —tint, stroke, label
     * colour— and nothing through `setBackground`: a `MaterialButton`
     * **discards** backgrounds it did not make itself, so a pill handed to it by
     * hand would not be drawn and on top of that it would not say so. The
     * drawing is done by its `MaterialShapeDrawable`, which is the one that
     * knows about corners, elevation and ripples on press.
     */
    private void refreshButton(android.widget.Button button, int id) {
        if (!(button instanceof com.google.android.material.button.MaterialButton)) {
            return;
        }
        com.google.android.material.button.MaterialButton material =
                (com.google.android.material.button.MaterialButton) button;
        String variant = buttonVariants.get(id);
        Integer color = buttonColors.get(id);
        int tint = color == null ? Color.WHITE : color;
        int stroke = 0;
        int background = Color.TRANSPARENT;
        int label = tint;

        if ("filled".equals(variant)) {
            background = tint;
            // Over a strong fill the label takes the colour that contrasts, not
            // the button's colour, or it ends up green on green.
            label = contrastOn(tint);
        } else if ("tonal".equals(variant)) {
            // The same colour, heavily dimmed. Material 3 uses the theme's
            // secondary container here; with a colour set by hand, dimming it is
            // the closest thing there is without inventing a palette.
            background = Color.argb(48, Color.red(tint), Color.green(tint), Color.blue(tint));
        } else if ("outlined".equals(variant)) {
            stroke = Math.round(density);
        }

        material.setBackgroundTintList(android.content.res.ColorStateList.valueOf(background));
        material.setStrokeWidth(stroke);
        material.setStrokeColor(android.content.res.ColorStateList.valueOf(tint));
        material.setTextColor(label);
        material.setIconTint(android.content.res.ColorStateList.valueOf(label));
    }

    /** Black or white, whichever reads on that colour. */
    private static int contrastOn(int color) {
        double light =
                (0.299 * Color.red(color) + 0.587 * Color.green(color) + 0.114 * Color.blue(color))
                        / 255.0;
        return light > 0.6 ? Color.BLACK : Color.WHITE;
    }

    /**
     * A Material icon as a `Drawable`, for where the system asks for one and not
     * a view: the toolbar's back arrow.
     */
    private android.graphics.drawable.Drawable iconDrawableFor(String name) {
        String glyph = iconGlyph(name);
        if (glyph.isEmpty()) {
            return null;
        }
        android.graphics.Paint paint = new android.graphics.Paint(
                android.graphics.Paint.ANTI_ALIAS_FLAG);
        paint.setTypeface(iconTypeface());
        paint.setTextSize(24 * density);
        paint.setColor(android.graphics.Color.WHITE);
        android.graphics.Rect bounds = new android.graphics.Rect();
        paint.getTextBounds(glyph, 0, glyph.length(), bounds);
        int side = Math.round(24 * density);
        android.graphics.Bitmap bitmap =
                android.graphics.Bitmap.createBitmap(
                        side, side, android.graphics.Bitmap.Config.ARGB_8888);
        android.graphics.Canvas canvas = new android.graphics.Canvas(bitmap);
        canvas.drawText(
                glyph,
                (side - bounds.width()) / 2f - bounds.left,
                (side - bounds.height()) / 2f - bounds.top,
                paint);
        return new android.graphics.drawable.BitmapDrawable(context.getResources(), bitmap);
    }

    private void dispatchIndex(int id, int index) {
        if (runtime != null) {
            runtime.dispatchIndexEvent(id, "change", index);
        }
    }

    private void dispatchValue(int id, double value) {
        if (runtime != null) {
            runtime.dispatchGesture(
                    id, "change", "", "value", new float[] {(float) value});
        }
    }

    private void dispatchText(int id, String event, String text) {
        if (runtime != null) {
            runtime.dispatchValueEvent(id, event, text == null ? "" : text);
        }
    }

    /** A tab icon's view, or `null` if that name does not exist. */
    private View tabIcon(String name) {
        String glyph = iconGlyph(name);
        if (glyph.isEmpty()) {
            return null;
        }
        TextView icon = newIconView();
        icon.setText(glyph);
        return icon;
    }

    private static final String ICON_TAG = "an-icon";

    private static boolean isIcon(View view) {
        return ICON_TAG.equals(view.getTag());
    }

    private android.graphics.Typeface iconTypeface() {
        if (iconFont == null) {
            try {
                iconFont = android.graphics.Typeface.createFromAsset(
                        context.getAssets(), "material-symbols.ttf");
            } catch (RuntimeException error) {
                android.util.Log.e("angular-native", "the icon font could not be loaded", error);
            }
        }
        return iconFont;
    }

    /** The character that draws this icon, or empty if it does not exist. */
    private String iconGlyph(String name) {
        if (name == null || name.isEmpty()) {
            return "";
        }
        if (iconCodepoints == null) {
            iconCodepoints = loadCodepoints();
        }
        String code = iconCodepoints.get(translateIcon(name));
        if (code == null) {
            return "";
        }
        return new String(Character.toChars(Integer.parseInt(code, 16)));
    }

    private java.util.HashMap<String, String> loadCodepoints() {
        java.util.HashMap<String, String> map = new java.util.HashMap<>();
        try (java.io.BufferedReader reader =
                new java.io.BufferedReader(
                        new java.io.InputStreamReader(
                                context.getAssets().open("material-symbols.codepoints")))) {
            String line;
            while ((line = reader.readLine()) != null) {
                int space = line.indexOf(' ');
                if (space > 0) {
                    map.put(line.substring(0, space), line.substring(space + 1).trim());
                }
            }
        } catch (java.io.IOException error) {
            android.util.Log.e("angular-native", "the icon map could not be read", error);
        }
        return map;
    }

    /**
     * Common names, translated into the Material Symbols one.
     *
     * The ones that already match are not needed: the list is only for those
     * called something different on each platform, so that the same template
     * works on both. Any Material Symbols name passes through as it comes, and
     * there are more than four thousand of them.
     */
    private static String translateIcon(String name) {
        switch (name) {
            case "profile":
            case "account":
                return "account_circle";
            case "back":
                return "arrow_back";
            case "forward":
                return "arrow_forward";
            case "more":
                return "more_horiz";
            case "calendar":
                return "calendar_month";
            case "camera":
                return "photo_camera";
            case "bell":
                return "notifications";
            case "play":
                return "play_arrow";
            case "location":
                return "location_on";
            case "chat":
                return "chat_bubble";
            default:
                return name;
        }
    }

    /** A number from a prop, with its default if none arrived. */
    private float number(String value, float fallback) {
        Float parsed = parseFloat(value);
        return parsed == null ? fallback : parsed;
    }

    // ------------------------------------------------------------- gestures

    /**
     * A view's gestures, all together.
     *
     * <p>An Android view takes only one {@code OnTouchListener}, so putting one
     * per gesture will not do: the last one wipes out the earlier ones. Here
     * there is one object per view that shares the same stream of touches out
     * among whichever detectors are needed.
     */
    private final class Gestures implements android.view.View.OnTouchListener {
        private final int id;
        private final android.view.View view;

        private boolean press;
        private boolean doublePress;
        private boolean longPress;
        private boolean pan;
        private boolean pinch;
        private boolean rotate;
        private boolean swipeLeft;
        private boolean swipeRight;
        private boolean swipeUp;
        private boolean swipeDown;

        private android.view.GestureDetector detector;
        private android.view.ScaleGestureDetector scaler;
        private android.view.VelocityTracker velocity;

        private float startX;
        private float startY;
        private float lastX;
        private float lastY;
        private boolean panning;

        private float rotationStart;
        private float rotationLast;
        private boolean rotating;

        Gestures(int id, android.view.View view) {
            this.id = id;
            this.view = view;
        }

        void set(String event, boolean enabled) {
            switch (event) {
                case "press":
                case "click":
                case "tap":
                    press = enabled;
                    break;
                case "doublePress":
                    doublePress = enabled;
                    break;
                case "longPress":
                    longPress = enabled;
                    break;
                case "pan":
                    pan = enabled;
                    break;
                case "pinch":
                    pinch = enabled;
                    break;
                case "rotate":
                    rotate = enabled;
                    break;
                case "swipeLeft":
                    swipeLeft = enabled;
                    break;
                case "swipeRight":
                    swipeRight = enabled;
                    break;
                case "swipeUp":
                    swipeUp = enabled;
                    break;
                case "swipeDown":
                    swipeDown = enabled;
                    break;
                default:
                    return;
            }
            rebuild();
        }

        /** Is any gesture still active? If not, the view goes back to being clean. */
        private boolean any() {
            return press
                    || doublePress
                    || longPress
                    || pan
                    || pinch
                    || rotate
                    || swipeLeft
                    || swipeRight
                    || swipeUp
                    || swipeDown;
        }

        /**
         * A continuous gesture keeps the touch: while the finger is moving nobody
         * else should see it. With loose taps only, the event is handed back so
         * that the {@code OnClickListener} keeps working, which is what makes the
         * view accessible.
         */
        private boolean consuming() {
            return pan || pinch || rotate;
        }

        private void rebuild() {
            boolean swipes = swipeLeft || swipeRight || swipeUp || swipeDown;
            boolean needsDetector = doublePress || longPress || swipes || (press && consuming());
            detector = needsDetector ? new android.view.GestureDetector(context, listener) : null;
            scaler = pinch ? new android.view.ScaleGestureDetector(context, scaleListener) : null;

            if (!any()) {
                view.setOnTouchListener(null);
                view.setOnClickListener(null);
                view.setClickable(false);
                gestures.remove(Integer.valueOf(id));
                return;
            }
            view.setOnTouchListener(this);
            // Clickable whenever there is any gesture, even if there is nothing
            // to do on a tap: a view that is not clickable only receives the
            // first touch, and without the rest of the sequence there is no
            // double tap and no swipe to recognise.
            view.setClickable(true);

            // The native click only when nobody keeps the touch; otherwise the
            // loose tap is recognised by the detector.
            if (press && !consuming()) {
                view.setOnClickListener(v -> dispatch("press", lastX, lastY));
            } else {
                view.setOnClickListener(null);
            }
        }

        @Override
        public boolean onTouch(android.view.View v, android.view.MotionEvent ev) {
            lastX = ev.getX() / density;
            lastY = ev.getY() / density;
            if (detector != null) {
                detector.onTouchEvent(ev);
            }
            if (scaler != null) {
                scaler.onTouchEvent(ev);
            }
            if (rotate) {
                trackRotation(ev);
            }
            if (pan) {
                trackPan(ev);
            }
            // Returning whatever the detector says will not do here. The
            // detector asks to keep the first touch so it can see the whole
            // gesture, and that runs over the view's click: tapping stopped
            // working as soon as the view also listened for a swipe. The touch is
            // only consumed when there is a continuous gesture, which is when it
            // really must not reach anybody else.
            return consuming();
        }

        /**
         * Dragging. The translation reported is the one from where the finger
         * started, not the one for this movement: it is what whoever is moving
         * something with a finger wants, and it matches what iOS sends.
         */
        private void trackPan(android.view.MotionEvent ev) {
            switch (ev.getActionMasked()) {
                case android.view.MotionEvent.ACTION_DOWN:
                    startX = ev.getX();
                    startY = ev.getY();
                    panning = true;
                    velocity = android.view.VelocityTracker.obtain();
                    velocity.addMovement(ev);
                    emitPan("begin", 0f, 0f);
                    break;
                case android.view.MotionEvent.ACTION_MOVE:
                    if (!panning) {
                        break;
                    }
                    if (velocity != null) {
                        velocity.addMovement(ev);
                    }
                    emitPan("move", ev.getX() - startX, ev.getY() - startY);
                    break;
                case android.view.MotionEvent.ACTION_UP:
                case android.view.MotionEvent.ACTION_CANCEL:
                    if (!panning) {
                        break;
                    }
                    panning = false;
                    boolean cancelled =
                            ev.getActionMasked() == android.view.MotionEvent.ACTION_CANCEL;
                    emitPan(cancelled ? "cancel" : "end", ev.getX() - startX, ev.getY() - startY);
                    if (velocity != null) {
                        velocity.recycle();
                        velocity = null;
                    }
                    break;
                default:
                    break;
            }
        }

        private void emitPan(String state, float dx, float dy) {
            float vx = 0f;
            float vy = 0f;
            if (velocity != null) {
                // Pixels per second, as iOS gives them.
                velocity.computeCurrentVelocity(1000);
                vx = velocity.getXVelocity() / density;
                vy = velocity.getYVelocity() / density;
            }
            dispatchGesture(
                    "pan",
                    state,
                    "x,y,translationX,translationY,velocityX,velocityY",
                    new float[] {lastX, lastY, dx / density, dy / density, vx, vy});
        }

        /**
         * Rotating with two fingers. Android ships no detector for this —there is
         * {@code ScaleGestureDetector} for the pinch, but nothing for the
         * rotation—, so the angle between the two fingers is worked out by hand.
         */
        private void trackRotation(android.view.MotionEvent ev) {
            if (ev.getPointerCount() < 2) {
                if (rotating) {
                    rotating = false;
                    emitRotation("end");
                }
                return;
            }
            float angle = angleBetween(ev);
            if (!rotating) {
                rotating = true;
                rotationStart = angle;
                rotationLast = angle;
                emitRotation("begin");
                return;
            }
            rotationLast = angle;
            emitRotation("move");
        }

        private float angleBetween(android.view.MotionEvent ev) {
            return (float)
                    Math.atan2(ev.getY(1) - ev.getY(0), ev.getX(1) - ev.getX(0));
        }

        private void emitRotation(String state) {
            dispatchGesture(
                    "rotate",
                    state,
                    "rotation,velocity",
                    new float[] {rotationLast - rotationStart, 0f});
        }

        private void dispatch(String name, float x, float y) {
            if (runtime != null) {
                runtime.dispatchEvent(id, name, x, y);
            }
        }

        private void dispatchGesture(String name, String state, String keys, float[] values) {
            if (runtime != null) {
                runtime.dispatchGesture(id, name, state, keys, values);
            }
        }

        private final android.view.GestureDetector.SimpleOnGestureListener listener =
                new android.view.GestureDetector.SimpleOnGestureListener() {
                    @Override
                    public boolean onDown(android.view.MotionEvent e) {
                        // Without this the detector discards the rest of the gesture.
                        return true;
                    }

                    @Override
                    public boolean onSingleTapUp(android.view.MotionEvent e) {
                        if (press && consuming()) {
                            dispatch("press", e.getX() / density, e.getY() / density);
                            return true;
                        }
                        return false;
                    }

                    @Override
                    public boolean onDoubleTap(android.view.MotionEvent e) {
                        if (!doublePress) {
                            return false;
                        }
                        dispatch("doublePress", e.getX() / density, e.getY() / density);
                        return true;
                    }

                    @Override
                    public void onLongPress(android.view.MotionEvent e) {
                        if (longPress) {
                            dispatch("longPress", e.getX() / density, e.getY() / density);
                        }
                    }

                    @Override
                    public boolean onFling(
                            android.view.MotionEvent down,
                            android.view.MotionEvent up,
                            float vx,
                            float vy) {
                        // The axis that moved most wins; on a diagonal, the
                        // faster one. It is the same thing UIKit decides.
                        String direction;
                        if (Math.abs(vx) > Math.abs(vy)) {
                            direction = vx > 0 ? "swipeRight" : "swipeLeft";
                        } else {
                            direction = vy > 0 ? "swipeDown" : "swipeUp";
                        }
                        boolean wanted =
                                ("swipeRight".equals(direction) && swipeRight)
                                        || ("swipeLeft".equals(direction) && swipeLeft)
                                        || ("swipeDown".equals(direction) && swipeDown)
                                        || ("swipeUp".equals(direction) && swipeUp);
                        if (!wanted) {
                            return false;
                        }
                        dispatch(direction, up.getX() / density, up.getY() / density);
                        return true;
                    }
                };

        private final android.view.ScaleGestureDetector.SimpleOnScaleGestureListener
                scaleListener =
                        new android.view.ScaleGestureDetector.SimpleOnScaleGestureListener() {
                            @Override
                            public boolean onScaleBegin(
                                    android.view.ScaleGestureDetector d) {
                                emitScale(d, "begin");
                                return true;
                            }

                            @Override
                            public boolean onScale(android.view.ScaleGestureDetector d) {
                                emitScale(d, "move");
                                return true;
                            }

                            @Override
                            public void onScaleEnd(android.view.ScaleGestureDetector d) {
                                emitScale(d, "end");
                            }

                            private void emitScale(
                                    android.view.ScaleGestureDetector d, String state) {
                                dispatchGesture(
                                        "pinch",
                                        state,
                                        "scale,velocity",
                                        new float[] {d.getScaleFactor(), 0f});
                            }
                        };
    }

    /** This view's gestures, creating them if this is the first one turned on. */
    private Gestures gestureFor(int id, android.view.View view, String event, boolean enabled) {
        Gestures existing = gestures.get(Integer.valueOf(id));
        if (existing != null) {
            return existing;
        }
        if (!enabled) {
            // Removing a gesture from a view that has none: nothing to do.
            return null;
        }
        Gestures created = new Gestures(id, view);
        gestures.put(Integer.valueOf(id), created);
        return created;
    }

    // --------------------------------------------------------------- console

    /** Output from `console.*` and from the core itself. It goes to logcat. */
    public void log(int level, String message) {
        switch (level) {
            case 0:
                android.util.Log.d("angular-native", message);
                break;
            case 2:
                android.util.Log.w("angular-native", message);
                break;
            case 3:
                android.util.Log.e("angular-native", message);
                break;
            default:
                android.util.Log.i("angular-native", message);
                break;
        }
    }

    // --------------------------------------------------------------- images

    private static ImageView.ScaleType scaleTypeOf(String mode) {
        if ("cover".equals(mode)) return ImageView.ScaleType.CENTER_CROP;
        if ("stretch".equals(mode)) return ImageView.ScaleType.FIT_XY;
        if ("center".equals(mode)) return ImageView.ScaleType.CENTER;
        return ImageView.ScaleType.FIT_CENTER;
    }

    /**
     * A path with no scheme is a file in the assets; with `http` or `https` it is
     * downloaded over the network on a separate thread. In both cases the real
     * size is reported with a `load` event: the layout cannot place something
     * whose size it does not know.
     */
    private void loadImage(int id, ImageView view, String source) {
        if (source == null || source.isEmpty()) {
            view.setImageDrawable(null);
            return;
        }
        if (!source.startsWith("http://") && !source.startsWith("https://")) {
            try (java.io.InputStream input = context.getAssets().open(source)) {
                android.graphics.Bitmap bitmap = android.graphics.BitmapFactory.decodeStream(input);
                applyImage(id, view, bitmap);
            } catch (java.io.IOException error) {
                android.util.Log.w("angular-native", source + " could not be opened");
            }
            return;
        }
        new Thread(
                        () -> {
                            android.graphics.Bitmap bitmap = null;
                            try {
                                java.net.HttpURLConnection connection =
                                        (java.net.HttpURLConnection)
                                                new java.net.URL(source).openConnection();
                                connection.setConnectTimeout(10000);
                                try (java.io.InputStream input = connection.getInputStream()) {
                                    bitmap = android.graphics.BitmapFactory.decodeStream(input);
                                } finally {
                                    connection.disconnect();
                                }
                            } catch (Exception error) {
                                android.util.Log.w("angular-native", source + " could not be downloaded");
                            }
                            // Hanging the bitmap off the view does belong to the UI thread.
                            final android.graphics.Bitmap loaded = bitmap;
                            view.post(() -> applyImage(id, view, loaded));
                        },
                        "an-image")
                .start();
    }

    private void applyImage(int id, ImageView view, android.graphics.Bitmap bitmap) {
        if (bitmap == null) {
            return;
        }
        view.setImageBitmap(bitmap);
        if (runtime != null) {
            runtime.dispatchEvent(
                    id, "load", bitmap.getWidth() / density, bitmap.getHeight() / density);
        }
    }

    // ---------------------------------------------------------------- device

    /**
     * Consumed by the `device` native module. It is returned as JSON rather than
     * as an object so as not to have to build a Java map from Rust over JNI.
     */
    public String deviceInfo() {
        return "{\"platform\":\"android\""
                + ",\"systemVersion\":\"" + android.os.Build.VERSION.RELEASE + "\""
                + ",\"model\":\"" + android.os.Build.MODEL + "\""
                + ",\"scale\":" + density
                + ",\"locale\":\"" + java.util.Locale.getDefault().toLanguageTag() + "\"}";
    }

    /**
     * The natural size of a system control, in points.
     *
     * A throwaway one is created and asked: it is the same thing iOS does with
     * `sizeThatFits`, and for the same reason —a switch's height changes between
     * versions of Android and with the accessibility settings.
     */
    public long measureControl(String name, float availableWidthDp) {
        View probe;
        switch (name) {
            case "Switch":
                probe = new com.google.android.material.materialswitch.MaterialSwitch(context);
                break;
            case "Slider":
                probe = new com.google.android.material.slider.Slider(context);
                break;
            case "ActivityIndicator":
                probe = new android.widget.ProgressBar(context);
                break;
            case "ProgressBar":
                probe =
                        new com.google.android.material.progressindicator.LinearProgressIndicator(
                                context);
                break;
            case "Button":
                probe = new com.google.android.material.button.MaterialButton(context);
                break;
            case "TabBar": {
                // The height is decided by Material, not by a constant of ours:
                // a real bar is asked, which is what this method does with every
                // other control.
                //
                // With one tab inside: an empty one measures zero, and then the
                // layout reserves no room for it and it cannot be seen.
                AnTabBar bar = new AnTabBar(context);
                bar.setTitles(new String[] {" "});
                probe = bar;
                break;
            }
            default:
                return 0;
        }
        int unspecified = View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED);
        probe.measure(unspecified, unspecified);
        float width = probe.getMeasuredWidth() / density;
        float height = probe.getMeasuredHeight() / density;

        // The tab bar moves itself clear of the gesture strip by adding it as
        // padding of its own. The probe is loose —no window, no insets to apply—
        // so that gap has to be added here: otherwise Material's 80 dp are shared
        // between content and strip and the label is left with zero height.
        if ("TabBar".equals(name)) {
            height += bottomInsetDp();
        }

        // Sliders and bars take up all the width they are given; their natural
        // measurement only rules the height.
        boolean stretches = "Slider".equals(name) || "ProgressBar".equals(name);
        if (stretches && availableWidthDp > 0) {
            width = availableWidthDp;
        }
        return pack(width, height);
    }

    /** The system strip at the bottom, in points. Zero if not yet known. */
    private float bottomInsetDp() {
        android.view.WindowInsets insets = container.getRootWindowInsets();
        if (insets == null) {
            return 0f;
        }
        return insets.getInsets(
                        android.view.WindowInsets.Type.systemBars()
                                | android.view.WindowInsets.Type.displayCutout())
                        .bottom
                / density;
    }

    /** Width and height in hundredths of a point, packed into a long. */
    private static long pack(float width, float height) {
        return (((long) Math.round(width * 100)) << 32)
                | (Math.round(height * 100) & 0xffffffffL);
    }

    // ------------------------------------------------------------- measuring

    /**
     * Returns width and height packed into a long, in hundredths of a point: two
     * JNI calls per measurement would cost twice as much and gain nothing.
     */
    public long measureText(
            String text,
            float sizeDp,
            int weight,
            boolean italic,
            String family,
            float maxWidthDp,
            int maxLines) {
        measurePaint.setTextSize(sizeDp * density);
        int style = (weight >= 600 ? Typeface.BOLD : Typeface.NORMAL)
                | (italic ? Typeface.ITALIC : Typeface.NORMAL);
        measurePaint.setTypeface(
                family == null || family.isEmpty()
                        ? Typeface.defaultFromStyle(style)
                        : Typeface.create(family, style));

        int limitPx =
                maxWidthDp < 0
                        ? Integer.MAX_VALUE / 2
                        : Math.max(0, Math.round(maxWidthDp * density));

        StaticLayout.Builder builder =
                StaticLayout.Builder.obtain(text, 0, text.length(), measurePaint, limitPx)
                        .setAlignment(Layout.Alignment.ALIGN_NORMAL)
                        .setIncludePad(false);
        if (maxLines > 0) {
            builder.setMaxLines(maxLines);
        }
        StaticLayout layout = builder.build();

        float widthPx = 0;
        for (int line = 0; line < layout.getLineCount(); line++) {
            widthPx = Math.max(widthPx, layout.getLineWidth(line));
        }
        float widthDp = widthPx / density;
        float heightDp = layout.getHeight() / density;

        long packed = ((long) Math.round(widthDp * 100)) << 32;
        return packed | (Math.round(heightDp * 100) & 0xffffffffL);
    }

    // ---------------------------------------------------------------- helpers

    private static int weightOf(String value) {
        try {
            return Integer.parseInt(value);
        } catch (NumberFormatException error) {
            return 400;
        }
    }

    private static int gravityOf(String value) {
        if ("center".equals(value)) {
            return Gravity.CENTER;
        }
        if ("right".equals(value)) {
            return Gravity.END;
        }
        return Gravity.START;
    }

    private static Float parseFloat(String value) {
        try {
            return Float.parseFloat(value);
        } catch (NumberFormatException | NullPointerException error) {
            return null;
        }
    }

    /** It accepts the same as the iOS side: `#rgb`, `#rrggbb`, `#rrggbbaa`. */
    private static Integer parseColor(String value) {
        if (value == null || value.isEmpty()) {
            return null;
        }
        try {
            if (value.startsWith("#") && value.length() == 9) {
                // Android expects #aarrggbb; the web writes #rrggbbaa.
                String rgb = value.substring(1, 7);
                String alpha = value.substring(7, 9);
                return Color.parseColor("#" + alpha + rgb);
            }
            return Color.parseColor(value);
        } catch (IllegalArgumentException error) {
            return null;
        }
    }
}
