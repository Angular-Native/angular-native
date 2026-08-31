package dev.angularnative;

import android.content.Context;
import android.view.View;
import android.view.ViewGroup;

/**
 * Contenedor que no calcula nada.
 *
 * El layout ya lo resolvió taffy; aquí solo hay que aplicar el marco que llega
 * de Rust. Dejar que Android midiera por su cuenta sería tener dos motores de
 * layout peleándose, igual que pasaría con Auto Layout en iOS.
 */
public final class AnViewGroup extends ViewGroup {

    /**
     * Marco en píxeles, ya convertido desde los puntos que usa el core.
     *
     * Hereda de `MarginLayoutParams` y no de `LayoutParams` a secas porque un
     * `ScrollView` es un `FrameLayout` por dentro y mide a sus hijos con
     * `measureChildWithMargins`: con los otros, la app se cae al montar el
     * primer scroll. El tamaño va en los campos `width`/`height` heredados,
     * que es lo que esos contenedores leen.
     */
    public static final class Frame extends ViewGroup.MarginLayoutParams {
        public int left;
        public int top;

        public Frame() {
            super(WRAP_CONTENT, WRAP_CONTENT);
        }
    }

    public AnViewGroup(Context context) {
        super(context);
        // Recortar por defecto. Sin esto, el contenido de un ScrollView se
        // dibuja por encima de lo que tiene alrededor: los hijos van en
        // posición absoluta y pueden quedar muy fuera de su contenedor.
        setClipChildren(true);
    }

    @Override
    protected void onMeasure(int widthSpec, int heightSpec) {
        int contentWidth = 0;
        int contentHeight = 0;
        for (int i = 0; i < getChildCount(); i++) {
            View child = getChildAt(i);
            ViewGroup.LayoutParams params = child.getLayoutParams();
            if (!(params instanceof Frame)) {
                continue;
            }
            Frame frame = (Frame) params;
            child.measure(
                    MeasureSpec.makeMeasureSpec(params.width, MeasureSpec.EXACTLY),
                    MeasureSpec.makeMeasureSpec(params.height, MeasureSpec.EXACTLY));
            contentWidth = Math.max(contentWidth, frame.left + params.width);
            contentHeight = Math.max(contentHeight, frame.top + params.height);
        }
        // Un ScrollView mide siempre a su hijo con altura UNSPECIFIED: si aquí
        // se devolviera el mínimo sugerido, el contenido quedaría en cero y no
        // se vería nada. Con el tamaño que ocupan los hijos, `resolveSize`
        // devuelve eso cuando no hay restricción y respeta la medida exacta
        // cuando sí la hay.
        setMeasuredDimension(
                resolveSize(Math.max(contentWidth, getSuggestedMinimumWidth()), widthSpec),
                resolveSize(Math.max(contentHeight, getSuggestedMinimumHeight()), heightSpec));
    }

    @Override
    protected void onLayout(boolean changed, int l, int t, int r, int b) {
        for (int i = 0; i < getChildCount(); i++) {
            View child = getChildAt(i);
            ViewGroup.LayoutParams params = child.getLayoutParams();
            if (!(params instanceof Frame)) {
                continue;
            }
            Frame frame = (Frame) params;
            child.layout(
                    frame.left, frame.top, frame.left + params.width, frame.top + params.height);
        }
    }

    @Override
    protected boolean checkLayoutParams(ViewGroup.LayoutParams params) {
        return params instanceof Frame;
    }

    @Override
    protected ViewGroup.LayoutParams generateDefaultLayoutParams() {
        return new Frame();
    }
}
