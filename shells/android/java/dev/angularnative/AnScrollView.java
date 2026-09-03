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
 * A scroll view with "pull to refresh".
 *
 * Android ships none in the platform: `SwipeRefreshLayout` lives in AndroidX,
 * which is a separate dependency. The downward drag while scrolled to the very
 * top is detected and an arc is drawn, which is what the system itself does, so
 * that `refreshing` means the same thing as the iOS `UIRefreshControl`.
 */
public final class AnScrollView extends ScrollView {

    public interface OnRefresh {
        void onRefresh();
    }

    /** How far it has to be pulled, in dp, for it to count. */
    private static final float THRESHOLD_DP = 72;

    private final float density;
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF arc = new RectF();

    private OnRefresh listener;
    private boolean refreshing;
    /** Whether the finger moves the content. It keeps clipping either way. */
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
     * Lets the watch crown scroll this view.
     *
     * Crown events reach the view that has the focus, and a list does not ask
     * for it on its own: without `setFocusableInTouchMode` the system sends them
     * to whoever does have it —usually nobody— and the crown does nothing, with
     * no error at all.
     *
     * It is only turned on on the watch, and not always: on a phone, a
     * scrollable view that takes the focus when touched takes it away from the
     * `EditText` underneath, and that is very noticeable.
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
     * The rotary crown.
     *
     * It is not a touch: it arrives as `ACTION_SCROLL` from
     * `SOURCE_ROTARY_ENCODER`, along the generic event path and not the touch
     * one, which is why `onTouchEvent` never saw it. The `AXIS_SCROLL` value is
     * in wheel detents, not pixels: what converts them is the system's scroll
     * factor, the same one a mouse uses.
     *
     * The sign is inverted because turning the crown upwards returns positive
     * values and going down the list means increasing `scrollY`.
     */
    @Override
    public boolean onGenericMotionEvent(MotionEvent event) {
        if (rotary
                && event.getAction() == MotionEvent.ACTION_SCROLL
                && event.isFromSource(InputDevice.SOURCE_ROTARY_ENCODER)) {
            float notches = event.getAxisValue(MotionEvent.AXIS_SCROLL);
            float pixels =
                    -notches * ViewConfiguration.get(getContext()).getScaledVerticalScrollFactor();
            // `scrollBy` does not clamp on its own; without the cap, the crown
            // keeps "scrolling" a list that has already ended and the `scroll`
            // event that comes out of here would report a scroll that never
            // happened.
            int max = Math.max(0, contentHeight() - getHeight());
            int target = Math.min(max, Math.max(0, getScrollY() + Math.round(pixels)));
            if (target != getScrollY()) {
                scrollTo(0, target);
            }
            return true;
        }
        return super.onGenericMotionEvent(event);
    }

    private int contentHeight() {
        return getChildCount() > 0 ? getChildAt(0).getHeight() : 0;
    }

    /**
     * With no gesture, the `ScrollView` does not even get to look at the touch.
     *
     * Android has no `setScrollEnabled` like UIKit's: what there is is deciding
     * whether to intercept the drag, so it is said here and the touch carries on
     * its way down to the children.
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
        // While being pulled, the arc grows with the finger; once refreshing, it spins.
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
