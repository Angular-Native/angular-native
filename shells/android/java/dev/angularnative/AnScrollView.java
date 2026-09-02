package dev.angularnative;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.RectF;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.ViewConfiguration;
import android.widget.ScrollView;

/**
 * Scroll con «tirar para recargar».
 *
 * Android no trae uno en la plataforma: `SwipeRefreshLayout` vive en AndroidX,
 * que es una dependencia aparte. Se detecta el arrastre hacia abajo estando
 * arriba del todo y se dibuja un arco, que es lo que hace el propio sistema,
 * para que `refreshing` signifique lo mismo que el `UIRefreshControl` de iOS.
 */
public final class AnScrollView extends ScrollView {

    public interface OnRefresh {
        void onRefresh();
    }

    /** Cuánto hay que tirar, en dp, para que cuente. */
    private static final float THRESHOLD_DP = 72;

    private final float density;
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF arc = new RectF();

    private OnRefresh listener;
    private boolean refreshing;
    /** Si el dedo mueve el contenido. Sigue recortando igual. */
    private boolean scrollEnabled = true;
    private float startY = Float.NaN;
    private float pull;
    private float spin;

    public AnScrollView(Context context) {
        super(context);
        this.density = context.getResources().getDisplayMetrics().density;
        paint.setStyle(Paint.Style.STROKE);
        paint.setStrokeWidth(3 * density);
        paint.setStrokeCap(Paint.Cap.ROUND);
        paint.setColor(Color.WHITE);
    }

    public void setOnRefresh(OnRefresh listener) {
        this.listener = listener;
    }

    public void setRefreshing(boolean refreshing) {
        this.refreshing = refreshing;
        if (!refreshing) {
            pull = 0;
        }
        invalidate();
    }

    public void setScrollEnabled(boolean enabled) {
        this.scrollEnabled = enabled;
    }

    /**
     * Deja que la corona del reloj desplace esta vista.
     *
     * Los eventos de la corona llegan a la vista que tiene el foco, y una lista
     * no lo pide sola: sin `setFocusableInTouchMode` el sistema los manda a
     * quien sea que lo tenga —normalmente a nadie— y la corona no hace nada,
     * sin error ninguno.
     *
     * Se enciende solo en el reloj y no siempre: en un teléfono, una vista
     * scrollable que pide el foco al tocarla se lo quita al `EditText` que
     * hubiera debajo, y eso sí se nota.
     */
    public void enableRotary() {
        rotary = true;
        setFocusable(true);
        setFocusableInTouchMode(true);
    }

    private boolean rotary;

    @Override
    protected void onAttachedToWindow() {
        super.onAttachedToWindow();
        if (rotary) {
            requestFocus();
        }
    }

    /**
     * La corona giratoria.
     *
     * No es un toque: llega como `ACTION_SCROLL` desde `SOURCE_ROTARY_ENCODER`,
     * por el camino de los eventos genéricos y no por el de los táctiles, que
     * es por lo que `onTouchEvent` nunca la vio. El valor de `AXIS_SCROLL` va
     * en muescas de rueda, no en píxeles: lo que las convierte es el factor de
     * desplazamiento del sistema, el mismo que usa un ratón.
     *
     * El signo se invierte porque la corona hacia arriba devuelve valores
     * positivos y bajar por la lista es aumentar `scrollY`.
     */
    @Override
    public boolean onGenericMotionEvent(MotionEvent event) {
        if (rotary
                && event.getAction() == MotionEvent.ACTION_SCROLL
                && event.isFromSource(InputDevice.SOURCE_ROTARY_ENCODER)) {
            float notches = event.getAxisValue(MotionEvent.AXIS_SCROLL);
            float pixels =
                    -notches * ViewConfiguration.get(getContext()).getScaledVerticalScrollFactor();
            // `scrollBy` no recorta por sí solo; sin el tope, la corona sigue
            // «desplazando» una lista que ya se acabó y el evento `scroll` que
            // sale de aquí contaría un desplazamiento que no ocurrió.
            int max = Math.max(0, contentHeight() - getHeight());
            int destino = Math.min(max, Math.max(0, getScrollY() + Math.round(pixels)));
            if (destino != getScrollY()) {
                scrollTo(0, destino);
            }
            return true;
        }
        return super.onGenericMotionEvent(event);
    }

    private int contentHeight() {
        return getChildCount() > 0 ? getChildAt(0).getHeight() : 0;
    }

    /**
     * Sin gesto, el `ScrollView` no llega ni a mirar el toque.
     *
     * Android no tiene un `setScrollEnabled` como el de UIKit: lo que hay es
     * decidir si se intercepta el arrastre, así que se dice aquí y el toque
     * sigue su camino hacia los hijos.
     */
    @Override
    public boolean onInterceptTouchEvent(MotionEvent event) {
        return scrollEnabled && super.onInterceptTouchEvent(event);
    }

    @Override
    public boolean onTouchEvent(MotionEvent event) {
        if (!scrollEnabled) {
            return false;
        }
        if (listener != null) {
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN:
                    startY = getScrollY() == 0 ? event.getY() : Float.NaN;
                    break;
                case MotionEvent.ACTION_MOVE:
                    if (!Float.isNaN(startY) && getScrollY() == 0) {
                        pull = Math.max(0, event.getY() - startY);
                        invalidate();
                    }
                    break;
                case MotionEvent.ACTION_UP:
                case MotionEvent.ACTION_CANCEL:
                    if (!refreshing && pull > THRESHOLD_DP * density) {
                        refreshing = true;
                        listener.onRefresh();
                    }
                    if (!refreshing) {
                        pull = 0;
                    }
                    startY = Float.NaN;
                    invalidate();
                    break;
                default:
                    break;
            }
        }
        return super.onTouchEvent(event);
    }

    @Override
    protected void dispatchDraw(Canvas canvas) {
        super.dispatchDraw(canvas);
        if (!refreshing && pull <= 0) {
            return;
        }
        // Mientras se tira, el arco crece con el dedo; una vez recargando, gira.
        float radius = 10 * density;
        float centerX = getWidth() / 2f;
        float centerY = getScrollY() + 28 * density;
        arc.set(centerX - radius, centerY - radius, centerX + radius, centerY + radius);

        float sweep;
        float start;
        if (refreshing) {
            spin = (spin + 8) % 360;
            start = spin;
            sweep = 270;
            postInvalidateOnAnimation();
        } else {
            start = -90;
            sweep = Math.min(1f, pull / (THRESHOLD_DP * density)) * 330;
        }
        paint.setAlpha(refreshing ? 255 : Math.round(Math.min(1f, pull / (THRESHOLD_DP * density)) * 255));
        canvas.drawArc(arc, start, sweep, false, paint);
    }
}
