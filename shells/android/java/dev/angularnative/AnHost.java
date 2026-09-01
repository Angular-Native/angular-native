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
    /**
     * Tamaño de letra por defecto, en dp. Tiene que ser el mismo que el de
     * `FontSpec::default()` en el núcleo: es con el que se mide.
     */
    private static final float DEFAULT_FONT_SIZE = 14f;

    /**
     * Tema del diálogo: sin marco ni fondo, porque el contenido lo dibuja el
     * árbol de vistas y el marco del sistema se vería por encima.
     */
    private static final int ANDROID_DIALOG_THEME = android.R.style.Theme_Translucent_NoTitleBar;

    /** Estado de presentación de cada `<Modal>`. */
    private final SparseArray<ModalState> modals = new SparseArray<>();

    private final java.util.ArrayList<Integer> dirtyModals = new java.util.ArrayList<>();

    /** Los ajustes de animación de cada vista que los haya pedido. */
    private final android.util.SparseArray<Animation> animations = new android.util.SparseArray<>();

    /** Las animaciones de marco en marcha, para poder cancelarlas. */
    private final java.util.HashMap<View, android.animation.ValueAnimator> frameAnimations =
            new java.util.HashMap<>();

    /** Los gestos activos de cada vista, uno por vista que tenga alguno. */
    private final java.util.HashMap<Integer, Gestures> gestures = new java.util.HashMap<>();

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
                // La misma medida con la que el núcleo midió. El tamaño por
                // defecto de un TextView depende del tema, y si no coincide
                // con el del layout el texto se sale de su caja y lo recorta
                // el padre, sin error ninguno.
                text.setTextSize(
                        android.util.TypedValue.COMPLEX_UNIT_DIP, DEFAULT_FONT_SIZE);
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
                modals.put(id, new ModalState());
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
        animations.remove(id);
        modals.remove(id);
        gestures.remove(Integer.valueOf(id));
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

    /** Se llama una vez por frame, cuando ya se aplicaron todas las ops. */
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
     * Presenta o retira los `<Modal>` que cambiaron.
     *
     * Un `Dialog` de verdad, no una vista encima de las demás. La diferencia
     * no es cómo se ve —eso era igual— sino que el sistema sepa que hay algo
     * modal delante: el botón de atrás lo cierra, TalkBack deja de leer lo de
     * detrás, y no compite en orden de dibujo con los diálogos del sistema.
     *
     * Va después del layout: el diálogo se lleva la vista tal como esté, y sin
     * marco calculado presentaría una caja vacía.
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
                    // Pegado abajo y a lo ancho, que es lo que se espera de
                    // una hoja. El alto lo pone el contenido.
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
                        // Cerrarlo desde fuera —el botón de atrás— también
                        // tiene que llegar a la plantilla, o la señal que lo
                        // abrió se queda diciendo que sigue abierto.
                        if (runtime != null) {
                            runtime.dispatchEvent(id, "dismiss", 0f, 0f);
                        }
                    });
            dialog.show();
            state.presented = dialog;
        }
        dirtyModals.clear();
    }

    /** Cómo está presentado un `<Modal>`. */
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
                visual(view, id).alpha(number(value, 1f));
                break;
            // Animación: no es un valor que se vea, dice cómo se llega a los
            // que sí.
            case "animate":
                animationFor(id).duration = (long) number(value, 0f);
                break;
            case "animateDelay":
                animationFor(id).delay = (long) number(value, 0f);
                break;
            case "animateEasing":
                animationFor(id).easing = value == null ? "ease-out" : value;
                break;
            // Transformaciones. No pasan por el layout: mover o escalar una
            // vista no cambia el sitio que ocupa, así que no hay que
            // recalcular nada y se puede seguir al dedo sin coste.
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
                // La API va en radianes, como el gesto de girar; Android
                // quiere grados.
                visual(view, id).rotation((float) Math.toDegrees(number(value, 0f)));
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
                if (modals.get(id) != null) {
                    modals.get(id).visible = "true".equals(value);
                    // Escondida mientras no esté presentada: quien la enseña
                    // es el diálogo. Visible sin presentar se dibujaría en
                    // línea sobre la página.
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
        Gestures gestures = gestureFor(id, view, event, enabled);
        if (gestures != null) {
            gestures.set(event, enabled);
        }
    }

    /**
     * Lleva una vista de su marco actual al nuevo, interpolando.
     *
     * Aquí no vale `ViewPropertyAnimator`: ese anima propiedades de dibujo
     * —desplazamiento, escala, opacidad— y el marco no es una de ellas, es el
     * resultado del layout. Hay que interpolar los cuatro números y pedir
     * layout en cada paso. Es más caro, y por eso solo pasa en las vistas que
     * lo han pedido.
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
            // Un cambio nuevo manda sobre el que estaba en marcha: seguir los
            // dos a la vez haría que la vista fuese a dos sitios.
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
     * Por dónde aplicar un cambio de dibujo: directo, o animándolo.
     *
     * Las dos formas se manejan igual —`ViewPropertyAnimator` con duración
     * cero aplica el valor y ya—, así que quien pone la prop no tiene que
     * saber cuál de las dos le toca.
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

    /** Cómo anima una vista sus cambios. */
    private static final class Animation {
        /** Milisegundos. Cero apaga la animación sin borrar el resto. */
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
                    // Sale rápido y frena al llegar, como en iOS.
                    return new android.view.animation.DecelerateInterpolator();
            }
        }
    }

    /** Un número de una prop, con su valor por defecto si no llegó ninguno. */
    private float number(String value, float fallback) {
        Float parsed = parseFloat(value);
        return parsed == null ? fallback : parsed;
    }

    // --------------------------------------------------------------- gestos

    /**
     * Los gestos de una vista, todos juntos.
     *
     * <p>Una vista de Android solo admite un {@code OnTouchListener}, así que
     * no vale poner uno por gesto: el último machaca a los anteriores. Aquí
     * hay un objeto por vista que reparte el mismo flujo de toques entre los
     * detectores que hagan falta.
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

        /** ¿Queda algún gesto activo? Si no, la vista vuelve a estar limpia. */
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
         * Un gesto continuo se queda el toque: mientras el dedo se mueve nadie
         * más debe verlo. Con solo toques sueltos se devuelve el evento para
         * que el {@code OnClickListener} siga funcionando, que es lo que hace
         * la vista accesible.
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
            // Clicable siempre que haya algún gesto, aunque no haya nada que
            // hacer al tocar: una vista que no lo es solo recibe el primer
            // toque, y sin el resto del recorrido no hay doble toque ni
            // deslizamiento que reconocer.
            view.setClickable(true);

            // El click nativo solo cuando nadie se queda el toque; si no, el
            // toque suelto lo reconoce el detector.
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
            // Aquí no vale devolver lo que diga el detector. El detector pide
            // quedarse el primer toque para poder ver el gesto entero, y eso
            // se lleva por delante el click de la vista: tocar dejaba de
            // funcionar en cuanto la vista escuchaba también un deslizamiento.
            // Solo se consume cuando hay un gesto continuo, que es cuando de
            // verdad no debe llegar a nadie más.
            return consuming();
        }

        /**
         * Arrastre. Se manda el desplazamiento desde donde empezó el dedo, no
         * el de este movimiento: es lo que quiere quien mueve algo con el
         * dedo, y coincide con lo que manda iOS.
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
                // Píxeles por segundo, como los da iOS.
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
         * Girar con dos dedos. Android no trae detector para esto —hay
         * {@code ScaleGestureDetector} para el pellizco, pero nada para el
         * giro—, así que se saca el ángulo entre los dos dedos a mano.
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
                        // Sin esto el detector descarta el resto del gesto.
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
                        // Gana el eje que más se ha movido; en diagonal, el
                        // más rápido. Es lo mismo que decide UIKit.
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

    /** Los gestos de esta vista, creándolos si es el primero que se activa. */
    private Gestures gestureFor(int id, android.view.View view, String event, boolean enabled) {
        Gestures existing = gestures.get(Integer.valueOf(id));
        if (existing != null) {
            return existing;
        }
        if (!enabled) {
            // Quitar un gesto de una vista que no tiene ninguno: nada que hacer.
            return null;
        }
        Gestures created = new Gestures(id, view);
        gestures.put(Integer.valueOf(id), created);
        return created;
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
