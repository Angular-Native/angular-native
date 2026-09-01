package dev.angularnative;

import android.content.Context;
import android.content.res.ColorStateList;
import android.graphics.Color;
import android.view.ViewGroup;
import com.google.android.material.button.MaterialButton;
import com.google.android.material.button.MaterialButtonToggleGroup;

/**
 * Control segmentado.
 *
 * Es el «segmented button» de Material 3 tal cual: un
 * `MaterialButtonToggleGroup` con botones marcables dentro. La forma de los
 * extremos, el contenedor del elegido, la marca de verificación y la respuesta
 * al pulsar las pone la librería, no este fichero.
 */
public final class AnSegmentedControl extends MaterialButtonToggleGroup {

    public interface OnSelected {
        void onSelected(int index);
    }

    private OnSelected listener;
    /** Para no avisar del segmento que se acaba de fijar desde la plantilla. */
    private boolean fijando;
    private Integer activeColor;

    public AnSegmentedControl(Context context) {
        super(context);
        setSingleSelection(true);
        // Siempre hay uno elegido: un segmentado sin nada marcado no representa
        // ningún estado, y al volver de la plantilla habría que adivinar cuál.
        setSelectionRequired(true);
        addOnButtonCheckedListener(
                (group, checkedId, isChecked) -> {
                    if (isChecked && !fijando && listener != null) {
                        listener.onSelected(checkedId - 1);
                    }
                });
    }

    public void setListener(OnSelected listener) {
        this.listener = listener;
    }

    public void setActiveColor(int color) {
        this.activeColor = color;
        for (int index = 0; index < getChildCount(); index++) {
            tint((MaterialButton) getChildAt(index));
        }
    }

    public void setItems(String[] items) {
        removeAllViews();
        for (int index = 0; index < items.length; index++) {
            // El estilo va por atributo del tema: así el botón sale con la
            // tipografía, la altura y el trazo que Material 3 le da a un
            // segmento, en vez de con los de un botón suelto.
            MaterialButton segment =
                    new MaterialButton(
                            getContext(),
                            null,
                            com.google.android.material.R.attr.materialButtonOutlinedStyle);
            segment.setText(items[index]);
            segment.setCheckable(true);
            // Los identificadores empiezan en 1: el 0 es `View.NO_ID` y el grupo
            // no sabría de qué segmento le hablan.
            segment.setId(index + 1);
            tint(segment);
            addView(segment, new LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
        }
    }

    public void setSelectedIndex(int index) {
        if (index < 0 || index >= getChildCount()) {
            return;
        }
        fijando = true;
        check(index + 1);
        fijando = false;
    }

    /**
     * Tiñe el segmento con el color de la app.
     *
     * Solo el relleno del elegido y el texto: el resto —trazo, forma, ondas al
     * pulsar— se queda como lo pinta Material.
     */
    private void tint(MaterialButton segment) {
        if (activeColor == null) {
            return;
        }
        int[][] estados = {new int[] {android.R.attr.state_checked}, new int[] {}};
        segment.setBackgroundTintList(
                new ColorStateList(estados, new int[] {activeColor, Color.TRANSPARENT}));
        segment.setTextColor(new ColorStateList(estados, new int[] {contrast(activeColor), activeColor}));
    }

    /** Blanco o negro, el que se lea sobre ese color. */
    private static int contrast(int color) {
        double luz =
                (0.299 * Color.red(color) + 0.587 * Color.green(color) + 0.114 * Color.blue(color))
                        / 255.0;
        return luz > 0.6 ? Color.BLACK : Color.WHITE;
    }
}
