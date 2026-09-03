//! `MKMapView`, the system's map.
//!
//! On macOS MapKit inherits from `NSView`, so the map goes into the tree as
//! one more view: there is no controller to contain and no layer to
//! reposition, which is what the iOS video does need.
//!
//! The class is declared here rather than pulling in `objc2-map-kit`, for the
//! same reason `web.rs` does not pull in `objc2-web-kit`: for three methods
//! every class in the framework would go in to be compiled on every build of
//! the host. `objc2` still checks each signature against the real one when the
//! message is sent, so what is gained in compile time is not paid for in
//! safety.
//!
//! **No key is needed.** Native MapKit —`MKMapView`'s, not JavaScript's— does
//! not ask for one: MapKit JS asks for it, and that is a different product.
//! What does ask for permission is showing where you are, and that is
//! `showsUser`; see `NSLocationUsageDescription` in
//! `shells/macos/Resources/Info.plist`.

use objc2::encode::{Encode, Encoding, RefEncode};
use objc2::rc::Retained;
use objc2::{extern_class, extern_methods, ClassType, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSResponder, NSView};
use objc2_core_foundation::CGFloat;
use objc2_foundation::NSObject;

// The framework has to be linked: nobody else does it for us.
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
    // Nameless on purpose, just as in the iOS host: the runtime describes
    // these two as anonymous structs —`{?=dd}`— and objc2 compares the whole
    // signature against the real one before sending the message, so giving
    // them the name they carry in the header does not match and aborts the
    // process.
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

/// How many degrees of longitude the window spans at that zoom level.
///
/// The primitive speaks in tile-style levels —0 is the whole world and each
/// level is twice as close— and MapKit speaks in degrees. The conversion lives
/// here and not in the template so that the same figure means the same thing
/// on all three platforms.
pub fn span_for_zoom(zoom: f64) -> f64 {
    360.0 / 2f64.powf(zoom.max(0.0))
}
