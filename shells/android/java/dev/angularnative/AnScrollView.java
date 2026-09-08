package dev.angularnative;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.RectF;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewConfiguration;
import android.view.ViewGroup;
import android.widget.FrameLayout;
import android.widget.HorizontalScrollView;
import android.widget.ScrollView;

/**
 * A scroll view with "pull to refresh", and with an axis.
 *
 * Android ships no refresh control in the platform: `SwipeRefreshLayout` lives
 * in AndroidX, which is a separate dependency. The downward drag while scrolled
 * to the very top is detected and an arc is drawn, which is what the system
 * itself does, so that `refreshing` means the same thing as the iOS
 * `UIRefreshControl`.
 *
 * The axis is the other thing UIKit gets for free and this does not: a
 * `ScrollView` scrolls downwards and a `HorizontalScrollView` sideways, and a
 * view cannot change class once it has been made. `[horizontal]` arrives as a
 * prop, which is always *after* the view was created, so the sideways one is
 * slipped in between this view and its content the moment it is asked for.
 * Swapping this view for another instead would mean rewriting the host's id
 * table, its listeners and whatever the parent had already been told, all for a
 * prop that is set once and never again.
 */
public final class AnScrollView extends ScrollView {

    public interface OnRefresh {
        void onRefresh();
    }

    /** The offset in pixels, on both axes at once: the template gets one event. */
    public interface OnScroll {
        void onScroll(int x, int y);
    }

    /** How far it has to be pulled, in dp, for it to count. */
    private static final float THRESHOLD_DP = 72;

    private final float density;
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF arc = new RectF();

    private OnRefresh listener;
    private OnScroll scrollListener;
    private boolean refreshing;
    /** Whether the finger moves the content. It keeps clipping either way. */
    private boolean scrollEnabled = true;
    /** Whether the scrollers may be drawn. Both axes are asked for together. */
    private boolean indicators = true;
    /**
     * The sideways half, made the first time `[horizontal]` asks for it and
     * kept afterwards: turning the axis back is rarer than turning it, and a
     * view that has been in the hierarchy costs nothing sitting detached.
     */
    private Sideways sideways;
    private boolean horizontal;
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
        if (sideways != null) {
            sideways.setScrollEnabled(enabled);
        }
    }

    /**
     * Which way the content may overflow.
     *
     * The core has already clamped the content to this view's own size on the
     * other axis, so the half that is not asked for has nothing to scroll and
     * stays out of the gesture's way on its own.
     */
    public void setHorizontal(boolean horizontal) {
        if (this.horizontal == horizontal) {
            return;
        }
        this.horizontal = horizontal;
        View content = content();
        if (content == null) {
            // The host adds the content right after creating this view, so this
            // is only reachable if that ever stops being true.
            return;
        }
        ViewGroup from = (ViewGroup) content.getParent();
        ViewGroup.LayoutParams params = content.getLayoutParams();
        from.removeView(content);
        if (horizontal) {
            if (sideways == null) {
                sideways = new Sideways(getContext());
                sideways.setScrollEnabled(scrollEnabled);
            }
            sideways.addView(content, params);
            addView(
                    sideways,
                    new FrameLayout.LayoutParams(
                            FrameLayout.LayoutParams.MATCH_PARENT,
                            FrameLayout.LayoutParams.WRAP_CONTENT));
        } else {
            removeView(sideways);
            addView(content, params);
        }
        applyIndicators();
        applyScrollListener();
    }

    /** Whether the system draws the scrollers. */
    public void setIndicatorsShown(boolean shown) {
        this.indicators = shown;
        applyIndicators();
    }

    /**
     * One listener for the two halves.
     *
     * `View.setOnScrollChangeListener` reports the view it is on, and once the
     * content hangs off the sideways half there are two of them: x comes from
     * one and y from the other. The template asked for one `(scroll)`, so they
     * are put back together here.
     */
    public void setOnScroll(OnScroll listener) {
        this.scrollListener = listener;
        applyScrollListener();
    }

    private void applyIndicators() {
        setVerticalScrollBarEnabled(indicators && !horizontal);
        setHorizontalScrollBarEnabled(false);
        if (sideways != null) {
            sideways.setHorizontalScrollBarEnabled(indicators && horizontal);
            sideways.setVerticalScrollBarEnabled(false);
        }
    }

    private void applyScrollListener() {
        OnScroll listener = scrollListener;
        setOnScrollChangeListener(
                listener == null
                        ? null
                        : (View.OnScrollChangeListener)
                                (v, x, y, oldX, oldY) -> listener.onScroll(sidewaysOffset(), y));
        if (sideways != null) {
            sideways.setOnScrollChangeListener(
                    listener == null
                            ? null
                            : (View.OnScrollChangeListener)
                                    (v, x, y, oldX, oldY) -> listener.onScroll(x, getScrollY()));
        }
    }

    private int sidewaysOffset() {
        return sideways == null ? 0 : sideways.getScrollX();
    }

    /** The single child the host mounts, wherever the axis has left it. */
    private View content() {
        if (sideways != null && sideways.getChildCount() > 0) {
            return sideways.getChildAt(0);
        }
        View first = getChildCount() > 0 ? getChildAt(0) : null;
        return first == sideways ? null : first;
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
            ViewConfiguration configuration = ViewConfiguration.get(getContext());
            // `scrollBy` does not clamp on its own; without the cap, the crown
            // keeps "scrolling" a list that has already ended and the `scroll`
            // event that comes out of here would report a scroll that never
            // happened.
            if (horizontal && sideways != null) {
                // The crown is the only way through a list on the watch, and a
                // sideways one is still a list: the detents go to whichever
                // half can move, or the strip would simply not answer it.
                float pixels = -notches * configuration.getScaledHorizontalScrollFactor();
                View content = sideways.getChildCount() > 0 ? sideways.getChildAt(0) : null;
                int width = content == null ? 0 : content.getWidth();
                int max = Math.max(0, width - sideways.getWidth());
                int at = sideways.getScrollX();
                int target = Math.min(max, Math.max(0, at + Math.round(pixels)));
                if (target != at) {
                    sideways.scrollTo(target, 0);
                }
                return true;
            }
            float pixels = -notches * configuration.getScaledVerticalScrollFactor();
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
     * The sideways half. It exists only to answer `scrollEnabled`, which
     * `HorizontalScrollView` has no more of a switch for than `ScrollView`
     * does: what there is is deciding whether to intercept the drag.
     */
    private static final class Sideways extends HorizontalScrollView {

        private boolean scrollEnabled = true;

        Sideways(Context context) {
            super(context);
        }

        void setScrollEnabled(boolean enabled) {
            this.scrollEnabled = enabled;
        }

        @Override
        public boolean onInterceptTouchEvent(MotionEvent event) {
            return scrollEnabled && super.onInterceptTouchEvent(event);
        }

        @Override
        public boolean onTouchEvent(MotionEvent event) {
            return scrollEnabled && super.onTouchEvent(event);
        }
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
