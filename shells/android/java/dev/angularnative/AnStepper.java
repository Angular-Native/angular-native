package dev.angularnative;

import android.content.Context;
import android.view.Gravity;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import com.google.android.material.button.MaterialButton;
import com.google.android.material.textview.MaterialTextView;

/**
 * Stepping up and down one at a time.
 *
 * Material 3 has no "stepper": it is not that the library is missing one, it is
 * that it does not exist in the design system. So it is assembled from pieces
 * that are Material's own —two icon buttons and a label—, and not by drawing an
 * imitation.
 *
 * The value is shown because on Android a pair of loose buttons does not say
 * what they are changing; on iOS the control does not show it because there the
 * convention is to have it alongside.
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
    private final MaterialButton minus;
    private final MaterialButton plus;

    public AnStepper(Context context) {
        super(context);
        setOrientation(HORIZONTAL);
        setGravity(Gravity.CENTER_VERTICAL);

        minus = button(context, "−", -1);
        plus = button(context, "+", 1);
        label = new MaterialTextView(context);
        label.setGravity(Gravity.CENTER);
        label.setTextAppearance(
                com.google.android.material.R.style.TextAppearance_Material3_TitleMedium);

        addView(minus);
        addView(label, new LayoutParams(0, ViewGroup.LayoutParams.MATCH_PARENT, 1f));
        addView(plus);
        refresh();
    }

    private MaterialButton button(Context context, String text, int direction) {
        // A Material 3 icon button: round, the size the system dictates and with
        // its ripples on press. It carries text rather than an icon because "−"
        // and "+" are exactly that, a character.
        MaterialButton button =
                new MaterialButton(
                        context, null, com.google.android.material.R.attr.materialIconButtonStyle);
        button.setText(text);
        button.setOnClickListener(v -> nudge(direction));
        return button;
    }

    private void nudge(int direction) {
        double next = Math.max(minimum, Math.min(maximum, value + direction * step));
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
        // No decimals when the step is a whole number: "3" and not "3.0".
        if (step == Math.rint(step) && value == Math.rint(value)) {
            label.setText(String.valueOf((long) value));
        } else {
            label.setText(String.valueOf(value));
        }
        // A button that can do nothing is disabled, which is what any system
        // control does on reaching its limit.
        minus.setEnabled(value > minimum);
        plus.setEnabled(value < maximum);
    }
}
