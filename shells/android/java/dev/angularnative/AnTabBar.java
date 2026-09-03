package dev.angularnative;

import android.content.Context;
import android.graphics.drawable.Drawable;
import android.view.Menu;
import android.view.MenuItem;

import com.google.android.material.bottomnavigation.BottomNavigationView;

/**
 * Tab bar.
 *
 * It is Material 3's `BottomNavigationView`, the real one: the Android platform
 * ships none —`android.widget` stopped at the tabs of 2011— and Material's lives
 * in a separate library, which this build resolves by hand because it does not
 * use Gradle.
 *
 * From it come the pill indicator behind the selected icon, the animation on
 * switching, the behaviour with TalkBack and whatever height it is due on each
 * version of the system.
 */
public final class AnTabBar extends BottomNavigationView {

    public interface OnTabSelected {
        void onSelected(int index);
    }

    /** Resolves an icon name to a drawable. */
    public interface IconResolver {
        Drawable resolve(String name);
    }

    private OnTabSelected listener;
    private IconResolver iconResolver;
    private String[] titles = new String[0];
    private String[] icons = new String[0];
    /** So the tab just set from the template is not reported back. */
    private boolean setting;
    /** The bar's two colours, which arrive in separate props. */
    private Integer activeColor;
    private Integer inactiveColor;

    public AnTabBar(Context context) {
        super(context);
        // Always labelled, like the bar in the Material 3 photograph. The
        // automatic mode hides them as soon as there are more than three tabs.
        setLabelVisibilityMode(LABEL_VISIBILITY_LABELED);
        setOnItemSelectedListener(
                item -> {
                    if (!setting && listener != null) {
                        listener.onSelected(item.getItemId() - 1);
                    }
                    return true;
                });
    }

    public void setListener(OnTabSelected listener) {
        this.listener = listener;
    }

    public void setIconResolver(IconResolver resolver) {
        this.iconResolver = resolver;
        rebuild();
    }

    public void setTitles(String[] titles) {
        this.titles = titles;
        rebuild();
    }

    /** Icons, in the same order as the titles. */
    public void setIcons(String[] icons) {
        this.icons = icons;
        rebuild();
    }

    public void setActiveColor(int color) {
        this.activeColor = color;
        applyColors();
    }

    /** Colour of the tabs that are not selected. */
    public void setInactiveColor(int color) {
        this.inactiveColor = color;
        applyColors();
    }

    /**
     * The colour of the selected tab and that of the rest, which arrive apart.
     *
     * With nothing said, the inactive one is the active one dimmed. It used to
     * come from whatever tint the icon already carried, and that left them white
     * on the light background of the bar: the labels were there, in the colour
     * of the background.
     */
    private void applyColors() {
        if (activeColor == null && inactiveColor == null) {
            return;
        }
        int active = activeColor == null ? inactiveColor : activeColor;
        int dimmed =
                inactiveColor != null
                        ? inactiveColor
                        : android.graphics.Color.argb(
                                150,
                                android.graphics.Color.red(active),
                                android.graphics.Color.green(active),
                                android.graphics.Color.blue(active));
        android.content.res.ColorStateList list =
                new android.content.res.ColorStateList(
                        new int[][] {new int[] {android.R.attr.state_checked}, new int[] {}},
                        new int[] {active, dimmed});
        setItemIconTintList(list);
        setItemTextColor(list);
    }

    public void setSelectedIndex(int index) {
        if (index < 0 || index >= getMenu().size()) {
            return;
        }
        // Setting it from the template is not selecting it: reporting here would
        // send the event back to whoever has just caused it.
        setting = true;
        setSelectedItemId(index + 1);
        setting = false;
    }

    private void rebuild() {
        Menu menu = getMenu();
        menu.clear();
        for (int index = 0; index < titles.length; index++) {
            // The identifiers start at 1: 0 is `Menu.NONE` and the bar treats it
            // as "no item".
            MenuItem item = menu.add(Menu.NONE, index + 1, index, titles[index]);
            if (iconResolver != null && index < icons.length) {
                Drawable icon = iconResolver.resolve(icons[index]);
                if (icon != null) {
                    item.setIcon(icon);
                }
            }
        }
        setLabelVisibilityMode(LABEL_VISIBILITY_LABELED);
    }

    /**
     * The bar's height. Material decides it, not us: it is asked by measuring,
     * which is what any Android layout does.
     */
    public int heightPx() {
        measure(
                MeasureSpec.makeMeasureSpec(0, MeasureSpec.UNSPECIFIED),
                MeasureSpec.makeMeasureSpec(0, MeasureSpec.UNSPECIFIED));
        return getMeasuredHeight();
    }
}
