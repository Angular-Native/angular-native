//! Video playback: an `AVPlayer` inside an `AVPlayerLayer`.
//!
//! There is no video view in UIKit: what there is is a layer that hangs off
//! any view. So the view is an ordinary `UIView` and the host resizes the
//! layer when the frame changes, which is what any app would do.
//!
//! The classes are declared by hand for the same reason as `WKWebView`: the
//! generated crates do not ship iOS's.

use objc2::rc::Retained;
use objc2::{extern_class, extern_methods, ClassType, MainThreadMarker, MainThreadOnly};
use objc2_foundation::{NSObject, NSString, NSURL};
use objc2_ui_kit::{UIResponder, UIViewController};
use objc2_quartz_core::CALayer;

#[link(name = "AVFoundation", kind = "framework")]
unsafe extern "C" {}

#[link(name = "AVKit", kind = "framework")]
unsafe extern "C" {}

extern_class!(
    /// The player with its view and its controls.
    ///
    /// It is the way Apple supports showing video: hanging an `AVPlayerLayer`
    /// off some arbitrary view compiles and runs, but the layer never gets as
    /// far as delivering frames —`readyForDisplay` stays false with the player
    /// audibly playing. It also brings the system's controls along, the same
    /// way Android's `VideoView` does.
    #[unsafe(super(UIViewController, UIResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct AVPlayerViewController;
);

impl AVPlayerViewController {
    pub fn new(_mtm: MainThreadMarker) -> Retained<Self> {
        unsafe { objc2::msg_send![<Self as ClassType>::class(), new] }
    }

    extern_methods!(
        #[unsafe(method(setPlayer:))]
        pub fn setPlayer(&self, player: Option<&AVPlayer>);

        #[unsafe(method(setShowsPlaybackControls:))]
        pub fn setShowsPlaybackControls(&self, shows: bool);
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

        #[unsafe(method(status))]
        pub fn status(&self) -> isize;

        /// Zero means stopped. Good for telling whether a `play()` really
        /// caught.
        #[unsafe(method(rate))]
        pub fn rate(&self) -> f32;

        #[unsafe(method(error))]
        pub fn error(&self) -> Option<Retained<objc2_foundation::NSError>>;
    );
}

extern_class!(
    #[unsafe(super(CALayer, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct AVPlayerLayer;
);

impl AVPlayerLayer {
    pub fn with_player(player: &AVPlayer, _mtm: MainThreadMarker) -> Retained<Self> {
        unsafe { objc2::msg_send![<Self as ClassType>::class(), playerLayerWithPlayer: player] }
    }

    extern_methods!(
        /// `AVLayerVideoGravityResizeAspect` and company. The string is
        /// passed straight through so as not to drag in the framework's
        /// constants.
        #[unsafe(method(setVideoGravity:))]
        pub fn setVideoGravity(&self, gravity: &NSString);

        #[unsafe(method(isReadyForDisplay))]
        pub fn isReadyForDisplay(&self) -> bool;

        #[unsafe(method(setPlayer:))]
        pub fn setPlayer(&self, player: Option<&AVPlayer>);

        #[unsafe(method(player))]
        pub fn player(&self) -> Option<Retained<AVPlayer>>;
    );
}
