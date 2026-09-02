//! Host macOS: implementa `HostRenderer` y `TextMeasurer` sobre AppKit, y
//! expone el runtime al shell de Swift por FFI.
//!
//! Es el hermano del host de iOS y no el del reloj: AppKit es imperativo y
//! `NSView` existe, así que el árbol del núcleo se refleja en una jerarquía de
//! vistas de verdad, con una vista por nodo montable y el marco escrito
//! directamente. Lo que cambia respecto a UIKit no es el planteamiento, son
//! tres cosas del escritorio, y las tres están comentadas donde se resuelven:
//!
//! - **La ventana se redimensiona en caliente.** En un teléfono el viewport
//!   solo cambia al rotar, que pasa una vez cada mucho; aquí cambia sesenta
//!   veces por segundo mientras alguien arrastra una esquina. Ver
//!   `ffi::an_runtime_set_viewport` y `AnRootView.layout()` en el shell.
//! - **Hay ratón, no dedos.** Los gestos son los reconocedores de AppKit, que
//!   no son los mismos que los de UIKit, y el deslizamiento no es uno de
//!   ellos: es un evento suelto que llega por la cadena de responder. Ver
//!   `flipped.rs`. Lo que hay además de los dedos —el puntero por encima y su
//!   forma— es `events::HoverTarget` y la prop `cursor`. Lo que aun así no se
//!   puede entregar se dice al suscribirse; ver `support::unsupported_event`.
//! - **El menú es del sistema y vive en la ventana, no en el árbol.** Lo pone
//!   el shell y no se expone a Angular: no hay primitiva para ello, e
//!   inventarla sería cambiar `packages/primitives`. Ver
//!   `shells/macos/Sources/AppDelegate.swift`.
//!
//! Todo lo de aquí corre en el hilo principal, igual que en iOS: AppKit no
//! admite otra cosa y el `MainThreadMarker` de objc2 lo hace explícito en el
//! tipo.

// El inventario de primitivas no toca plataforma a propósito: así se puede
// comprobar desde `cargo test` en cualquier máquina, y `scripts/check-macos.sh`
// lo puede leer sin compilar nada.
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
mod video;
#[cfg(target_os = "macos")]
mod web;

#[cfg(target_os = "macos")]
pub use host::AppKitHost;
#[cfg(target_os = "macos")]
pub use measure::AppKitMeasurer;
