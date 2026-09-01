package dev.angularnative;

import android.content.Context;
import android.graphics.Color;
import android.graphics.drawable.GradientDrawable;
import android.util.TypedValue;
import android.view.Gravity;
import android.widget.LinearLayout;
import android.widget.TextView;

/**
 * Control segmentado.
 *
 * Android no lo trae en la plataforma —el de Material vive en una librería
 * aparte, que este build no usa—, así que se dibuja aquí con vistas del
 * sistema, igual que la barra de pestañas: el segmento elegido lleva una
 * píldora detrás, que es como se ve en Material 3.
 */
public final class AnSegmentedControl extends LinearLayout {

    public interface OnSelected {
        void onSelected(int index);
    }

    private final float density;
    private OnSelected listener;
    private String[] items = new String[0];
    private int selected;
    private int activeColor = Color.WHITE;

    public AnSegmentedControl(Context context) {
        super(context);
        this.density = context.getResources().getDisplayMetrics().density;
        setOrientation(HORIZONTAL);
        setGravity(Gravity.CENTER_VERTICAL);
        GradientDrawable fondo = new GradientDrawable();
        fondo.setCornerRadius(1000f);
        fondo.setColor(Color.argb(28, 255, 255, 255));
        setBackground(fondo);
        int pad = Math.round(3 * density);
        setPadding(pad, pad, pad, pad);
    }

    public void setListener(OnSelected listener) {
        this.listener = listener;
    }

    public void setActiveColor(int color) {
        this.activeColor = color;
        refresh();
    }

    public void setItems(String[] items) {
        this.items = items;
        removeAllViews();
        for (int index = 0; index < items.length; index++) {
            TextView segment = new TextView(getContext());
            segment.setText(items[index]);
            segment.setGravity(Gravity.CENTER);
            segment.setTextSize(TypedValue.COMPLEX_UNIT_SP, 13);
            segment.setSingleLine(true);
            final int position = index;
            segment.setOnClickListener(
                    v -> {
                        setSelectedIndex(position);
                        if (listener != null) {
                            listener.onSelected(position);
                        }
                    });
            addView(segment, new LayoutParams(0, LayoutParams.MATCH_PARENT, 1f));
        }
        refresh();
    }

    public void setSelectedIndex(int index) {
        selected = index;
        refresh();
    }

    private void refresh() {
        for (int index = 0; index < getChildCount(); index++) {
            TextView segment = (TextView) getChildAt(index);
            boolean active = index == selected;
            if (active) {
                GradientDrawable pill = new GradientDrawable();
                pill.setCornerRadius(1000f);
                pill.setColor(activeColor);
                segment.setBackground(pill);
                segment.setTextColor(contrast(activeColor));
            } else {
                segment.setBackground(null);
                segment.setTextColor(Color.argb(180, 255, 255, 255));
            }
        }
    }

    /** Blanco o negro, el que se lea sobre ese color. */
    private static int contrast(int color) {
        double luz =
                (0.299 * Color.red(color) + 0.587 * Color.green(color) + 0.114 * Color.blue(color))
                        / 255.0;
        return luz > 0.6 ? Color.BLACK : Color.WHITE;
    }
}
