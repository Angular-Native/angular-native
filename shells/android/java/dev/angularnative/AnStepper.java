package dev.angularnative;

import android.content.Context;
import android.graphics.Color;
import android.graphics.drawable.GradientDrawable;
import android.util.TypedValue;
import android.view.Gravity;
import android.widget.LinearLayout;
import android.widget.TextView;

/**
 * Subir y bajar de uno en uno.
 *
 * `UIStepper` no tiene equivalente en la plataforma de Android, así que se arma
 * con dos botones y el valor entre ellos. El valor se enseña porque en Android
 * un par de botones sueltos no dice qué están cambiando; en iOS el control no
 * lo enseña porque ahí la convención es tenerlo al lado.
 */
public final class AnStepper extends LinearLayout {

    public interface OnChanged {
        void onChanged(double value);
    }

    private OnChanged listener;
    private double value;
    private double minimum;
    private double maximum = 100;
    private double step = 1;
    private final TextView label;

    public AnStepper(Context context) {
        super(context);
        float density = context.getResources().getDisplayMetrics().density;
        setOrientation(HORIZONTAL);
        setGravity(Gravity.CENTER_VERTICAL);

        GradientDrawable fondo = new GradientDrawable();
        fondo.setCornerRadius(1000f);
        fondo.setColor(Color.argb(28, 255, 255, 255));
        setBackground(fondo);

        addView(boton(context, "−", -1), botonParams(density));
        label = new TextView(context);
        label.setGravity(Gravity.CENTER);
        label.setTextSize(TypedValue.COMPLEX_UNIT_SP, 15);
        label.setTextColor(Color.WHITE);
        addView(label, new LayoutParams(0, LayoutParams.MATCH_PARENT, 1f));
        addView(boton(context, "+", 1), botonParams(density));
        refresh();
    }

    private LayoutParams botonParams(float density) {
        return new LayoutParams(Math.round(44 * density), LayoutParams.MATCH_PARENT);
    }

    private TextView boton(Context context, String texto, int direccion) {
        TextView boton = new TextView(context);
        boton.setText(texto);
        boton.setGravity(Gravity.CENTER);
        boton.setTextSize(TypedValue.COMPLEX_UNIT_SP, 20);
        boton.setTextColor(Color.WHITE);
        boton.setOnClickListener(v -> nudge(direccion));
        return boton;
    }

    private void nudge(int direccion) {
        double next = Math.max(minimum, Math.min(maximum, value + direccion * step));
        if (next == value) {
            return;
        }
        value = next;
        refresh();
        if (listener != null) {
            listener.onChanged(value);
        }
    }

    public void setListener(OnChanged listener) {
        this.listener = listener;
    }

    public void setValue(double value) {
        this.value = Math.max(minimum, Math.min(maximum, value));
        refresh();
    }

    public void setMinimum(double minimum) {
        this.minimum = minimum;
        setValue(value);
    }

    public void setMaximum(double maximum) {
        this.maximum = maximum;
        setValue(value);
    }

    public void setStep(double step) {
        this.step = step > 0 ? step : 1;
    }

    private void refresh() {
        // Sin decimales cuando el paso es entero: "3" y no "3.0".
        if (step == Math.rint(step) && value == Math.rint(value)) {
            label.setText(String.valueOf((long) value));
        } else {
            label.setText(String.valueOf(value));
        }
    }
}
