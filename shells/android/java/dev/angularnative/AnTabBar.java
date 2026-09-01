package dev.angularnative;

import android.content.Context;
import android.graphics.drawable.Drawable;
import android.view.Menu;
import android.view.MenuItem;

import com.google.android.material.bottomnavigation.BottomNavigationView;

/**
 * Barra de pestañas.
 *
 * Es la `BottomNavigationView` de Material 3, la de verdad: la plataforma de
 * Android no trae ninguna —`android.widget` se quedó en las pestañas de 2011—
 * y la de Material vive en una librería aparte, que este build se trae
 * resuelta a mano porque no usa Gradle.
 *
 * De ahí salen el indicador de píldora detrás del icono elegido, la animación
 * al cambiar, el comportamiento con TalkBack y el alto que le toque en cada
 * versión del sistema.
 */
public final class AnTabBar extends BottomNavigationView {

    public interface OnTabSelected {
        void onSelected(int index);
    }

    /** Resuelve el nombre de un icono a un drawable. */
    public interface IconResolver {
        Drawable resolve(String name);
    }

    private OnTabSelected listener;
    private IconResolver iconResolver;
    private String[] titles = new String[0];
    private String[] icons = new String[0];
    /** Para no avisar de la pestaña que se acaba de fijar desde la plantilla. */
    private boolean fijando;
    /** Los dos colores de la barra, que llegan en props distintas. */
    private Integer activeColor;
    private Integer inactiveColor;

    public AnTabBar(Context context) {
        super(context);
        // Con rótulo siempre, como la barra de la foto de Material 3. El modo
        // automático los esconde en cuanto hay más de tres pestañas.
        setLabelVisibilityMode(LABEL_VISIBILITY_LABELED);
        setOnItemSelectedListener(
                item -> {
                    if (!fijando && listener != null) {
                        listener.onSelected(item.getItemId() - 1);
                    }
                    return true;
                });
    }

    public void setListener(OnTabSelected listener) {
        this.listener = listener;
    }

    public void setIconResolver(IconResolver resolver) {
        this.iconResolver = resolver;
        rebuild();
    }

    public void setTitles(String[] titles) {
        this.titles = titles;
        rebuild();
    }

    /** Iconos, en el mismo orden que los títulos. */
    public void setIcons(String[] icons) {
        this.icons = icons;
        rebuild();
    }

    public void setActiveColor(int color) {
        this.activeColor = color;
        applyColors();
    }

    /** Color de las pestañas que no están elegidas. */
    public void setInactiveColor(int color) {
        this.inactiveColor = color;
        applyColors();
    }

    /**
     * El color de la pestaña elegida y el de las demás, que llegan sueltos.
     *
     * Sin decir nada, el de las inactivas es el activo rebajado. Antes salía
     * del tinte que ya tuviera el icono, y eso las dejaba en blanco sobre el
     * fondo claro de la barra: los rótulos estaban ahí, del color del fondo.
     */
    private void applyColors() {
        if (activeColor == null && inactiveColor == null) {
            return;
        }
        int activo = activeColor == null ? inactiveColor : activeColor;
        int apagado =
                inactiveColor != null
                        ? inactiveColor
                        : android.graphics.Color.argb(
                                150,
                                android.graphics.Color.red(activo),
                                android.graphics.Color.green(activo),
                                android.graphics.Color.blue(activo));
        android.content.res.ColorStateList lista =
                new android.content.res.ColorStateList(
                        new int[][] {new int[] {android.R.attr.state_checked}, new int[] {}},
                        new int[] {activo, apagado});
        setItemIconTintList(lista);
        setItemTextColor(lista);
    }

    public void setSelectedIndex(int index) {
        if (index < 0 || index >= getMenu().size()) {
            return;
        }
        // Fijarla desde la plantilla no es elegirla: avisar aquí devolvería el
        // evento a quien lo acaba de provocar.
        fijando = true;
        setSelectedItemId(index + 1);
        fijando = false;
    }

    private void rebuild() {
        Menu menu = getMenu();
        menu.clear();
        for (int index = 0; index < titles.length; index++) {
            // Los identificadores empiezan en 1: el 0 es `Menu.NONE` y la
            // barra lo trata como "sin elemento".
            MenuItem item = menu.add(Menu.NONE, index + 1, index, titles[index]);
            if (iconResolver != null && index < icons.length) {
                Drawable icono = iconResolver.resolve(icons[index]);
                if (icono != null) {
                    item.setIcon(icono);
                }
            }
        }
        setLabelVisibilityMode(LABEL_VISIBILITY_LABELED);
    }

    /**
     * Alto de la barra. Lo decide Material, no nosotros: se le pregunta
     * midiendo, que es lo que hace cualquier layout de Android.
     */
    public int heightPx() {
        measure(
                MeasureSpec.makeMeasureSpec(0, MeasureSpec.UNSPECIFIED),
                MeasureSpec.makeMeasureSpec(0, MeasureSpec.UNSPECIFIED));
        return getMeasuredHeight();
    }
}
