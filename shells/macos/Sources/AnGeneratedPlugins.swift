// The placeholder `an` replaces.
//
// The real one is written per build out of the app's `package.json` —one
// `AnPluginRegistry.register` line per plugin— and it lands in `build/`, not
// here. This file exists so the shell is a complete program on its own: the
// registry calls `AnGeneratedPlugins.install()` unconditionally, and without
// something under that name `swiftc -typecheck` over `Sources/*.swift` fails on
// a symbol that only exists at build time. `scripts/check-accessibility.sh` does
// exactly that type-check, and it is worth keeping honest.
//
// It is **excluded by name** from the real build (see `swift_sources` and the
// filter in `an-cli`'s `macos.rs` and `watchos.rs`); two enums of the same name
// in one `swiftc` invocation is a redeclaration error, which is the loud way for
// that filter to fail rather than the quiet one.
enum AnGeneratedPlugins {
    static func install() {
        // Nothing. An app built by `an` never sees this file.
    }
}
