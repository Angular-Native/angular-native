//! The system's controls: switch, slider, indicators, button and tab bar.
//!
//! How big they are is not the framework's to decide. A `UISwitch` does not
//! measure the same on iOS 17 as on iOS 26, nor does it at the large
//! accessibility text size. Each control is asked once, at startup and on the
//! main thread, because creating them off it is not allowed.

use std::collections::HashMap;

use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_core_foundation::CGSize;
use objc2_ui_kit::{UIActivityIndicatorView, UIButton, UIProgressView, UITabBar, UIView};
// tvOS does not have them, and a `use` of something that will not be used is
// a warning.
#[cfg(not(target_os = "tvos"))]
use objc2_ui_kit::{UISlider, UISwitch};

/// Natural sizes, in points, indexed by the control's name.
pub type ControlSizes = HashMap<String, (f32, f32)>;

/// The point size an `<an-icon>` is drawn at when the template says nothing.
///
/// It has to match `Icon.size`'s default in `packages/primitives`, because that
/// is where it comes from on the way in: the directive pins the box's width and
/// height with it, so this only ever decides the measurement of an icon that
/// somehow arrived with neither.
const DEFAULT_ICON_POINTS: f32 = 24.0;

/// Asks each control how much room it takes. Called once, at startup.
pub fn measure_controls(mtm: MainThreadMarker) -> ControlSizes {
    // An infinite width so each one gives its natural size with nothing
    // clipping it.
    let unbounded = CGSize { width: f64::MAX / 2.0, height: f64::MAX / 2.0 };
    let mut sizes = ControlSizes::new();

    let mut record = |name: &str, view: &UIView| {
        let fitted = view.sizeThatFits(unbounded);
        sizes.insert(name.to_owned(), (fitted.width as f32, fitted.height as f32));
    };

    // The ones tvOS does not have are skipped altogether: asking for the size
    // of a class that does not exist is not a measurement that comes out
    // wrong, it is an `objc_getClass` returning null and a process quitting.
    // With no entry in the table, `measure_control` returns (0, 0) and the gap
    // shows on the screen, which is what a control that is not there deserves.
    #[cfg(not(target_os = "tvos"))]
    {
        record("Switch", &UISwitch::new(mtm));
        record("Slider", &UISlider::new(mtm));
        record("Stepper", &objc2_ui_kit::UIStepper::new(mtm));
        record("DatePicker", &objc2_ui_kit::UIDatePicker::new(mtm));
    }
    record("ActivityIndicator", &UIActivityIndicatorView::new(mtm));
    record("ProgressBar", &UIProgressView::new(mtm));
    record("Button", &UIButton::new(mtm));
    record("TabBar", &UITabBar::new(mtm));
    record("SegmentedControl", &objc2_ui_kit::UISegmentedControl::new(mtm));
    record("SearchBar", &objc2_ui_kit::UISearchBar::new(mtm));
    // The drop-down is a button with a menu: it measures what a button
    // measures.
    record("Picker", &UIButton::new(mtm));
    // The header. It is the same `UINavigationBar` the host mounts, and it
    // knows its own height —44 on a phone, more on an iPad, and different
    // again at the large accessibility sizes—, which is why it is asked and
    // not written down. Without this line it was not in the table at all, and
    // an `<an-navigation-bar>` with no explicit height was laid out 0 points
    // tall: the title, the back button and everything in it went off the
    // screen with nothing said.
    record("NavigationBar", &objc2_ui_kit::UINavigationBar::new(mtm));
    // The icon, in the `UIImageView` the host mounts for it.
    //
    // The point size is the one number here that cannot be asked of the
    // system: an SF Symbol has no natural size —it is drawn at whatever size
    // its configuration says—, so something has to say which one. 24 is
    // `<an-icon>`'s default `[size]`, declared in `packages/primitives`, and
    // the template overrides it by pinning width and height whenever it writes
    // `[size]`. What the system does decide is everything else: the aspect
    // that symbol comes out with at that size and what the image view keeps
    // around it.
    let icon = objc2_ui_kit::UIImageView::new(mtm);
    icon.setImage(crate::icons::symbol("home", DEFAULT_ICON_POINTS, 400).as_deref());
    record("Icon", &icon);

    // Some of them return zero from `sizeThatFits` because they have no
    // content yet; for those the known natural size wins.
    for (name, fallback) in [
        #[cfg(not(target_os = "tvos"))]
        ("Slider", (200.0, 32.0)),
        ("ProgressBar", (200.0, 4.0)),
        ("Button", (80.0, 44.0)),
        // A segmented control with no segments and a search bar with no text
        // measure nothing useful: until they have content their known size
        // wins.
        ("SegmentedControl", (320.0, 32.0)),
        ("SearchBar", (320.0, 56.0)),
        #[cfg(not(target_os = "tvos"))]
        ("Stepper", (94.0, 32.0)),
        ("Picker", (140.0, 44.0)),
        #[cfg(not(target_os = "tvos"))]
        ("DatePicker", (200.0, 44.0)),
        ("NavigationBar", (320.0, 44.0)),
        ("Icon", (DEFAULT_ICON_POINTS, DEFAULT_ICON_POINTS)),
    ] {
        let entry = sizes.entry(name.to_owned()).or_insert(fallback);
        if entry.0 <= 0.0 {
            entry.0 = fallback.0;
        }
        if entry.1 <= 0.0 {
            entry.1 = fallback.1;
        }
    }
    sizes
}

/// A `UITabBar` with no items measures nothing useful, and having them means
/// building them.
pub fn tab_bar_items(
    mtm: MainThreadMarker,
    titles: &[String],
    icons: &[String],
) -> Retained<objc2_foundation::NSArray<objc2_ui_kit::UITabBarItem>> {
    use objc2_foundation::{NSArray, NSString};
    use objc2_ui_kit::UITabBarItem;

    let items: Vec<Retained<UITabBarItem>> = titles
        .iter()
        .enumerate()
        .map(|(index, title)| {
            // The icon carries no size: in a tab bar UIKit picks it, and
            // forcing it here would be fighting the bar.
            let image = icons.get(index).and_then(|name| crate::icons::symbol(name, 0.0, 400));
            unsafe {
                UITabBarItem::initWithTitle_image_tag(
                    mtm.alloc::<UITabBarItem>(),
                    Some(&NSString::from_str(title)),
                    image.as_deref(),
                    index as isize,
                )
            }
        })
        .collect();
    NSArray::from_retained_slice(&items)
}
