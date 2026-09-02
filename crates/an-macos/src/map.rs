//! `MKMapView`, el mapa del sistema.
//!
//! En macOS MapKit hereda de `NSView`, así que el mapa entra en el árbol como
//! una vista más: no hay controlador que contener ni capa que recolocar, que es
//! lo que sí hace falta para el vídeo de iOS.
//!
//! La clase se declara aquí en vez de traerse `objc2-map-kit`, por lo mismo que
//! `web.rs` no se trae `objc2-web-kit`: por tres métodos entrarían a compilar
//! todas las clases del framework en cada build del host. `objc2` sigue
//! comprobando cada firma contra la de verdad al mandar el mensaje, así que lo
//! que se gana en tiempo de compilación no se paga en seguridad.
//!
//! **No hace falta ninguna clave.** El MapKit nativo —el de `MKMapView`, no el
//! de JavaScript— no la pide: la pide MapKit JS, que es otro producto. Lo que
//! sí pide permiso es enseñar dónde estás, y eso es `showsUser`; ver
//! `NSLocationUsageDescription` en `shells/macos/Resources/Info.plist`.

use objc2::encode::{Encode, Encoding, RefEncode};
use objc2::rc::Retained;
use objc2::{extern_class, extern_methods, ClassType, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSResponder, NSView};
use objc2_core_foundation::CGFloat;
use objc2_foundation::NSObject;

// El framework hay que enlazarlo: nadie más lo hace por nosotros.
#[link(name = "MapKit", kind = "framework")]
unsafe extern "C" {}

/// Un punto del globo. Grados, no radianes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CLLocationCoordinate2D {
    pub latitude: CGFloat,
    pub longitude: CGFloat,
}

unsafe impl Encode for CLLocationCoordinate2D {
    const ENCODING: Encoding =
        Encoding::Struct("CLLocationCoordinate2D", &[CGFloat::ENCODING, CGFloat::ENCODING]);
}

unsafe impl RefEncode for CLLocationCoordinate2D {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
}

/// Cuánto globo se ve, en grados. Es el zoom dicho de otra manera: MapKit no
/// trabaja con niveles de zoom sino con cuánto abarca la ventana.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct MKCoordinateSpan {
    pub latitude_delta: CGFloat,
    pub longitude_delta: CGFloat,
}

unsafe impl Encode for MKCoordinateSpan {
    // Sin nombre a propósito, igual que en el host de iOS: el runtime describe
    // estas dos como structs anónimas —`{?=dd}`— y objc2 compara la firma
    // entera contra la de verdad antes de mandar el mensaje, así que ponerles
    // el nombre que llevan en la cabecera no cuadra y aborta el proceso.
    const ENCODING: Encoding = Encoding::Struct("?", &[CGFloat::ENCODING, CGFloat::ENCODING]);
}

unsafe impl RefEncode for MKCoordinateSpan {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct MKCoordinateRegion {
    pub center: CLLocationCoordinate2D,
    pub span: MKCoordinateSpan,
}

unsafe impl Encode for MKCoordinateRegion {
    const ENCODING: Encoding =
        Encoding::Struct("?", &[CLLocationCoordinate2D::ENCODING, MKCoordinateSpan::ENCODING]);
}

unsafe impl RefEncode for MKCoordinateRegion {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
}

extern_class!(
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct MKMapView;
);

impl MKMapView {
    pub fn new(_mtm: MainThreadMarker) -> Retained<Self> {
        unsafe { objc2::msg_send![<Self as ClassType>::class(), new] }
    }

    extern_methods!(
        #[unsafe(method(setRegion:animated:))]
        pub fn setRegion_animated(&self, region: MKCoordinateRegion, animated: bool);

        #[unsafe(method(setShowsUserLocation:))]
        pub fn setShowsUserLocation(&self, shows: bool);
    );
}

/// Cuántos grados de longitud abarca la ventana en ese nivel de zoom.
///
/// La primitiva habla de niveles al estilo de las teselas —0 es el mundo entero
/// y cada nivel es el doble de cerca— y MapKit habla de grados. La conversión
/// vive aquí y no en la plantilla para que la misma cifra signifique lo mismo
/// en las tres plataformas.
pub fn span_for_zoom(zoom: f64) -> f64 {
    360.0 / 2f64.powf(zoom.max(0.0))
}
