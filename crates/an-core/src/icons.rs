//! Common icon names, translated into the SF Symbol each one maps to.
//!
//! It lives in the core for the same reason the color table does: with more
//! than one Apple host —iOS, tvOS, visionOS and the watch— two copies would be
//! two places where `back` could stop being `chevron.left`, and an icon that
//! changes drawing depending on the screen is the kind of bug nobody sees until
//! a user sees it.
//!
//! The list is deliberately short: it covers what almost any app carries —a tab
//! bar, a header— and for anything else you write the symbol's name directly.
//! There are more than five thousand of them and duplicating them here would
//! make no sense.

/// The SF Symbol a common name maps to. Anything not in the table comes out
/// untouched: it is the symbol's native name, which the template is free to
/// write directly.
pub fn translate(name: &str) -> &str {
    match name {
        "home" => "house.fill",
        "search" => "magnifyingglass",
        "settings" => "gearshape.fill",
        "profile" | "account" => "person.crop.circle.fill",
        "back" => "chevron.left",
        "forward" => "chevron.right",
        "close" => "xmark",
        "add" => "plus",
        "remove" => "minus",
        "delete" => "trash",
        "edit" => "pencil",
        "share" => "square.and.arrow.up",
        "favorite" => "heart.fill",
        "star" => "star.fill",
        "menu" => "line.3.horizontal",
        "more" => "ellipsis",
        "check" => "checkmark",
        "info" => "info.circle",
        "warning" => "exclamationmark.triangle.fill",
        "refresh" => "arrow.clockwise",
        "calendar" => "calendar",
        "camera" => "camera.fill",
        "bell" => "bell.fill",
        "chat" => "bubble.left.fill",
        "mail" => "envelope.fill",
        "list" => "list.bullet",
        "play" => "play.fill",
        "pause" => "pause.fill",
        "download" => "arrow.down.circle",
        "upload" => "arrow.up.circle",
        "location" => "location.fill",
        "lock" => "lock.fill",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn common_names_are_translated_and_native_ones_pass_through() {
        assert_eq!(super::translate("back"), "chevron.left");
        assert_eq!(super::translate("account"), "person.crop.circle.fill");
        // An SF Symbol written out by hand is left alone: the table is not an
        // allowlist, it is a shortcut.
        assert_eq!(super::translate("square.and.arrow.up"), "square.and.arrow.up");
        assert_eq!(super::translate("figure.run"), "figure.run");
    }
}
