//! `WKWebView`, declared by hand.
//!
//! On macOS `objc2-web-kit` does cover this class —it inherits from `NSView`,
//! which is what the generator understands— but pulling in the whole crate for
//! four methods is four hundred more classes to compile on every build of the
//! host. It is declared here just as on iOS, with the same four signatures,
//! and `objc2` still checks them against the real ones at run time.

use objc2::rc::Retained;
use objc2::{extern_class, extern_methods, ClassType, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSResponder, NSView};
use objc2_foundation::{NSObject, NSString, NSURL, NSURLRequest};

// The framework has to be linked: nobody else does it for us.
#[link(name = "WebKit", kind = "framework")]
unsafe extern "C" {}

extern_class!(
    #[unsafe(super(NSView, NSResponder, NSObject))]
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
        // All four return a `WKNavigation *` that is of no use whatsoever
        // here, but it has to be declared: objc2 checks the signature against
        // the real one, and saying `void` where there is an object aborts the
        // process.
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
