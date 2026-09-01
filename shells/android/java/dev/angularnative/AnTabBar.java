package dev.angularnative;

import android.content.Context;
import android.graphics.Color;
import android.graphics.Typeface;
import android.util.TypedValue;
import android.view.Gravity;
import android.view.View;
import android.widget.LinearLayout;
import android.widget.TextView;

/**
 * Barra de pestañas.
 *
 * Android no trae una en la plataforma: `BottomNavigationView` vive en
 * Material, que es una dependencia aparte. Se dibuja aquí con vistas del
 * sistema, respetando el alto y la tipografía que usa el sistema, para que
 * `TabBar` signifique lo mismo en las dos plataformas aunque debajo no lo sea.
 */
public final class AnTabBar extends LinearLayout {

    /**
     * Alto estándar de una barra inferior en Android, en dp.
     *
     * 56 es el alto de una barra de solo texto. Con icono encima del rótulo
     * hace falta más, o el rótulo se sale por abajo: 80 es lo que usa la barra
     * de navegación inferior de Material.
     */
    public static final int HEIGHT_DP = 80;

    public interface OnTabSelected {
        void onSelected(int index);
    }

    /** Crea la vista del icono de una pestaña, o `null` si ese nombre no existe. */
    public interface IconFactory {
        View create(String name);
    }

    private final float density;
    private OnTabSelected listener;
    private IconFactory iconFactory;
    private int selected;
    private int activeColor = Color.WHITE;
    private String[] titles = new String[0];
    private String[] icons = new String[0];

    public AnTabBar(Context context) {
        super(context);
        this.density = context.getResources().getDisplayMetrics().density;
        setOrientation(HORIZONTAL);
        setGravity(Gravity.CENTER_VERTICAL);
    }

    public void setListener(OnTabSelected listener) {
        this.listener = listener;
    }

    public void setIconFactory(IconFactory factory) {
        this.iconFactory = factory;
        rebuild();
    }

    public void setActiveColor(int color) {
        this.activeColor = color;
        refresh();
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

    public void setSelectedIndex(int index) {
        selected = index;
        refresh();
    }

    private void rebuild() {
        removeAllViews();
        for (int index = 0; index < titles.length; index++) {
            LinearLayout tab = new LinearLayout(getContext());
            tab.setOrientation(VERTICAL);
            tab.setGravity(Gravity.CENTER);

            if (iconFactory != null && index < icons.length) {
                View icon = iconFactory.create(icons[index]);
                if (icon != null) {
                    tab.addView(
                            icon,
                            new LayoutParams(
                                    LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT));
                }
            }

            TextView label = new TextView(getContext());
            label.setText(titles[index]);
            label.setGravity(Gravity.CENTER);
            label.setTextSize(TypedValue.COMPLEX_UNIT_SP, 12);
            label.setSingleLine(true);
            tab.addView(
                    label, new LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT));

            final int position = index;
            tab.setOnClickListener(
                    v -> {
                        setSelectedIndex(position);
                        if (listener != null) {
                            listener.onSelected(position);
                        }
                    });
            // Todas las pestañas reparten el ancho por igual.
            addView(tab, new LayoutParams(0, LayoutParams.MATCH_PARENT, 1f));
        }
        refresh();
    }

    private void refresh() {
        for (int index = 0; index < getChildCount(); index++) {
            View child = getChildAt(index);
            if (!(child instanceof LinearLayout)) {
                continue;
            }
            LinearLayout tab = (LinearLayout) child;
            boolean active = index == selected;
            int color = active ? activeColor : Color.argb(140, 255, 255, 255);
            // El icono y el rótulo llevan el mismo color: si no, la pestaña
            // activa tendría el rótulo encendido y el icono apagado.
            for (int part = 0; part < tab.getChildCount(); part++) {
                View inner = tab.getChildAt(part);
                if (inner instanceof TextView) {
                    ((TextView) inner).setTextColor(color);
                }
            }
            View last = tab.getChildAt(tab.getChildCount() - 1);
            if (last instanceof TextView) {
                ((TextView) last).setTypeface(null, active ? Typeface.BOLD : Typeface.NORMAL);
            }
        }
    }

    public int heightPx() {
        return Math.round(HEIGHT_DP * density);
    }
}
