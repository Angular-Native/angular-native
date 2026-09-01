//! `MKMapView`, declarada a mano.
//!
//! `objc2-map-kit` genera esta clase solo para macOS, igual que pasa con
//! `WKWebView`: la de iOS hereda de `UIView` y no está en el crate.

use objc2::encode::{Encode, Encoding, RefEncode};
use objc2::rc::Retained;
use objc2::{extern_class, extern_methods, ClassType, MainThreadMarker, MainThreadOnly};
use objc2_core_foundation::CGFloat;
use objc2_foundation::NSObject;
use objc2_ui_kit::{UIResponder, UIView};

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

/// Cuánto globo se ve, en grados. Es el zoom, dicho de otra manera: MapKit no
/// trabaja con niveles de zoom sino con cuánto abarca la ventana.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct MKCoordinateSpan {
    pub latitude_delta: CGFloat,
    pub longitude_delta: CGFloat,
}

unsafe impl Encode for MKCoordinateSpan {
    // Sin nombre a propósito. El runtime describe estas dos como structs
    // anónimas —`{?=dd}`— y objc2 compara la firma entera contra la de verdad
    // antes de enviar el mensaje: ponerles el nombre que tienen en la cabecera
    // no cuadra y aborta.
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
    const ENCODING: Encoding = Encoding::Struct(
        "?",
        &[CLLocationCoordinate2D::ENCODING, MKCoordinateSpan::ENCODING],
    );
}

unsafe impl RefEncode for MKCoordinateRegion {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
}

extern_class!(
    #[unsafe(super(UIView, UIResponder, NSObject))]
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

        #[unsafe(method(setMapType:))]
        pub fn setMapType(&self, kind: usize);

        #[unsafe(method(setShowsUserLocation:))]
        pub fn setShowsUserLocation(&self, shows: bool);
    );
}
