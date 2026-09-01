//! Reproducción de vídeo: `AVPlayer` dentro de un `AVPlayerLayer`.
//!
//! No hay una vista de vídeo en UIKit: lo que hay es una capa que se cuelga de
//! cualquier vista. Así que la vista es una `UIView` normal y el host le
//! ajusta la capa cuando cambia el marco, que es lo que haría cualquier app.
//!
//! Las clases se declaran a mano por lo mismo que `WKWebView`: los crates
//! generados no traen las de iOS.

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
    /// El reproductor con su vista y sus controles.
    ///
    /// Es la forma que soporta Apple para enseñar vídeo: colgar un
    /// `AVPlayerLayer` de una vista cualquiera se compila y se ejecuta, pero
    /// la capa nunca llega a entregar fotogramas —`readyForDisplay` se queda
    /// en falso con el reproductor sonando—. Además así vienen los controles
    /// del sistema, igual que trae el `VideoView` de Android.
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

        /// Cero es parado. Sirve para saber si un `play()` prendió de verdad.
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
        /// `AVLayerVideoGravityResizeAspect` y compañía. Se pasa la cadena
        /// directamente para no arrastrar las constantes del framework.
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
