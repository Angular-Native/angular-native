//! `MKMapView`, declared by hand.
//!
//! `objc2-map-kit` generates this class only for macOS, the same way it goes
//! with `WKWebView`: iOS's inherits from `UIView` and is not in the crate.

use objc2::encode::{Encode, Encoding, RefEncode};
use objc2::rc::Retained;
use objc2::{extern_class, extern_methods, ClassType, MainThreadMarker, MainThreadOnly};
use objc2_core_foundation::CGFloat;
use objc2_foundation::NSObject;
use objc2_ui_kit::{UIResponder, UIView};

#[link(name = "MapKit", kind = "framework")]
unsafe extern "C" {}

/// A point on the globe. Degrees, not radians.
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

/// How much of the globe is in view, in degrees. It is the zoom said another
/// way: MapKit does not work in zoom levels but in how much the window spans.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct MKCoordinateSpan {
    pub latitude_delta: CGFloat,
    pub longitude_delta: CGFloat,
}

unsafe impl Encode for MKCoordinateSpan {
    // Nameless on purpose. The runtime describes these two as anonymous
    // structs —`{?=dd}`— and objc2 compares the whole signature against the
    // real one before sending the message: giving them the name they carry in
    // the header does not match and aborts.
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
