//! `WKWebView`, declared by hand.
//!
//! `objc2-web-kit` only generates this class for macOS, where it inherits from
//! `NSView`; iOS's inherits from `UIView` and is not in the crate. Declaring
//! it here is no mystery —it is an Objective-C class like any other— and it
//! saves waiting for the generator to cover it.

use objc2::rc::Retained;
use objc2::{extern_class, extern_methods, ClassType, MainThreadMarker, MainThreadOnly};
use objc2_foundation::{NSObject, NSString, NSURL, NSURLRequest};
use objc2_ui_kit::{UIResponder, UIView};

// The framework has to be linked: nobody else does it for us.
#[link(name = "WebKit", kind = "framework")]
unsafe extern "C" {}

extern_class!(
    #[unsafe(super(UIView, UIResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct WKWebView;
);

impl WKWebView {
    /// A new, empty web view. `new` cannot be declared in `extern_methods!`
    /// —it takes no arguments and the thread marker is not one of them— so it
    /// is called down objc2's ordinary path.
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let _ = mtm;
        unsafe { objc2::msg_send![<Self as ClassType>::class(), new] }
    }

    extern_methods!(
        // All four return a `WKNavigation *`, which is of no use whatsoever
        // here but has to be declared: objc2 checks the signature against the
        // real one at run time, and saying `void` where there is an object
        // aborts the process. A good check to have, incidentally: it is a
        // mistake that goes unnoticed in bare Objective-C.
        #[unsafe(method(loadRequest:))]
        pub fn loadRequest(&self, request: &NSURLRequest) -> Option<Retained<NSObject>>;

        #[unsafe(method(loadHTMLString:baseURL:))]
        pub fn loadHTMLString_baseURL(
            &self,
            html: &NSString,
            base: Option<&NSURL>,
        ) -> Option<Retained<NSObject>>;

        #[unsafe(method(reload))]
        pub fn reload(&self) -> Option<Retained<NSObject>>;

        #[unsafe(method(goBack))]
        pub fn goBack(&self) -> Option<Retained<NSObject>>;
    );
}
