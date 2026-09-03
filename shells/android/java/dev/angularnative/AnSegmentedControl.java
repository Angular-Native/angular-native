package dev.angularnative;

import android.content.Context;
import android.content.res.ColorStateList;
import android.graphics.Color;
import android.view.ViewGroup;
import com.google.android.material.button.MaterialButton;
import com.google.android.material.button.MaterialButtonToggleGroup;

/**
 * Segmented control.
 *
 * It is the Material 3 "segmented button" as it comes: a
 * `MaterialButtonToggleGroup` with checkable buttons inside. The shape of the
 * end segments, the container of the selected one, the check mark and the
 * response to a press are all put there by the library, not by this file.
 */
public final class AnSegmentedControl extends MaterialButtonToggleGroup {

    public interface OnSelected {
        void onSelected(int index);
    }

    private OnSelected listener;
    /** So the segment just set from the template is not reported back. */
    private boolean setting;
    private Integer activeColor;

    public AnSegmentedControl(Context context) {
        super(context);
        setSingleSelection(true);
        // There is always one selected: a segmented control with nothing checked
        // represents no state at all, and coming back from the template one
        // would have to guess which.
        setSelectionRequired(true);
        addOnButtonCheckedListener(
                (group, checkedId, isChecked) -> {
                    if (isChecked && !setting && listener != null) {
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
            // The style goes through a theme attribute: that way the button
            // comes out with the typography, the height and the stroke Material
            // 3 gives a segment, rather than those of a standalone button.
            MaterialButton segment =
                    new MaterialButton(
                            getContext(),
                            null,
                            com.google.android.material.R.attr.materialButtonOutlinedStyle);
            segment.setText(items[index]);
            segment.setCheckable(true);
            // The identifiers start at 1: 0 is `View.NO_ID` and the group would
            // not know which segment it was being told about.
            segment.setId(index + 1);
            tint(segment);
            addView(segment, new LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
        }
    }

    public void setSelectedIndex(int index) {
        if (index < 0 || index >= getChildCount()) {
            return;
        }
        setting = true;
        check(index + 1);
        setting = false;
    }

    /**
     * Tints the segment with the app's colour.
     *
     * Only the fill of the selected one and the text: the rest —stroke, shape,
     * ripples on press— stays as Material paints it.
     */
    private void tint(MaterialButton segment) {
        if (activeColor == null) {
            return;
        }
        int[][] states = {new int[] {android.R.attr.state_checked}, new int[] {}};
        segment.setBackgroundTintList(
                new ColorStateList(states, new int[] {activeColor, Color.TRANSPARENT}));
        segment.setTextColor(new ColorStateList(states, new int[] {contrast(activeColor), activeColor}));
    }

    /** Black or white, whichever reads on that colour. */
    private static int contrast(int color) {
        double light =
                (0.299 * Color.red(color) + 0.587 * Color.green(color) + 0.114 * Color.blue(color))
                        / 255.0;
        return light > 0.6 ? Color.BLACK : Color.WHITE;
    }
}
