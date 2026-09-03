package dev.angularnative;

import android.content.Context;
import android.view.View;
import android.view.ViewGroup;

/**
 * A container that computes nothing.
 *
 * The layout has already been resolved by taffy; all that is needed here is to
 * apply the frame that arrives from Rust. Letting Android measure on its own
 * would mean two layout engines fighting each other, just as would happen with
 * Auto Layout on iOS.
 */
public final class AnViewGroup extends ViewGroup {

    /**
     * A frame in pixels, already converted from the points the core uses.
     *
     * It extends `MarginLayoutParams` and not plain `LayoutParams` because a
     * `ScrollView` is a `FrameLayout` underneath and measures its children with
     * `measureChildWithMargins`: with the others, the app crashes on mounting
     * the first scroll view. The size goes in the inherited `width`/`height`
     * fields, which is what those containers read.
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
        // Clip by default. Without this, the contents of a ScrollView are drawn
        // over whatever surrounds it: the children are absolutely positioned and
        // can end up well outside their container.
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
        // A ScrollView always measures its child with an UNSPECIFIED height: if
        // the suggested minimum were returned here, the content would come out
        // at zero and nothing would show. Given the size the children take up,
        // `resolveSize` returns that when there is no constraint and honours the
        // exact measurement when there is one.
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
