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
 * Host de Android: monta vistas y mide texto.
 *
 * Es la contrapartida de `UikitHost`, pero el reparto entre lenguajes es
 * distinto. En iOS, Rust habla con UIKit directamente porque el puente
 * Objective-C es barato y está tipado. Aquí cada llamada cruza JNI, así que
 * Rust manda órdenes gruesas y la lógica de vistas vive en Java, que es donde
 * es natural escribirla.
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
    /** Lo que dura una transición de pila. Igual que en iOS. */
    private static final long TRANSITION_MS = 300;
    /** Resolución del deslizador y de la barra de progreso, que van en enteros. */
    private static final int SLIDER_STEPS = 1000;

    private final Context context;
    private final AnViewGroup container;
    private final float density;
    private final SparseArray<View> views = new SparseArray<>();
    /** El contenido de un ScrollView: Android exige un único hijo. */
    private final SparseArray<AnViewGroup> scrollContent = new SparseArray<>();
    private final TextPaint measurePaint = new TextPaint(TextPaint.ANTI_ALIAS_FLAG);
    private final SparseArray<TextWatcher> watchers = new SparseArray<>();
    /** Radios por esquina en puntos: arriba-izq, arriba-der, abajo-der, abajo-izq. */
    private final SparseArray<float[]> corners = new SparseArray<>();
    /** Tipografía pendiente por nodo: llega en props sueltas y hay que juntarla. */
    private final SparseArray<float[]> fontState = new SparseArray<>();
    /** Sentido de la próxima transición de cada pila: `push`, `pop` o nada. */
    private final SparseArray<String> transitions = new SparseArray<>();
    /** Pantallas que entraron en este frame y aún no se han animado. */
    private final java.util.List<int[]> entering = new java.util.ArrayList<>();
    /** Pantallas que salen: siguen montadas hasta que la animación acaba. */
    private final java.util.List<Object[]> leaving = new java.util.ArrayList<>();
    private final java.util.Set<Integer> animatingOut = new java.util.HashSet<>();
    /** Nodos de pila suscritos a `back`, para el botón físico. */
    private final java.util.List<Integer> backListeners = new java.util.ArrayList<>();
    /** Nodos suscritos al área segura, con los márgenes que ya se les contó. */
    private final SparseArray<float[]> safeArea = new SparseArray<>();
    /** Diálogos declarados, con lo que llevan puesto. */
    private final SparseArray<AlertState> alerts = new SparseArray<>();
    private final java.util.List<Integer> dirtyAlerts = new java.util.ArrayList<>();

    /** Lo que un `Alert` lleva puesto mientras no se presenta. */
    private static final class AlertState {
        String title = "";
        String message = "";
        String[] buttons = new String[0];
        boolean visible;
        android.app.AlertDialog presented;
    }

    private AnRuntime runtime;
    /** Última posición tocada, en puntos y relativa a la vista tocada. */
    private float lastTouchX;
    private float lastTouchY;

    public AnHost(Context context, AnViewGroup container) {
        this.context = context;
        this.container = container;
        this.density = context.getResources().getDisplayMetrics().density;
    }

    public void attachRuntime(AnRuntime runtime) {
        this.runtime = runtime;
    }

    private int px(float dp) {
        return Math.round(dp * density);
    }

    // ------------------------------------------------------------ estructura

    public void createView(int id, int kind) {
        View view;
        switch (kind) {
            case KIND_TEXT: {
                TextView text = new TextView(context);
                text.setIncludeFontPadding(false);
                text.setPadding(0, 0, 0, 0);
                view = text;
                break;
            }
            case KIND_IMAGE:
                view = new ImageView(context);
                break;
            case KIND_SCROLL: {
                AnScrollView scroll = new AnScrollView(context);
                AnViewGroup content = new AnViewGroup(context);
                // El contenido lo mide el ScrollView, no nosotros, y un
                // ScrollView es un FrameLayout por dentro: exige sus propios
                // LayoutParams en el hijo.
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
            case KIND_TABBAR:
                view = new AnTabBar(context);
                break;
            case KIND_SWITCH:
                view = new android.widget.Switch(context);
                break;
            case KIND_SLIDER: {
                android.widget.SeekBar seek = new android.widget.SeekBar(context);
                // El deslizador de Android trabaja con enteros; se usa una
                // escala fija y se convierte al leer y al escribir.
                seek.setMax(SLIDER_STEPS);
                view = seek;
                break;
            }
            case KIND_SPINNER: {
                android.widget.ProgressBar spinner = new android.widget.ProgressBar(context);
                spinner.setIndeterminate(true);
                view = spinner;
                break;
            }
            case KIND_PROGRESS: {
                android.widget.ProgressBar bar =
                        new android.widget.ProgressBar(
                                context, null, android.R.attr.progressBarStyleHorizontal);
                bar.setMax(SLIDER_STEPS);
                view = bar;
                break;
            }
            case KIND_BUTTON:
                view = new android.widget.Button(context);
                break;
            case KIND_MODAL: {
                AnViewGroup overlay = new AnViewGroup(context);
                overlay.setVisibility(View.GONE);
                view = overlay;
                break;
            }
            case KIND_ALERT: {
                // Un diálogo no tiene vista propia: lo presenta el sistema. Se
                // monta una vacía para que el árbol tenga dónde colgarlo.
                View placeholder = new View(context);
                placeholder.setVisibility(View.GONE);
                alerts.put(id, new AlertState());
                view = placeholder;
                break;
            }
            case KIND_STACK: {
                AnViewGroup stack = new AnViewGroup(context);
                // Las pantallas que entran y salen se salen del marco.
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
        // Una pantalla que se está yendo sigue en pantalla hasta que la
        // animación acabe: quitarla ahora daría un salto.
        if (view != null && !animatingOut.contains(id) && view.getParent() instanceof ViewGroup) {
            ((ViewGroup) view.getParent()).removeView(view);
        }
        views.remove(id);
        scrollContent.remove(id);
        watchers.remove(id);
        transitions.remove(id);
        backListeners.remove(Integer.valueOf(id));
        corners.remove(id);
        fontState.remove(id);
        borderWidths.remove(id);
        borderColors.remove(id);
        sliderRanges.remove(id);
        sliderValues.remove(id);
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
            // Una pantalla que entra queda por encima de la que sale, aunque
            // el árbol la coloque antes.
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
            // Se queda montada hasta que termine de salir.
            animatingOut.add(childId);
            leaving.add(new Object[] {parentId, child});
            return;
        }
        parent.removeView(child);
    }

    /** Un ScrollView no admite hijos sueltos: van a su contenedor interno. */
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
        views.clear();
        scrollContent.clear();
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
        frame.left = px(x);
        frame.top = px(y);
        frame.width = px(width);
        frame.height = px(height);
        view.setLayoutParams(frame);
        view.requestLayout();
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

    /** Se llama una vez por frame, cuando ya se aplicaron todas las ops. */
    public void flush() {
        container.requestLayout();
        runStackAnimations();
        syncAlerts();
    }

    private void markAlertDirty(int id) {
        if (!dirtyAlerts.contains(id)) {
            dirtyAlerts.add(id);
        }
    }

    /**
     * Presenta o retira los diálogos que cambiaron.
     *
     * Se hace al cerrar el frame, cuando todas sus props ya llegaron:
     * presentarlo en cuanto cambia `visible` mostraría un diálogo sin título.
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
            android.app.AlertDialog.Builder builder =
                    new android.app.AlertDialog.Builder(context)
                            .setTitle(state.title)
                            .setMessage(state.message)
                            .setCancelable(false);
            // Android coloca los botones por papel, no por orden: con más de
            // tres no cabrían, así que a partir de ahí se usa una lista.
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
     * Anima las pantallas que entraron o salieron en este frame.
     *
     * Se hace aquí y no al insertar porque hasta que el layout no pasa no hay
     * ancho que animar: una pantalla recién creada mide cero.
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
            // Entra desde la derecha; la de debajo se desplaza un tercio, que
            // es el paralaje que hacen las dos plataformas.
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
                    // La vista se quita al acabar; hasta entonces sigue montada.
                    .withEndAction(() -> parent.removeView(screen))
                    .start();
        }
        leaving.clear();
        animatingOut.clear();
    }

    // --- deslizador: Android trabaja en enteros y el framework en flotantes
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

    private void applySliderValue(int id, android.widget.SeekBar seek) {
        float[] range = sliderRange(id);
        Float value = sliderValues.get(id);
        if (value == null) {
            return;
        }
        float span = range[1] - range[0];
        float ratio = span <= 0 ? 0 : (value - range[0]) / span;
        seek.setProgress(Math.round(Math.max(0f, Math.min(1f, ratio)) * SLIDER_STEPS));
    }

    private float sliderValueOf(int id, int progress) {
        float[] range = sliderRange(id);
        return range[0] + (range[1] - range[0]) * progress / (float) SLIDER_STEPS;
    }

    /** Lista de cadenas en JSON: es como viajan los títulos de las pestañas. */
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

    /** Cuenta los márgenes del sistema si cambiaron desde la última vez. */
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
        if (previous[0] == top && previous[1] == right && previous[2] == bottom && previous[3] == left) {
            return;
        }
        safeArea.put(id, new float[] {top, right, bottom, left});
        // Cuatro cifras no caben en un evento de posición: van como JSON, que
        // es el mismo camino que usan las pestañas.
        runtime.dispatchValueEvent(
                id,
                "safeArea",
                "{\"top\":" + top + ",\"right\":" + right
                        + ",\"bottom\":" + bottom + ",\"left\":" + left + "}");
    }

    /** La llama la Activity cuando el usuario pulsa atrás. */
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
        if (view instanceof TextView) {
            ((TextView) view).setText(text);
        }
    }

    public void setProp(int id, String key, String value) {
        View view = views.get(id);
        if (view == null) {
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
            case "opacity":
                view.setAlpha(parseFloat(value) == null ? 1f : parseFloat(value));
                break;
            // --- controles del sistema
            case "on":
                if (view instanceof android.widget.Switch) {
                    ((android.widget.Switch) view).setChecked("true".equals(value));
                }
                break;
            case "minimumValue":
            case "maximumValue": {
                Float bound = parseFloat(value);
                if (view instanceof android.widget.SeekBar && bound != null) {
                    float[] range = sliderRange(id);
                    range["minimumValue".equals(key) ? 0 : 1] = bound;
                    applySliderValue(id, (android.widget.SeekBar) view);
                }
                break;
            }
            case "animating":
                if (view instanceof android.widget.ProgressBar) {
                    view.setVisibility("false".equals(value) ? View.INVISIBLE : View.VISIBLE);
                }
                break;
            case "progress": {
                Float progress = parseFloat(value);
                if (view instanceof android.widget.ProgressBar && progress != null) {
                    ((android.widget.ProgressBar) view)
                            .setProgress(Math.round(Math.max(0f, Math.min(1f, progress)) * SLIDER_STEPS));
                }
                break;
            }
            case "title":
                if (alerts.get(id) != null) {
                    alerts.get(id).title = value == null ? "" : value;
                    markAlertDirty(id);
                    break;
                }
                if (view instanceof android.widget.Button) {
                    ((android.widget.Button) view).setText(value);
                }
                break;
            case "items":
                if (view instanceof AnTabBar) {
                    ((AnTabBar) view).setTitles(parseStringList(value));
                }
                break;
            case "selectedIndex": {
                Float index = parseFloat(value);
                if (view instanceof AnTabBar && index != null) {
                    ((AnTabBar) view).setSelectedIndex(Math.round(index));
                }
                break;
            }
            case "visible":
                if (alerts.get(id) != null) {
                    alerts.get(id).visible = "true".equals(value);
                    markAlertDirty(id);
                    break;
                }
                view.setVisibility("false".equals(value) ? View.GONE : View.VISIBLE);
                break;
            // --- diálogos del sistema
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
            case "testID":
                view.setContentDescription(value);
                break;
            case "color": {
                Integer color = parseColor(value);
                if (color == null) {
                    break;
                }
                if (view instanceof AnTabBar) {
                    ((AnTabBar) view).setActiveColor(color);
                } else if (view instanceof android.widget.Switch) {
                    // El pulgar y la vía llevan tintes distintos; el mismo
                    // color en los dos es lo más parecido al de iOS.
                    ((android.widget.Switch) view)
                            .setThumbTintList(android.content.res.ColorStateList.valueOf(color));
                    ((android.widget.Switch) view)
                            .setTrackTintList(android.content.res.ColorStateList.valueOf(color));
                } else if (view instanceof android.widget.ProgressBar) {
                    ((android.widget.ProgressBar) view)
                            .setProgressTintList(android.content.res.ColorStateList.valueOf(color));
                    ((android.widget.ProgressBar) view)
                            .setIndeterminateTintList(
                                    android.content.res.ColorStateList.valueOf(color));
                } else if (view instanceof android.widget.SeekBar) {
                    ((android.widget.SeekBar) view)
                            .setProgressTintList(android.content.res.ColorStateList.valueOf(color));
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
                    }
                }
                break;
            case "fontWeight":
                if (view instanceof TextView) {
                    boolean bold = "bold".equals(value) || weightOf(value) >= 600;
                    ((TextView) view).setTypeface(null, bold ? Typeface.BOLD : Typeface.NORMAL);
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
                if (view instanceof EditText) {
                    ((EditText) view).setHint(value);
                }
                break;
            case "value": {
                Float sliderValue = parseFloat(value);
                if (view instanceof android.widget.SeekBar && sliderValue != null) {
                    sliderValues.put(id, sliderValue);
                    applySliderValue(id, (android.widget.SeekBar) view);
                    break;
                }
                if (view instanceof EditText) {
                    EditText input = (EditText) view;
                    // Escribir en cada tecla le movería el cursor al final.
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
            default:
                break;
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
     * `setCornerRadii` quiere ocho valores —radio X e Y de cada esquina— en el
     * orden arriba-izq, arriba-der, abajo-der, abajo-izq.
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

    /** El borde son dos props que llegan sueltas y se aplican juntas. */
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

    // ---------------------------------------------------------------- eventos

    public void setListener(int id, String event, boolean enabled) {
        View view = views.get(id);
        if (view == null) {
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
                // Se guarda el observador para poder quitarlo: `addTextChangedListener`
                // no admite reemplazo, solo alta y baja.
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
        if (view instanceof android.widget.Switch && "change".equals(event)) {
            ((android.widget.Switch) view)
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
        if (view instanceof android.widget.SeekBar && "change".equals(event)) {
            ((android.widget.SeekBar) view)
                    .setOnSeekBarChangeListener(
                            enabled
                                    ? new android.widget.SeekBar.OnSeekBarChangeListener() {
                                        @Override
                                        public void onProgressChanged(
                                                android.widget.SeekBar bar,
                                                int progress,
                                                boolean fromUser) {
                                            if (fromUser && runtime != null) {
                                                runtime.dispatchValueEvent(
                                                        id,
                                                        "change",
                                                        String.valueOf(sliderValueOf(id, progress)));
                                            }
                                        }

                                        @Override
                                        public void onStartTrackingTouch(android.widget.SeekBar bar) {}

                                        @Override
                                        public void onStopTrackingTouch(android.widget.SeekBar bar) {}
                                    }
                                    : null);
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
        // El área segura no la produce ningún gesto: la sabe el sistema, y
        // cambia al rotar o al aparecer la barra de navegación.
        if ("safeArea".equals(event)) {
            if (enabled) {
                safeArea.put(id, new float[] {Float.NaN, Float.NaN, Float.NaN, Float.NaN});
                reportSafeArea(id);
            } else {
                safeArea.remove(id);
        AlertState alert = alerts.get(id);
        if (alert != null && alert.presented != null) {
            alert.presented.dismiss();
        }
        alerts.remove(id);
            }
            return;
        }
        if ("back".equals(event)) {
            // El botón físico de atrás: el equivalente del gesto de borde de
            // iOS. Aquí solo se avisa; deshacer la navegación es del router.
            backListeners.remove(Integer.valueOf(id));
            if (enabled) {
                backListeners.add(id);
                stackIds.add(id);
            }
            return;
        }
        if ("doublePress".equals(event)) {
            if (!enabled) {
                view.setOnTouchListener(null);
                return;
            }
            android.view.GestureDetector detector =
                    new android.view.GestureDetector(
                            context,
                            new android.view.GestureDetector.SimpleOnGestureListener() {
                                @Override
                                public boolean onDoubleTap(android.view.MotionEvent e) {
                                    if (runtime != null) {
                                        runtime.dispatchEvent(
                                                id, "doublePress", e.getX() / density, e.getY() / density);
                                    }
                                    return true;
                                }
                            });
            view.setOnTouchListener((v, touch) -> detector.onTouchEvent(touch));
            view.setClickable(true);
            return;
        }
        if ("press".equals(event) || "click".equals(event) || "tap".equals(event)) {
            if (!enabled) {
                view.setOnClickListener(null);
                view.setOnTouchListener(null);
                view.setClickable(false);
                return;
            }
            // `OnClickListener` no dice dónde se tocó, y iOS sí lo manda: se
            // apunta la posición al pasar el dedo y se usa al soltar. El
            // click se mantiene porque es lo que hace la vista accesible.
            view.setOnTouchListener(
                    (v, touch) -> {
                        lastTouchX = touch.getX() / density;
                        lastTouchY = touch.getY() / density;
                        return false;
                    });
            view.setOnClickListener(
                    v -> {
                        if (runtime != null) {
                            runtime.dispatchEvent(id, "press", lastTouchX, lastTouchY);
                        }
                    });
        }
    }

    // --------------------------------------------------------------- consola

    /** Salida de `console.*` y del propio core. Va a logcat. */
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

    // ------------------------------------------------------------- imágenes

    private static ImageView.ScaleType scaleTypeOf(String mode) {
        if ("cover".equals(mode)) return ImageView.ScaleType.CENTER_CROP;
        if ("stretch".equals(mode)) return ImageView.ScaleType.FIT_XY;
        if ("center".equals(mode)) return ImageView.ScaleType.CENTER;
        return ImageView.ScaleType.FIT_CENTER;
    }

    /**
     * Una ruta sin esquema es un fichero de los assets; con `http` o `https`
     * se baja por red en un hilo aparte. En los dos casos se avisa del tamaño
     * real con un evento `load`: el layout no puede colocar algo cuyo tamaño
     * no conoce.
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
                android.util.Log.w("angular-native", "no se pudo abrir " + source);
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
                                android.util.Log.w("angular-native", "no se pudo bajar " + source);
                            }
                            // Colgar el bitmap de la vista sí es del hilo de UI.
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

    // ------------------------------------------------------------ dispositivo

    /**
     * Lo consume el módulo nativo `device`. Se devuelve como JSON en vez de
     * como objeto para no tener que construir un mapa Java desde Rust por JNI.
     */
    public String deviceInfo() {
        return "{\"platform\":\"android\""
                + ",\"systemVersion\":\"" + android.os.Build.VERSION.RELEASE + "\""
                + ",\"model\":\"" + android.os.Build.MODEL + "\""
                + ",\"scale\":" + density
                + ",\"locale\":\"" + java.util.Locale.getDefault().toLanguageTag() + "\"}";
    }

    /**
     * Tamaño natural de un control del sistema, en puntos.
     *
     * Se crea uno de mentira y se le pregunta: es lo mismo que hace iOS con
     * `sizeThatFits`, y por el mismo motivo —el alto de un interruptor cambia
     * entre versiones de Android y con los ajustes de accesibilidad.
     */
    public long measureControl(String name, float availableWidthDp) {
        View probe;
        switch (name) {
            case "Switch":
                probe = new android.widget.Switch(context);
                break;
            case "Slider":
                probe = new android.widget.SeekBar(context);
                break;
            case "ActivityIndicator":
                probe = new android.widget.ProgressBar(context);
                break;
            case "ProgressBar":
                probe =
                        new android.widget.ProgressBar(
                                context, null, android.R.attr.progressBarStyleHorizontal);
                break;
            case "Button":
                probe = new android.widget.Button(context);
                break;
            case "TabBar":
                return pack(
                        availableWidthDp > 0 ? availableWidthDp : 320f, AnTabBar.HEIGHT_DP);
            default:
                return 0;
        }
        int unspecified = View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED);
        probe.measure(unspecified, unspecified);
        float width = probe.getMeasuredWidth() / density;
        float height = probe.getMeasuredHeight() / density;

        // Deslizadores y barras ocupan todo el ancho que se les dé; su medida
        // natural solo manda en el alto.
        boolean stretches = "Slider".equals(name) || "ProgressBar".equals(name);
        if (stretches && availableWidthDp > 0) {
            width = availableWidthDp;
        }
        return pack(width, height);
    }

    /** Ancho y alto en centésimas de punto, empaquetados en un long. */
    private static long pack(float width, float height) {
        return (((long) Math.round(width * 100)) << 32)
                | (Math.round(height * 100) & 0xffffffffL);
    }

    // ---------------------------------------------------------------- medición

    /**
     * Devuelve ancho y alto empaquetados en un long, en centésimas de punto:
     * dos llamadas JNI por medición costarían el doble sin ganar nada.
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

    /** Acepta lo mismo que el lado de iOS: `#rgb`, `#rrggbb`, `#rrggbbaa`. */
    private static Integer parseColor(String value) {
        if (value == null || value.isEmpty()) {
            return null;
        }
        try {
            if (value.startsWith("#") && value.length() == 9) {
                // Android espera #aarrggbb; la web escribe #rrggbbaa.
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
