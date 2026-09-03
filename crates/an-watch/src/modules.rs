//! The watch's native modules.
//!
//! There is one, `device`, and it is not read from here: it is handed over by
//! the SwiftUI shell at startup, the same way the controls' natural sizes are
//! and for the same reason. `WKInterfaceDevice` is WatchKit, there is no Rust
//! binding for it, and reading the four fields would mean four trips through
//! Objective-C for what Swift settles in one line. `an-android` does exactly
//! this with `AnHost.deviceInfo()`.
//!
//! What the shell does **not** decide is which platform this is. That is
//! settled here, for the same reason `an-ios` takes it from the `cfg`: the host
//! is the watch's and it cannot be anything else, and asking somebody else
//! about it would only be room to get it wrong.
//!
//! Plugins do not live here. They come from outside, they are Swift, and what
//! carries the call out to them is `crate::plugins` — the same postman `an-ios`
//! has. What this module still owns is the reason a *missing* module is
//! missing, and that reason is now built from what the `.app` actually carries:
//! see `plugins::absent_note`.

use an_bridge::native_module;
use serde_json::{Map, Value};

/// Where this is running. `'watchos'` is one of the values of `NativePlatform`
/// in `packages/platform-native/src/native-modules.ts`, and until this module
/// existed it was a value no host ever produced.
const PLATFORM: &str = "watchos";

pub struct DeviceModule {
    /// `None` when the shell handed over nothing usable. The call is then
    /// rejected saying so, rather than answering an object with holes in it
    /// that would be read as if it were real.
    info: Option<Value>,
}

impl DeviceModule {
    /// `raw` is what came in through `an_watch_runtime_new`: a flat JSON object
    /// with `systemVersion`, `model`, `scale` and `locale`. It may be `None`,
    /// and anything that is not an object is treated as if it were.
    pub fn from_shell(raw: Option<&str>) -> Self {
        let Some(raw) = raw else {
            return DeviceModule { info: None };
        };
        let parsed: Map<String, Value> = match serde_json::from_str(raw) {
            Ok(parsed) => parsed,
            Err(error) => {
                eprintln!("angular-native: the device information makes no sense: {error}");
                return DeviceModule { info: None };
            }
        };
        let mut info = parsed;
        // Overwritten and not defaulted: whatever the shell said about the
        // platform, this is a watch.
        info.insert("platform".to_owned(), Value::String(PLATFORM.to_owned()));
        DeviceModule { info: Some(Value::Object(info)) }
    }
}

native_module! {
    DeviceModule as "device" {
        fn info(&mut self, _args: ()) -> Result<Value, String> {
            self.info.clone().ok_or_else(|| {
                "the watchOS shell did not hand over the device information at startup"
                    .to_owned()
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use an_bridge::modules::ModuleRegistry;
    use serde_json::json;

    use super::DeviceModule;

    /// The whole way in: registered under the name JS calls, answering the
    /// method JS calls, over the real registry. Anything short of that would
    /// still be green with the module wired to nothing.
    #[test]
    fn the_device_answers_through_the_registry() {
        let mut registry = ModuleRegistry::new();
        registry.register(Box::new(DeviceModule::from_shell(Some(
            r#"{"systemVersion":"11.2","model":"Apple Watch","scale":2.0,"locale":"en_GB"}"#,
        ))));

        let id = registry.invoke("device", "info", json!(null));
        let answers = registry.drain();
        assert_eq!(answers.len(), 1);
        assert_eq!(answers[0].0, id);
        let info = answers[0].1.as_ref().expect("the device has to answer");

        // The platform is the crate's word and not the shell's: the shell never
        // sent one and this is a watch either way.
        assert_eq!(info["platform"], "watchos");
        assert_eq!(info["systemVersion"], "11.2");
        assert_eq!(info["model"], "Apple Watch");
        assert_eq!(info["scale"], 2.0);
        assert_eq!(info["locale"], "en_GB");
    }

    /// And it stays the crate's word even if the shell says otherwise. The
    /// alternative —trusting what came in— is a `'watchos'` that turns into
    /// whatever a bad build of the shell felt like sending.
    #[test]
    fn the_shell_cannot_rename_the_platform() {
        let mut registry = ModuleRegistry::new();
        registry.register(Box::new(DeviceModule::from_shell(Some(
            r#"{"platform":"ios","model":"Apple Watch"}"#,
        ))));
        registry.invoke("device", "info", json!(null));
        let answers = registry.drain();
        assert_eq!(answers[0].1.as_ref().unwrap()["platform"], "watchos");
    }

    /// With nothing from the shell the call is rejected saying so. Answering an
    /// object with holes in it would be read as real data by anybody who got it.
    #[test]
    fn with_no_data_from_the_shell_it_says_so_instead_of_making_it_up() {
        let mut registry = ModuleRegistry::new();
        registry.register(Box::new(DeviceModule::from_shell(None)));
        registry.invoke("device", "info", json!(null));
        let answers = registry.drain();
        let error = answers[0].1.as_ref().expect_err("there is nothing to answer with");
        assert!(error.contains("did not hand over"), "{error}");
    }

    /// A method that does not exist is turned down naming the one that was
    /// asked for, which is what `native_module!` is there to guarantee.
    #[test]
    fn a_method_that_does_not_exist_names_itself() {
        let mut registry = ModuleRegistry::new();
        registry.register(Box::new(DeviceModule::from_shell(Some("{}"))));
        registry.invoke("device", "battery", json!(null));
        let answers = registry.drain();
        let error = answers[0].1.as_ref().expect_err("there is no such method");
        assert!(error.contains("battery"), "{error}");
    }

    /// And a module nobody registered comes back with the name *and* the reason
    /// there is nothing under it, which on this host is not a typo.
    ///
    /// The clipboard is the right example and not an arbitrary one: it is the
    /// plugin that ships in this repo and cannot exist on a watch, so this is
    /// the exact rejection somebody porting an app from the phone will read.
    #[test]
    fn an_absent_module_comes_back_with_the_reason() {
        let mut registry = ModuleRegistry::new();
        registry.explain_absent(crate::plugins::absent_note());
        registry.invoke("clipboard", "read", json!(null));
        let answers = registry.drain();
        let error = answers[0].1.as_ref().expect_err("there is no clipboard here");
        assert!(error.contains("clipboard"), "{error}");
        assert!(error.contains("watchOS half"), "{error}");
    }
}
