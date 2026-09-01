package dev.angularnative;

import android.content.Context;
import android.view.Gravity;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import com.google.android.material.button.MaterialButton;
import com.google.android.material.textview.MaterialTextView;

/**
 * Subir y bajar de uno en uno.
 *
 * Material 3 no tiene «stepper»: no es que falte en la librería, es que no
 * existe en el sistema de diseño. Así que se arma con piezas que sí son suyas
 * —dos botones de icono y un rótulo—, y no dibujando una imitación.
 *
 * El valor se enseña porque en Android un par de botones sueltos no dice qué
 * están cambiando; en iOS el control no lo enseña porque ahí la convención es
 * tenerlo al lado.
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
    private final MaterialTextView label;
    private final MaterialButton menos;
    private final MaterialButton mas;

    public AnStepper(Context context) {
        super(context);
        setOrientation(HORIZONTAL);
        setGravity(Gravity.CENTER_VERTICAL);

        menos = boton(context, "−", -1);
        mas = boton(context, "+", 1);
        label = new MaterialTextView(context);
        label.setGravity(Gravity.CENTER);
        label.setTextAppearance(
                com.google.android.material.R.style.TextAppearance_Material3_TitleMedium);

        addView(menos);
        addView(label, new LayoutParams(0, ViewGroup.LayoutParams.MATCH_PARENT, 1f));
        addView(mas);
        refresh();
    }

    private MaterialButton boton(Context context, String texto, int direccion) {
        // Botón de icono de Material 3: redondo, del tamaño que manda el
        // sistema y con sus ondas al pulsar. Lleva texto en vez de icono porque
        // «−» y «+» son eso, un carácter.
        MaterialButton boton =
                new MaterialButton(
                        context, null, com.google.android.material.R.attr.materialIconButtonStyle);
        boton.setText(texto);
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
        // Un botón que no puede hacer nada se apaga, que es lo que hace
        // cualquier control del sistema al llegar al tope.
        menos.setEnabled(value > minimum);
        mas.setEnabled(value < maximum);
    }
}
