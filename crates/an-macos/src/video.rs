//! Vídeo: `AVPlayerView` con un `AVPlayer` dentro.
//!
//! Aquí el escritorio sale más barato que el teléfono, y no por poco. En UIKit
//! no hay ninguna vista de vídeo: hay un `AVPlayerViewController`, y meterlo en
//! el árbol obliga a hacerlo hijo del controlador que manda y a recolocarle la
//! vista a mano en cada frame, porque nace después de que el marco esté puesto.
//! En AppKit sí la hay —`AVPlayerView` **es** una `NSView`—, así que el
//! reproductor entra en el árbol como cualquier otra vista, con sus controles
//! del sistema incluidos y sin nada que sincronizar.
//!
//! Las clases se declaran a mano por lo mismo que en `web.rs` y en `map.rs`:
//! traerse `objc2-av-kit` y `objc2-av-foundation` por seis métodos mete dos
//! frameworks enteros de clases generadas en cada build del host. `objc2`
//! comprueba cada firma contra la de verdad al mandar el mensaje.

use objc2::rc::Retained;
use objc2::{extern_class, extern_methods, ClassType, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSResponder, NSView};
use objc2_foundation::{NSObject, NSURL};

#[link(name = "AVFoundation", kind = "framework")]
unsafe extern "C" {}

#[link(name = "AVKit", kind = "framework")]
unsafe extern "C" {}

/// Los controles del sistema, pegados abajo dentro de la vista.
/// `AVPlayerViewControlsStyleInline`.
pub const CONTROLS_INLINE: isize = 1;

extern_class!(
    /// La vista de vídeo de AppKit: imagen y controles de reproducción.
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

        /// `AVPlayerStatusReadyToPlay` es 1. Antes de eso, `play()` no prende.
        #[unsafe(method(status))]
        pub fn status(&self) -> isize;

        /// Cero es parado. Sirve para saber si un `play()` agarró de verdad.
        #[unsafe(method(rate))]
        pub fn rate(&self) -> f32;

        #[unsafe(method(error))]
        pub fn error(&self) -> Option<Retained<objc2_foundation::NSError>>;
    );
}
