package dev.angularnative;

import android.content.Context;
import android.graphics.Color;
import android.graphics.Typeface;
import android.util.TypedValue;
import android.view.Gravity;
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

    private final float density;
    private OnTabSelected listener;
    private int selected;
    private int activeColor = Color.WHITE;
    private String[] titles = new String[0];
    private String[] icons = new String[0];
    /** Resuelve el nombre de un icono a un drawable del sistema. */
    public interface IconResolver {
        android.graphics.drawable.Drawable resolve(String name);
    }

    private IconResolver iconResolver;

    public AnTabBar(Context context) {
        super(context);
        this.density = context.getResources().getDisplayMetrics().density;
        setOrientation(HORIZONTAL);
        setGravity(Gravity.CENTER_VERTICAL);
    }

    public void setListener(OnTabSelected listener) {
        this.listener = listener;
    }

    public void setActiveColor(int color) {
        this.activeColor = color;
        refresh();
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

    private void rebuild() {
        removeAllViews();
        for (int index = 0; index < titles.length; index++) {
            TextView tab = new TextView(getContext());
            tab.setText(titles[index]);
            tab.setGravity(Gravity.CENTER);
            tab.setTextSize(TypedValue.COMPLEX_UNIT_SP, 12);
            tab.setSingleLine(true);
            // El icono va encima del texto, que es donde lo pone Android.
            if (iconResolver != null && index < icons.length) {
                android.graphics.drawable.Drawable icon = iconResolver.resolve(icons[index]);
                if (icon != null) {
                    int side = Math.round(24 * density);
                    icon.setBounds(0, 0, side, side);
                    tab.setCompoundDrawables(null, icon, null, null);
                    tab.setCompoundDrawablePadding(Math.round(2 * density));
                }
            }
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

    public void setSelectedIndex(int index) {
        selected = index;
        refresh();
    }

    private void refresh() {
        for (int index = 0; index < getChildCount(); index++) {
            TextView tab = (TextView) getChildAt(index);
            boolean active = index == selected;
            int color = active ? activeColor : Color.argb(140, 255, 255, 255);
            tab.setTextColor(color);
            tab.setTypeface(null, active ? Typeface.BOLD : Typeface.NORMAL);
            // El icono se tiñe con el mismo color que su texto: si no, la
            // pestaña activa tendría el rótulo encendido y el icono apagado.
            for (android.graphics.drawable.Drawable drawable : tab.getCompoundDrawables()) {
                if (drawable != null) {
                    drawable.setTint(color);
                }
            }
        }
    }

    public int heightPx() {
        return Math.round(HEIGHT_DP * density);
    }
}
