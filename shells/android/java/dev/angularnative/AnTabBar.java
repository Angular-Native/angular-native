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

    /** Alto estándar de una barra inferior en Android, en dp. */
    public static final int HEIGHT_DP = 56;

    public interface OnTabSelected {
        void onSelected(int index);
    }

    private final float density;
    private OnTabSelected listener;
    private int selected;
    private int activeColor = Color.WHITE;

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

    public void setTitles(String[] titles) {
        removeAllViews();
        for (int index = 0; index < titles.length; index++) {
            TextView tab = new TextView(getContext());
            tab.setText(titles[index]);
            tab.setGravity(Gravity.CENTER);
            tab.setTextSize(TypedValue.COMPLEX_UNIT_SP, 12);
            tab.setSingleLine(true);
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
            tab.setTextColor(active ? activeColor : Color.argb(140, 255, 255, 255));
            tab.setTypeface(null, active ? Typeface.BOLD : Typeface.NORMAL);
        }
    }

    public int heightPx() {
        return Math.round(HEIGHT_DP * density);
    }
}
