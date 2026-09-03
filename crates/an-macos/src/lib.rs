//! The macOS host: it implements `HostRenderer` and `TextMeasurer` over
//! AppKit, and exposes the runtime to the Swift shell over FFI.
//!
//! It is the iOS host's sibling and not the watch's: AppKit is imperative and
//! `NSView` exists, so the core's tree is mirrored in a real view hierarchy,
//! with one view per mountable node and the frame written straight in. What
//! differs from UIKit is not the approach, it is three things about the
//! desktop, and all three are commented where they are dealt with:
//!
//! - **The window resizes live.** On a phone the viewport only changes on
//!   rotation, which happens once in a long while; here it changes sixty times
//!   a second while somebody drags a corner. See
//!   `ffi::an_runtime_set_viewport` and `AnRootView.layout()` in the shell.
//! - **There is a mouse, not fingers.** The gestures are AppKit's recognisers,
//!   which are not the same as UIKit's, and the swipe is not one of them: it
//!   is a loose event that arrives through the responder chain. See
//!   `flipped.rs`. What exists on top of fingers —the pointer hovering and its
//!   shape— is `events::HoverTarget` and the `cursor` prop. What still cannot
//!   be delivered is said at subscription time; see
//!   `support::unsupported_event`.
//! - **The menu belongs to the system and lives in the window, not in the
//!   tree.** The shell puts it there and it is not exposed to Angular: there
//!   is no primitive for it, and inventing one would mean changing
//!   `packages/primitives`. See `shells/macos/Sources/AppDelegate.swift`.
//!
//! Everything here runs on the main thread, just as on iOS: AppKit will have
//! it no other way and objc2's `MainThreadMarker` makes that explicit in the
//! type.

// The inventory of primitives touches no platform on purpose: that way it can
// be checked from `cargo test` on any machine, and `scripts/check-macos.sh`
// can read it without compiling anything.
pub mod support;

#[cfg(target_os = "macos")]
mod accessibility;
#[cfg(target_os = "macos")]
mod alert;
#[cfg(target_os = "macos")]
mod color;
#[cfg(target_os = "macos")]
mod controls;
#[cfg(target_os = "macos")]
mod events;
#[cfg(target_os = "macos")]
mod ffi;
#[cfg(target_os = "macos")]
mod flipped;
#[cfg(target_os = "macos")]
mod host;
#[cfg(target_os = "macos")]
mod icons;
#[cfg(target_os = "macos")]
mod images;
#[cfg(target_os = "macos")]
mod map;
#[cfg(target_os = "macos")]
mod measure;
#[cfg(target_os = "macos")]
mod modules;
#[cfg(target_os = "macos")]
mod video;
#[cfg(target_os = "macos")]
mod web;

#[cfg(target_os = "macos")]
pub use host::AppKitHost;
#[cfg(target_os = "macos")]
pub use measure::AppKitMeasurer;
