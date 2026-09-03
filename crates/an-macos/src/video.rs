//! Video: an `AVPlayerView` with an `AVPlayer` inside it.
//!
//! Here the desktop works out cheaper than the phone, and not by a little. In
//! UIKit there is no video view at all: there is an `AVPlayerViewController`,
//! and putting it in the tree means making it a child of the presiding
//! controller and repositioning its view by hand on every frame, because it
//! comes into being after the frame has been set. In AppKit there is one
//! —`AVPlayerView` **is** an `NSView`— so the player goes into the tree like
//! any other view, system controls and all, with nothing to keep in sync.
//!
//! The classes are declared by hand for the same reason as in `web.rs` and
//! `map.rs`: pulling in `objc2-av-kit` and `objc2-av-foundation` for six
//! methods puts two whole frameworks' worth of generated classes into every
//! build of the host. `objc2` checks each signature against the real one when
//! the message is sent.

use objc2::rc::Retained;
use objc2::{extern_class, extern_methods, ClassType, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSResponder, NSView};
use objc2_foundation::{NSObject, NSURL};

#[link(name = "AVFoundation", kind = "framework")]
unsafe extern "C" {}

#[link(name = "AVKit", kind = "framework")]
unsafe extern "C" {}

/// The system's controls, pinned along the bottom inside the view.
/// `AVPlayerViewControlsStyleInline`.
pub const CONTROLS_INLINE: isize = 1;

extern_class!(
    /// AppKit's video view: picture and playback controls.
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct AVPlayerView;
);

impl AVPlayerView {
    pub fn new(_mtm: MainThreadMarker) -> Retained<Self> {
        unsafe { objc2::msg_send![<Self as ClassType>::class(), new] }
    }

    extern_methods!(
        #[unsafe(method(setPlayer:))]
        pub fn setPlayer(&self, player: Option<&AVPlayer>);

        #[unsafe(method(setControlsStyle:))]
        pub fn setControlsStyle(&self, style: isize);
    );
}

extern_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct AVPlayer;
);

impl AVPlayer {
    pub fn with_url(url: &NSURL, _mtm: MainThreadMarker) -> Retained<Self> {
        unsafe { objc2::msg_send![<Self as ClassType>::class(), playerWithURL: url] }
    }

    extern_methods!(
        #[unsafe(method(play))]
        pub fn play(&self);

        #[unsafe(method(pause))]
        pub fn pause(&self);

        #[unsafe(method(setMuted:))]
        pub fn setMuted(&self, muted: bool);

        /// `AVPlayerStatusReadyToPlay` is 1. Before that, `play()` does not
        /// catch.
        #[unsafe(method(status))]
        pub fn status(&self) -> isize;

        /// Zero means stopped. Good for telling whether a `play()` really
        /// took hold.
        #[unsafe(method(rate))]
        pub fn rate(&self) -> f32;

        #[unsafe(method(error))]
        pub fn error(&self) -> Option<Retained<objc2_foundation::NSError>>;
    );
}
