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
    /**
     * La tipografía llega en props sueltas —familia, cursiva, peso— y
     * `Typeface.create` las quiere juntas: se guardan hasta poder aplicarlas.
     */
    private final SparseArray<FontState> fontState = new SparseArray<>();

    /** Lo que se sabe de la letra de un nodo, según va llegando. */
    private static final class FontState {
        String family;
        boolean italic;
        boolean bold;
        /** Espaciado entre letras en puntos; Android lo quiere en emes. */
        Float letterSpacing;
        /** Alto de línea en puntos. */
        Float lineHeight;
    }
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
        boolean sheet;
        String title = "";
        String message = "";
        String[] buttons = new String[0];
        boolean visible;
        androidx.appcompat.app.AlertDialog presented;
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

    /** Variante y color de cada botón, que llegan en props sueltas. */
    private final SparseArray<String> buttonVariants = new SparseArray<>();

    private final SparseArray<Integer> buttonColors = new SparseArray<>();

    /** Centro de cada mapa: la latitud y la longitud llegan por separado. */
    private final java.util.HashMap<Integer, float[]> mapCenters = new java.util.HashMap<>();

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
                // `SearchView` es el campo de búsqueda de la plataforma: trae
                // su lupa, su botón de borrar y el teclado con la tecla de
                // buscar. Un `EditText` con un icono al lado no es lo mismo.
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
                // Varias líneas y sin fondo propio: el marco lo pone la
                // plantilla, igual que en el campo de una línea.
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
                // `VideoView` sí está en la plataforma, con sus controles y su
                // gestión de foco de audio.
                android.widget.VideoView video = new android.widget.VideoView(context);
                video.setOnPreparedListener(player -> player.setLooping(true));
                view = video;
                break;
            }
            case KIND_WEB: {
                android.webkit.WebView web = new android.webkit.WebView(context);
                web.getSettings().setJavaScriptEnabled(true);
                // Sin esto los enlaces se abren en el navegador del sistema y
                // la vista se queda en blanco.
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
            case KIND_TABBAR: {
                AnTabBar tabBar = new AnTabBar(context);
                tabBar.setIconResolver(this::iconDrawableFor);
                view = tabBar;
                break;
            }
            case KIND_SWITCH:
                // El de Material 3, con el pulgar que crece y su marca al
                // encender. `android.widget.Switch` es el del framework y se
                // quedó en el aspecto de hace años.
                view = new com.google.android.material.materialswitch.MaterialSwitch(context);
                break;
            case KIND_SLIDER: {
                // El deslizador de Material 3: vía gruesa, tope con forma de
                // barra y la etiqueta del valor al arrastrar. Trabaja con
                // flotantes, así que no hace falta la escala de enteros que
                // pedía `SeekBar`.
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
                // La barra de progreso de Material 3: extremos redondeados y
                // el hueco entre lo hecho y lo que falta.
                com.google.android.material.progressindicator.LinearProgressIndicator bar =
                        new com.google.android.material.progressindicator.LinearProgressIndicator(
                                context);
                bar.setIndeterminate(false);
                bar.setMax(SLIDER_STEPS);
                view = bar;
                break;
            }
            case KIND_BUTTON: {
                // `MaterialButton` en su variante de texto, que es lo que hace
                // un `UIButton` en iOS: así `<Button>` significa lo mismo en
                // las dos plataformas. Con `[variant]` se pide el relleno, y
                // entonces la píldora la pone Material, no nosotros.
                com.google.android.material.button.MaterialButton button =
                        new com.google.android.material.button.MaterialButton(
                                context,
                                null,
                                com.google.android.material.R.attr.materialButtonOutlinedStyle);
                button.setAllCaps(false);
                button.setStrokeWidth(0);
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
        buttonVariants.remove(id);
        buttonColors.remove(id);
        modals.remove(id);
        mapCenters.remove(id);
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
            // El de Material 3, no el del framework: esquinas redondeadas,
            // botones sin mayúsculas forzadas y la tipografía que toca. El de
            // `android.app` se quedó en el aspecto de hace diez años.
            com.google.android.material.dialog.MaterialAlertDialogBuilder builder =
                    new com.google.android.material.dialog.MaterialAlertDialogBuilder(context)
                            .setTitle(state.title)
                            .setCancelable(false);
            if (state.sheet) {
                // Una hoja de acciones en Android es una lista de opciones,
                // no botones al pie: no hay un control aparte para esto.
                builder.setItems(buttons, (dialog, which) -> emitAlertSelection(id, which));
                androidx.appcompat.app.AlertDialog created = builder.create();
                created.setOnDismissListener(d -> state.presented = null);
                created.show();
                state.presented = created;
                continue;
            }
            builder.setMessage(state.message);
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

    /**
     * Pone el rango y el valor en el deslizador de Material.
     *
     * Los tres llegan en props sueltas y en cualquier orden, y `Slider` se
     * queja si el valor cae fuera del rango, así que se ponen juntos y en
     * orden cada vez.
     */
    private void applySliderValue(int id, com.google.android.material.slider.Slider slider) {
        float[] range = sliderRange(id);
        Float value = sliderValues.get(id);
        float min = range[0];
        float max = range[1] > range[0] ? range[1] : range[0] + 1f;
        slider.setValueFrom(min);
        slider.setValueTo(max);
        if (value != null) {
            slider.setValue(Math.max(min, Math.min(max, value)));
        }
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
            case "icons":
                if (view instanceof AnTabBar) {
                    ((AnTabBar) view).setIcons(parseStringList(value));
                }
                break;
            // --- control segmentado, desplegable y fecha
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
                    float[] centro = mapCenters.computeIfAbsent(id, k -> new float[2]);
                    centro["latitude".equals(key) ? 0 : 1] = number(value, 0f);
                    ((AnMapView) view).setCenter(centro[0], centro[1]);
                }
                break;
            case "zoom":
                if (view instanceof AnMapView) {
                    ((AnMapView) view).setZoom(number(value, 12f));
                }
                break;
            case "showsUser":
                // El mapa de aquí no sabe dónde estás: no es el del sistema.
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
                // `VideoView` no expone el volumen; haría falta llegar al
                // `MediaPlayer` de dentro, y no lo entrega.
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
                // Android no pone rótulo al atrás: solo el icono, que es lo
                // que hace cualquier app de la plataforma.
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
            case "name":
                if (view instanceof TextView && isIcon(view)) {
                    ((TextView) view).setText(iconGlyph(value));
                }
                break;
            case "iconSize":
                if (view instanceof TextView && isIcon(view)) {
                    // El tamaño del glifo es el de la caja: un icono de 24
                    // ocupa 24, sin el hueco de línea que deja un texto.
                    ((TextView) view)
                            .setTextSize(TypedValue.COMPLEX_UNIT_DIP, number(value, 24f));
                }
                break;
            case "iconWeight":
                if (view instanceof TextView && isIcon(view)) {
                    // Material Symbols es una fuente variable: el grosor del
                    // trazo es un eje, no otro fichero.
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
                } else if (view instanceof androidx.appcompat.widget.SwitchCompat) {
                    // Solo la vía: el pulgar lo pinta Material para que
                    // contraste con ella. Tintar los dos del mismo color
                    // dejaba el pulgar invisible.
                    ((androidx.appcompat.widget.SwitchCompat) view)
                            .setTrackTintList(android.content.res.ColorStateList.valueOf(color));
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
                    // El color y la variante llegan sueltos y en cualquier
                    // orden: se guardan los dos y se rehace el botón entero.
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
                        // El espaciado entre letras va en emes: al cambiar el
                        // tamaño cambia lo que vale una eme.
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
            case "secureTextEntry":
                if (view instanceof EditText) {
                    EditText secret = (EditText) view;
                    secret.setInputType(
                            android.text.InputType.TYPE_CLASS_TEXT
                                    | ("true".equals(value)
                                            ? android.text.InputType
                                                    .TYPE_TEXT_VARIATION_PASSWORD
                                            : android.text.InputType
                                                    .TYPE_TEXT_VARIATION_NORMAL));
                    // Un campo de contraseña se dibuja en monoespaciada salvo
                    // que se le devuelva la suya después de cambiarle el tipo.
                    applyTypeface(id, secret);
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
                    // El rebote de iOS aquí es el estirón del final del
                    // desplazamiento: el mismo sitio del gesto, dibujado como
                    // lo dibuja cada plataforma.
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

    private FontState fontStateOf(int id) {
        FontState state = fontState.get(id);
        if (state == null) {
            state = new FontState();
            fontState.put(id, state);
        }
        return state;
    }

    /**
     * Familia, cursiva y negrita van juntas o no van.
     *
     * `setTypeface(null, style)` conserva la familia y `Typeface.create` pide
     * el estilo, así que aplicar una sola de las tres props borra las otras
     * dos. Se guardan las tres y se rehace la tipografía entera.
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
     * Interlineado y espaciado entre letras.
     *
     * El núcleo ya medía con los dos y el host dibujaba sin ellos: el layout
     * reservaba un hueco que el texto no llenaba. El espaciado va en emes, así
     * que depende del tamaño de letra y hay que rehacerlo cuando cambia.
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
        // Antes de API 28 no hay alto de línea, solo lo que se añade al que ya
        // trae la fuente: se resta para llegar al mismo sitio.
        int natural = text.getPaint().getFontMetricsInt(null);
        text.setLineSpacing(Math.max(0, px - natural), 1f);
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

    /**
     * Los iconos de Material, en la app.
     *
     * El juego que trae Android —`android.R.drawable`— lleva congelado desde
     * 2011 por compatibilidad: es el de Gingerbread, no el de Material 3, y no
     * se parece en nada a lo que la gente espera hoy. Los actuales viven en
     * librerías que no están en la plataforma, así que la app se trae la
     * fuente variable de Material Symbols y dibuja el glifo.
     *
     * Se buscan por codepoint y no por ligadura: una ligadura que no existe se
     * dibuja como las letras del nombre, y un icono que se equivoca es mejor
     * que no salga a que salga escrito.
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
     * El fondo de un botón según su variante.
     *
     * Android no trae los botones de Material 3 en la plataforma —viven en la
     * librería de Material, que es una dependencia aparte y este build no usa
     * Gradle—, así que la píldora se dibuja aquí con un `GradientDrawable`. El
     * botón sigue siendo un `android.widget.Button` de verdad: lo único
     * nuestro es el fondo.
     */
    private void applyButtonVariant(android.widget.Button button, int id, String variant) {
        buttonVariants.put(id, variant == null ? "text" : variant);
        refreshButton(button, id);
    }

    private void refreshButton(android.widget.Button button, int id) {
        String variant = buttonVariants.get(id);
        Integer color = buttonColors.get(id);
        if (variant == null || "text".equals(variant)) {
            button.setBackground(null);
            if (color != null) {
                button.setTextColor(color);
            }
            return;
        }
        int tint = color == null ? Color.WHITE : color;
        android.graphics.drawable.GradientDrawable pill =
                new android.graphics.drawable.GradientDrawable();
        // Radio enorme a propósito: `GradientDrawable` lo recorta a la mitad
        // del alto, que es justo la píldora de Material 3.
        pill.setCornerRadius(1000f);
        if ("filled".equals(variant)) {
            pill.setColor(tint);
            // Sobre un relleno fuerte el rótulo va del color del fondo de la
            // app, no del color del botón, o no se lee.
            button.setTextColor(contrastOn(tint));
        } else {
            // Tonal: el mismo color muy rebajado, con el rótulo en el color.
            pill.setColor(Color.argb(48, Color.red(tint), Color.green(tint), Color.blue(tint)));
            button.setTextColor(tint);
        }
        button.setBackground(pill);
    }

    /** Blanco o negro, el que se lea sobre ese color. */
    private static int contrastOn(int color) {
        double luz =
                (0.299 * Color.red(color) + 0.587 * Color.green(color) + 0.114 * Color.blue(color))
                        / 255.0;
        return luz > 0.6 ? Color.BLACK : Color.WHITE;
    }

    /**
     * Un icono de Material como `Drawable`, para donde el sistema pide uno y
     * no una vista: la flecha de atrás de la barra de herramientas.
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

    /** La vista del icono de una pestaña, o `null` si ese nombre no existe. */
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
                android.util.Log.e("angular-native", "no se pudo cargar la fuente de iconos", error);
            }
        }
        return iconFont;
    }

    /** El carácter que dibuja este icono, o vacío si no existe. */
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
            android.util.Log.e("angular-native", "no se pudo leer el mapa de iconos", error);
        }
        return map;
    }

    /**
     * Nombres comunes, traducidos al de Material Symbols.
     *
     * Los que ya coinciden no hacen falta: la lista es solo para los que se
     * llaman distinto en cada plataforma, para que la misma plantilla valga
     * para las dos. Cualquier nombre de Material Symbols pasa tal cual, y son
     * más de cuatro mil.
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
                // El alto lo decide Material, no una constante nuestra: se le
                // pregunta a una barra de verdad, que es lo que hace este
                // método con todos los demás controles.
                //
                // Con una pestaña dentro: vacía mide cero, y entonces el
                // layout no le reserva sitio y no se ve.
                AnTabBar barra = new AnTabBar(context);
                barra.setTitles(new String[] {" "});
                probe = barra;
                break;
            }
            default:
                return 0;
        }
        int unspecified = View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED);
        probe.measure(unspecified, unspecified);
        float width = probe.getMeasuredWidth() / density;
        float height = probe.getMeasuredHeight() / density;

        // La barra de pestañas se aparta ella sola de la franja de gestos
        // metiéndola como relleno propio. La sonda está suelta —sin ventana, sin
        // márgenes que aplicar— así que ese hueco hay que sumarlo aquí: si no,
        // los 80 dp de Material se reparten entre contenido y franja y el
        // rótulo se queda con cero de alto.
        if ("TabBar".equals(name)) {
            height += bottomInsetDp();
        }

        // Deslizadores y barras ocupan todo el ancho que se les dé; su medida
        // natural solo manda en el alto.
        boolean stretches = "Slider".equals(name) || "ProgressBar".equals(name);
        if (stretches && availableWidthDp > 0) {
            width = availableWidthDp;
        }
        return pack(width, height);
    }

    /** Franja del sistema de abajo, en puntos. Cero si aún no se conoce. */
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
