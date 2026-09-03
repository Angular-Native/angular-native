//! Where the signing settings live, and everything that has to be true before a
//! single byte is compiled.
//!
//! Signing is the one part of a build that fails late, slowly and in another
//! language. `codesign` answers with an exit code and a sentence about a
//! "resource envelope"; `apksigner` with a Java stack trace; an expired
//! provisioning profile with an installation that is refused on the device and
//! logged nowhere the person who ran the build can see. All of it half a minute
//! to two minutes after the command was typed, and none of it saying what to do.
//!
//! So this module does two things and only two:
//!
//!   · it resolves the settings —from the project manifest, from the
//!     environment—, and
//!   · it checks **before anything is compiled** that what the build is going to
//!     need is there, valid, and about the app being built.
//!
//! Nothing in here compiles, signs or uploads. The platform modules do that,
//! and by then they have a struct in hand whose every field has already been
//! checked.
//!
//! ## Where the settings go
//!
//! In `angular-native.json`, next to `app` and `platforms`, in a `signing`
//! object keyed by platform:
//!
//! ```json
//! {
//!   "app": { "name": "MyApp", "bundleId": "com.example.myapp", "entry": "src/main.native.ts" },
//!   "platforms": ["ios", "android"],
//!   "signing": {
//!     "ios": {
//!       "team": "ABCDE12345",
//!       "identity": "Apple Development",
//!       "profile": "ios/profiles/development.mobileprovision"
//!     },
//!     "macos": {
//!       "identity": "Developer ID Application",
//!       "team": "ABCDE12345",
//!       "notaryProfile": "an-notary"
//!     },
//!     "android": {
//!       "keystore": "android/release.keystore",
//!       "keyAlias": "upload",
//!       "storePasswordEnv": "AN_ANDROID_KEYSTORE_PASSWORD",
//!       "keyPasswordEnv": "AN_ANDROID_KEY_PASSWORD"
//!     }
//!   }
//! }
//! ```
//!
//! **No password ever goes in that file.** `storePasswordEnv` is the *name of an
//! environment variable*, not a password; a literal `storePassword` is refused
//! by [`Settings::read`] with the file and the key named. The manifest is
//! committed, and a repository that carries the keystore password is a
//! repository whose keystore has to be replaced.
//!
//! Every setting also has an environment variable that wins over the manifest
//! (`AN_IOS_TEAM`, `AN_ANDROID_KEYSTORE`…). That is what CI uses, and it is
//! what makes these paths runnable from the monorepo, which has no manifest at
//! all.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde_json::{Map, Value};

use crate::workspace::Workspace;

/// The documentation page every one of these errors points at. It is one page,
/// and it is the one that says what to ask Apple and Google for.
pub const DOCS: &str = "https://angular-native.dev/guide/signing-and-distribution/";

// ---------------------------------------------------------------------------
// The raw settings
// ---------------------------------------------------------------------------

/// The `signing` object as it was written, plus where it was written, which is
/// what the error messages need in order to name a file.
pub struct Settings {
    /// The manifest the section came from. `None` in the monorepo, where there
    /// is no manifest and the environment is the only source.
    source: Option<PathBuf>,
    /// The project's root, for resolving relative paths and for asking git
    /// whether a file it is pointed at is ignored.
    root: Option<PathBuf>,
    sections: Map<String, Value>,
}

/// A key whose value is a secret has no business in a file that gets committed.
///
/// The list is short and closed on purpose: it is not a heuristic over
/// everything that looks like a password, it is the set of keys somebody would
/// naturally reach for when the manifest already has `storePasswordEnv` in it
/// and they want to stop typing an export. Whatever is added here has to be
/// added to the documentation too.
const FORBIDDEN: &[&str] = &[
    "password",
    "passphrase",
    "storePassword",
    "keyPassword",
    "keystorePassword",
    "secret",
    "appleId",
    "appSpecificPassword",
    "apiKey",
    "privateKey",
];

impl Settings {
    pub fn read(workspace: &Workspace) -> Result<Self> {
        let Some(project) = &workspace.project else {
            return Ok(Settings { source: None, root: None, sections: Map::new() });
        };
        let path = project.root.join(crate::workspace::MARKER);
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("{} could not be read", path.display()))?;
        let parsed: Value = serde_json::from_str(&text)
            .with_context(|| format!("{} is not valid JSON", path.display()))?;
        let sections = match parsed.get("signing") {
            None => Map::new(),
            Some(Value::Object(map)) => map.clone(),
            Some(_) => bail!(
                "{}: signing has to be an object keyed by platform \
                 (\"ios\", \"macos\", \"android\").\nSee {DOCS}",
                path.display()
            ),
        };
        check_for_secrets(&path, &sections)?;
        Ok(Settings {
            source: Some(path),
            root: Some(project.root.clone()),
            sections,
        })
    }

    /// The file the settings came from, named the way an error message wants to
    /// name it.
    fn where_from(&self) -> String {
        match &self.source {
            Some(path) => path.display().to_string(),
            None => crate::workspace::MARKER.to_owned(),
        }
    }

    fn section(&self, platform: &str) -> Option<&Map<String, Value>> {
        self.sections.get(platform).and_then(Value::as_object)
    }

    /// One setting. The environment wins: it is what CI sets, and it is the only
    /// source there is when `an` runs inside the monorepo.
    ///
    /// An empty environment variable counts as unset. Exporting `AN_IOS_TEAM=`
    /// to "clear" it is common enough, and taking it as an identity of the empty
    /// name would produce a `codesign` error about nothing.
    fn value(&self, platform: &str, key: &str, variable: &str) -> Option<String> {
        if let Ok(from_env) = std::env::var(variable) {
            if !from_env.trim().is_empty() {
                return Some(from_env);
            }
        }
        self.section(platform)?
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_owned)
    }

    /// A setting that names a file, resolved against the project's root.
    ///
    /// Relative in the manifest, because the manifest is committed and an
    /// absolute path belongs to the machine of whoever wrote it. Absolute from
    /// the environment, because that is a path on the machine running the build
    /// and nothing else will do on CI, where the keystore is decoded into a
    /// temporary directory.
    fn file(&self, platform: &str, key: &str, variable: &str) -> Option<PathBuf> {
        let raw = self.value(platform, key, variable)?;
        let path = PathBuf::from(&raw);
        if path.is_absolute() {
            return Some(path);
        }
        Some(match &self.root {
            Some(root) => root.join(path),
            None => path,
        })
    }

    /// Says nothing if the file is out of git's reach; complains, naming it, if
    /// it is committed or merely committable.
    ///
    /// A keystore in the repository is not a mistake that shows up later: it is
    /// the whole of the app's identity on Play, and the fix once it is pushed is
    /// to generate another one and change the upload key with Google. So it is
    /// said at the moment the CLI first reads the path, which is the last moment
    /// where it is still cheap.
    ///
    /// Everything here is silent when git is not around, when this is not a
    /// repository, or when the file lives outside it. A build has no business
    /// failing because somebody keeps their credentials in a directory that is
    /// not version-controlled — that is the good case.
    fn check_not_committed(&self, path: &Path, what: &str) -> Result<()> {
        let Some(root) = &self.root else { return Ok(()) };
        if !path.starts_with(root) {
            return Ok(());
        }
        let git = |args: &[&str]| -> Option<bool> {
            let output = Command::new("git")
                .args(args)
                .arg(path)
                .current_dir(root)
                .output()
                .ok()?;
            Some(output.status.success())
        };
        // Not a repository, or no git: there is nothing to say.
        let Some(tracked) = git(&["ls-files", "--error-unmatch"]) else { return Ok(()) };
        let relative = path.strip_prefix(root).unwrap_or(path).display().to_string();
        if tracked {
            bail!(
                "{relative} is committed to git, and it is the {what}.\n\
                 It has to come out of the repository's history, and the credential it \
                 holds has to be replaced: anybody who has ever cloned this has it.\n\
                 Keep it outside the project, or point at it with an environment \
                 variable.\nSee {DOCS}"
            );
        }
        if git(&["check-ignore", "-q"]) == Some(false) {
            bail!(
                "{relative} is the {what}, and git is not ignoring it: the next \
                 `git add .` commits it.\n\
                 Add it to .gitignore first:\n\
                 \x20   echo '{relative}' >> .gitignore\n\
                 See {DOCS}"
            );
        }
        Ok(())
    }

    /// A password, read from the environment variable the manifest names.
    ///
    /// The manifest holds the *name*; the value never touches disk. The default
    /// name is used when the manifest says nothing, so a project that sets the
    /// two usual variables needs no `signing.android` password keys at all.
    fn password(&self, platform: &str, key: &str, default_variable: &str) -> Result<String> {
        let variable = self
            .section(platform)
            .and_then(|section| section.get(key))
            .and_then(Value::as_str)
            .unwrap_or(default_variable)
            .to_owned();
        match std::env::var(&variable) {
            Ok(value) if !value.is_empty() => Ok(value),
            _ => bail!(
                "the environment variable {variable} is not set, and it is where the \
                 {} password is read from.\n\
                 \x20   export {variable}='…'\n\
                 The password does not go in {}: that file is committed.\nSee {DOCS}",
                if key == "keyPasswordEnv" { "key" } else { "keystore" },
                self.where_from()
            ),
        }
    }

    /// The error for a platform that has no section and no environment either.
    /// It shows the block to paste, because "configure signing" is not an
    /// instruction anybody can act on.
    fn missing(&self, platform: &str, command: &str, block: &str, variables: &str) -> anyhow::Error {
        let where_from = self.where_from();
        let intro = match &self.source {
            Some(_) => format!("{where_from} has no signing.{platform} section"),
            None => "there is no angular-native.json here —this is the monorepo, and it \
                     has no project manifest—"
                .to_owned(),
        };
        let add_it_to = match &self.source {
            Some(_) => format!("Add it to {where_from}"),
            None => "In a project made with `an init`, it goes in its angular-native.json"
                .to_owned(),
        };
        anyhow::anyhow!(
            "{intro}, and `{command}` needs one.\n\n\
             {add_it_to}:\n\n{block}\n\n\
             Or set it in the environment, which is what CI does and what works in the \
             monorepo:\n{variables}\n\nSee {DOCS}"
        )
    }
}

/// Walks the whole `signing` object looking for a key that holds a secret.
///
/// Recursive, because a section could grow subsections; and by key name rather
/// than by what the value looks like, because a password that happens to look
/// like a team identifier is still a password.
fn check_for_secrets(path: &Path, sections: &Map<String, Value>) -> Result<()> {
    fn walk(path: &Path, trail: &str, map: &Map<String, Value>) -> Result<()> {
        for (key, value) in map {
            if let Some(forbidden) = FORBIDDEN.iter().find(|f| f.eq_ignore_ascii_case(key)) {
                bail!(
                    "{}: {trail}{forbidden} is a secret, and this file is committed.\n\
                     Name an environment variable instead of holding the value:\n\
                     \x20   \"{forbidden}Env\": \"AN_SOMETHING_PASSWORD\"\n\
                     and export the value where the build runs. If this password has \
                     already been pushed, it has to be changed.\nSee {DOCS}",
                    path.display()
                );
            }
            if let Value::Object(nested) = value {
                walk(path, &format!("{trail}{key}."), nested)?;
            }
        }
        Ok(())
    }
    walk(path, "signing.", sections)
}

// ---------------------------------------------------------------------------
// Apple: iOS, tvOS, visionOS
// ---------------------------------------------------------------------------

/// Everything a device build or an archive needs, already checked.
pub struct Apple {
    /// What `codesign --sign` receives. Either the SHA-1 of a certificate or a
    /// prefix of its common name; both are what `security find-identity` prints.
    pub identity: String,
    /// The name the identity was found under, for saying it out loud.
    pub identity_name: String,
    pub team: String,
    pub profile: PathBuf,
    /// The profile's `Entitlements` dictionary. It is the base of what gets
    /// signed into the app: the system will not grant an entitlement the profile
    /// does not carry, so signing more than this produces an app that installs
    /// and is killed on launch.
    pub profile_entitlements: Map<String, Value>,
    /// The profile's name, as it reads in the developer portal.
    pub profile_name: String,
}

/// Development or distribution. It picks the default identity and it is what
/// tells the two apart in the error messages, which is most of their value.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// Onto a device plugged into this Mac.
    Development,
    /// Into an `.ipa` for TestFlight, the App Store or an ad-hoc build.
    Distribution,
}

impl Purpose {
    fn identity_default(self) -> &'static str {
        match self {
            Purpose::Development => "Apple Development",
            Purpose::Distribution => "Apple Distribution",
        }
    }

    fn command(self) -> &'static str {
        match self {
            Purpose::Development => "an ios --physical",
            Purpose::Distribution => "an ios --archive",
        }
    }
}

/// Resolves and checks the Apple settings for one platform slug (`ios`, `tvos`,
/// `visionos`).
///
/// Every failure here happens before `cargo` is called. That is deliberate: the
/// device build is a full compile of the core plus the shell, and finding out at
/// the end of it that the profile expired last month is the worst version of
/// this same message.
pub fn apple(
    settings: &Settings,
    platform: &str,
    bundle_id: &str,
    purpose: Purpose,
) -> Result<Apple> {
    let block = format!(
        "  \"signing\": {{\n    \
         \"{platform}\": {{\n      \
         \"team\": \"ABCDE12345\",\n      \
         \"identity\": \"{}\",\n      \
         \"profile\": \"{platform}/profiles/{}.mobileprovision\"\n    }}\n  }}",
        purpose.identity_default(),
        match purpose {
            Purpose::Development => "development",
            Purpose::Distribution => "distribution",
        }
    );
    let variables = "    export AN_IOS_TEAM=ABCDE12345\n\
                     \x20   export AN_IOS_IDENTITY='Apple Development'\n\
                     \x20   export AN_IOS_PROFILE=/path/to/profile.mobileprovision";

    let profile = settings
        .file(platform, "profile", "AN_IOS_PROFILE")
        .ok_or_else(|| settings.missing(platform, purpose.command(), &block, variables))?;
    settings.check_not_committed(&profile, "provisioning profile")?;
    if !profile.is_file() {
        bail!(
            "{} is missing, and it is the provisioning profile `{}` signs with.\n\
             Download it from https://developer.apple.com/account/resources/profiles/list \
             — or open Xcode once with this bundle id and let it create one, and it \
             will be in ~/Library/MobileDevice/Provisioning Profiles.\nSee {DOCS}",
            profile.display(),
            purpose.command()
        );
    }
    let decoded = decode_profile(&profile)?;
    let parsed = ProfileFacts::parse(&decoded, &profile)?;
    parsed.check_not_expired(&profile, &now_iso8601())?;
    parsed.check_bundle_id(&profile, bundle_id)?;

    let team = match settings.value(platform, "team", "AN_IOS_TEAM") {
        Some(configured) => {
            if !parsed.teams.iter().any(|team| team == &configured) {
                bail!(
                    "the team in {} is {configured:?}, and {} belongs to {}.\n\
                     One of the two is wrong: either the profile is somebody else's, or \
                     signing.{platform}.team is out of date.\nSee {DOCS}",
                    settings.where_from(),
                    profile.display(),
                    parsed.teams.join(", ")
                );
            }
            configured
        }
        // With no team configured the profile's is used: it is the authority
        // anyway, and asking somebody to copy a ten-character string out of a
        // file they already handed us is asking them to get it wrong.
        None => parsed.teams.first().cloned().context("the profile carries no team")?,
    };

    let wanted = settings
        .value(platform, "identity", "AN_IOS_IDENTITY")
        .unwrap_or_else(|| purpose.identity_default().to_owned());
    let (identity, identity_name) = find_identity(&wanted, purpose)?;

    Ok(Apple {
        identity,
        identity_name,
        team,
        profile,
        profile_entitlements: parsed.entitlements,
        profile_name: parsed.name,
    })
}

/// A `.mobileprovision` is a CMS envelope with a plist inside. `security cms -D`
/// is what unwraps it; there is no way to read one with a text editor and no
/// need to link a CMS library to do it here.
fn decode_profile(profile: &Path) -> Result<String> {
    let output = Command::new("security")
        .args(["cms", "-D", "-i"])
        .arg(profile)
        .output()
        .context("security could not be run")?;
    if !output.status.success() {
        bail!(
            "{} is not a provisioning profile that can be read: `security cms -D` \
             refused it.\n{}\nDownload it again from \
             https://developer.apple.com/account/resources/profiles/list.\nSee {DOCS}",
            profile.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// What is read out of a decoded profile, and the three things that are checked
/// against it.
///
/// It is parsed out of the XML by hand rather than converted to JSON with
/// `plutil`, and that is not laziness: a profile carries `<date>` and `<data>`
/// values, and `plutil -convert json` refuses a plist that holds either. The
/// keys that matter here are a string, a date and a dictionary of strings, and
/// those come out of the XML unambiguously.
pub struct ProfileFacts {
    pub name: String,
    pub expires: String,
    pub teams: Vec<String>,
    pub entitlements: Map<String, Value>,
}

impl ProfileFacts {
    pub fn parse(xml: &str, profile: &Path) -> Result<Self> {
        let name = plist_string(xml, "Name").unwrap_or_else(|| "unnamed".to_owned());
        let expires = plist_date(xml, "ExpirationDate").with_context(|| {
            format!(
                "{}: the profile carries no ExpirationDate, so it is not a profile.\n\
                 See {DOCS}",
                profile.display()
            )
        })?;
        let teams = plist_string_array(xml, "TeamIdentifier");
        if teams.is_empty() {
            bail!(
                "{}: the profile carries no TeamIdentifier.\nSee {DOCS}",
                profile.display()
            );
        }
        let entitlements = plist_string_dict(xml, "Entitlements");
        Ok(ProfileFacts { name, expires, teams, entitlements })
    }

    /// The identifier the profile is for: `TEAMID.com.example.app`, or
    /// `TEAMID.*` for a wildcard profile.
    pub fn application_identifier(&self) -> Option<&str> {
        self.entitlements.get("application-identifier")?.as_str()
    }

    pub fn check_not_expired(&self, profile: &Path, now: &str) -> Result<()> {
        // ISO-8601 in UTC sorts as text: same width, most significant first.
        // Comparing them as strings avoids a date library for one comparison.
        if self.expires.as_str() >= now {
            return Ok(());
        }
        bail!(
            "the provisioning profile {} expired on {} —today is {}—, and an expired \
             profile produces an app the device refuses to install, saying nothing \
             useful about why.\n\
             Renew it at \
             https://developer.apple.com/account/resources/profiles/list and download \
             it over {}.\nSee {DOCS}",
            self.name,
            &self.expires[..10.min(self.expires.len())],
            &now[..10.min(now.len())],
            profile.display()
        )
    }

    pub fn check_bundle_id(&self, profile: &Path, bundle_id: &str) -> Result<()> {
        let Some(identifier) = self.application_identifier() else {
            bail!(
                "{}: the profile's entitlements carry no application-identifier.\n\
                 See {DOCS}",
                profile.display()
            );
        };
        // `TEAMID.the.bundle.id`. The team prefix is not part of the comparison:
        // it is checked on its own, and it is not what somebody typing a bundle
        // id gets wrong.
        let Some((_, allowed)) = identifier.split_once('.') else {
            bail!(
                "{}: application-identifier is {identifier:?}, which has no team prefix.\n\
                 See {DOCS}",
                profile.display()
            );
        };
        if matches(allowed, bundle_id) {
            return Ok(());
        }
        let extra = if allowed.ends_with('*') {
            String::new()
        } else {
            format!(
                "\nA profile is tied to one app id. Either register {bundle_id} at \
                 https://developer.apple.com/account/resources/identifiers/list and \
                 make a profile for it, or change app.bundleId to {allowed}."
            )
        };
        bail!(
            "the profile {} is for {allowed}, and this app is {bundle_id}.\n\
             Signing it anyway produces an app that installs and is killed on \
             launch.{extra}\nSee {DOCS}",
            profile.display()
        )
    }
}

/// A wildcard app id (`com.example.*`) against a bundle id. The only wildcard
/// Apple allows is a trailing one.
pub fn matches(allowed: &str, bundle_id: &str) -> bool {
    match allowed.strip_suffix('*') {
        Some(prefix) => bundle_id.starts_with(prefix),
        None => allowed == bundle_id,
    }
}

/// Looks a codesigning identity up in the keychain.
///
/// The match is a prefix of the certificate's common name —"Apple Development"
/// finds "Apple Development: Jane Doe (7A8B9C0D1E)"— or the SHA-1 as it is
/// printed. That is how everybody refers to these in practice, and demanding the
/// full name would mean copying a parenthesised identifier by hand.
///
/// When there is no match, what there *is* gets listed. "identity not found" on
/// a machine with three certificates in it is a message that sends people to
/// Stack Overflow; the list turns it into a typo they can see.
pub fn find_identity(wanted: &str, purpose: Purpose) -> Result<(String, String)> {
    let found = codesigning_identities()?;
    if let Some((hash, name)) =
        found.iter().find(|(hash, name)| name.starts_with(wanted) || hash == wanted)
    {
        return Ok((hash.clone(), name.clone()));
    }
    let (what, how) = match purpose {
        Purpose::Development => (
            "a development certificate",
            "Xcode ▸ Settings ▸ Accounts ▸ Manage Certificates ▸ + ▸ Apple Development",
        ),
        Purpose::Distribution => (
            "a distribution certificate",
            "https://developer.apple.com/account/resources/certificates/list ▸ + ▸ \
             Apple Distribution, then download it and open it",
        ),
    };
    if found.is_empty() {
        bail!(
            "there is no codesigning identity in this keychain, and {} needs {what}.\n\
             Create one: {how}\n\
             Then `security find-identity -v -p codesigning` will list it.\nSee {DOCS}",
            purpose.command()
        );
    }
    bail!(
        "there is no codesigning identity whose name starts with {wanted:?}.\n\
         What there is:\n{}\n\
         Set signing.identity —or AN_IOS_IDENTITY— to one of those, or create {what}: \
         {how}\nSee {DOCS}",
        found
            .iter()
            .map(|(_, name)| format!("\x20   {name}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

/// The keychain's codesigning identities, as `(sha1, common name)`.
///
/// `security find-identity -v -p codesigning` prints one per line:
///
/// ```text
///   1) 3A7B… "Apple Development: Jane Doe (7A8B9C0D1E)"
/// ```
///
/// Valid ones only: `-v` leaves out the expired ones, which is what is wanted.
/// An expired certificate in the list would be offered as a candidate and fail
/// at `codesign` time with an error about a "resource envelope".
fn codesigning_identities() -> Result<Vec<(String, String)>> {
    let output = Command::new("security")
        .args(["find-identity", "-v", "-p", "codesigning"])
        .output()
        .context("security could not be run")?;
    // A keychain with no identities exits non-zero on some versions of macOS and
    // zero on others. Either way the answer is "there are none", and that is a
    // sentence this module already knows how to say properly.
    Ok(parse_identities(&String::from_utf8_lossy(&output.stdout)))
}

pub fn parse_identities(listing: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for line in listing.lines() {
        // The hash is looked for by shape and not by position: `security`
        // numbers the lines, and a keychain with ten identities in it starts
        // printing `10)` with one more character than `9)`.
        let Some(hash) = line
            .split_whitespace()
            .find(|field| field.len() == 40 && field.chars().all(|c| c.is_ascii_hexdigit()))
        else {
            continue;
        };
        let (Some(start), Some(end)) = (line.find('"'), line.rfind('"')) else { continue };
        if end <= start {
            continue;
        }
        found.push((hash.to_owned(), line[start + 1..end].to_owned()));
    }
    found
}

// ---------------------------------------------------------------------------
// macOS
// ---------------------------------------------------------------------------

/// What a Developer ID build needs. There is no provisioning profile here: an
/// app distributed outside the App Store is signed with a Developer ID
/// certificate and notarised, and neither step involves a profile.
pub struct Macos {
    pub identity: String,
    pub identity_name: String,
    /// The `notarytool` keychain profile: a name, not a credential. The Apple ID
    /// and the app-specific password behind it live in the keychain, put there
    /// once by `xcrun notarytool store-credentials`.
    pub notary_profile: Option<String>,
}

pub fn macos(settings: &Settings, notarising: bool) -> Result<Macos> {
    // There is no `team` here on purpose: on macOS nothing needs it. The
    // certificate carries the team, and notarytool gets it from the keychain
    // profile. A key in the manifest that changes nothing is a key somebody
    // will spend an afternoon getting right.
    let block = "  \"signing\": {\n    \
                 \"macos\": {\n      \
                 \"identity\": \"Developer ID Application\",\n      \
                 \"notaryProfile\": \"an-notary\"\n    }\n  }";
    let variables = "    export AN_MACOS_IDENTITY='Developer ID Application'\n\
                     \x20   export AN_MACOS_NOTARY_PROFILE=an-notary";
    let wanted = settings
        .value("macos", "identity", "AN_MACOS_IDENTITY")
        .ok_or_else(|| settings.missing("macos", "an macos --sign", block, variables))?;
    let (identity, identity_name) = find_developer_id(&wanted)?;

    let notary_profile = settings.value("macos", "notaryProfile", "AN_MACOS_NOTARY_PROFILE");
    if notarising && notary_profile.is_none() {
        bail!(
            "`an macos --notarize` needs a notarytool keychain profile, and neither \
             signing.macos.notaryProfile nor AN_MACOS_NOTARY_PROFILE says which one.\n\n\
             Create it once, with an app-specific password from \
             https://account.apple.com ▸ Sign-In and Security ▸ App-Specific Passwords:\n\n\
             \x20   xcrun notarytool store-credentials an-notary \\\n\
             \x20       --apple-id you@example.com \\\n\
             \x20       --team-id ABCDE12345 \\\n\
             \x20       --password xxxx-xxxx-xxxx-xxxx\n\n\
             Then put \"notaryProfile\": \"an-notary\" in {}. The password itself stays \
             in the keychain and never reaches the project.\nSee {DOCS}",
            settings.where_from()
        );
    }

    Ok(Macos { identity, identity_name, notary_profile })
}

/// Same lookup as on iOS, with a different sentence when there is nothing.
///
/// A Developer ID certificate is the one thing on this list that a free Apple ID
/// cannot produce, so the message says so: without that, somebody spends an
/// afternoon inside Xcode's account panel looking for a button that is not there
/// for them.
fn find_developer_id(wanted: &str) -> Result<(String, String)> {
    let found = codesigning_identities()?;
    if let Some((hash, name)) =
        found.iter().find(|(hash, name)| name.starts_with(wanted) || hash == wanted)
    {
        return Ok((hash.clone(), name.clone()));
    }
    bail!(
        "there is no codesigning identity whose name starts with {wanted:?}.\n\
         {}\n\
         A \"Developer ID Application\" certificate is created at \
         https://developer.apple.com/account/resources/certificates/list and needs the \
         paid Apple Developer Program: a free Apple ID cannot issue one, and without it \
         an app cannot be notarised.\n\
         For a build that only has to run on this machine, leave `--sign` off: \
         `an macos` signs ad hoc and needs nobody's account.\nSee {DOCS}",
        if found.is_empty() {
            "This keychain has no codesigning identity at all.".to_owned()
        } else {
            format!(
                "What there is:\n{}",
                found
                    .iter()
                    .map(|(_, name)| format!("\x20   {name}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        }
    )
}

// ---------------------------------------------------------------------------
// Android
// ---------------------------------------------------------------------------

/// A release keystore, its alias and the two passwords, all of them present.
pub struct Android {
    pub keystore: PathBuf,
    pub key_alias: String,
    pub store_password: String,
    pub key_password: String,
}

pub fn android(settings: &Settings) -> Result<Android> {
    let block = "  \"signing\": {\n    \
                 \"android\": {\n      \
                 \"keystore\": \"android/release.keystore\",\n      \
                 \"keyAlias\": \"upload\",\n      \
                 \"storePasswordEnv\": \"AN_ANDROID_KEYSTORE_PASSWORD\",\n      \
                 \"keyPasswordEnv\": \"AN_ANDROID_KEY_PASSWORD\"\n    }\n  }";
    let variables = "    export AN_ANDROID_KEYSTORE=/path/to/release.keystore\n\
                     \x20   export AN_ANDROID_KEY_ALIAS=upload\n\
                     \x20   export AN_ANDROID_KEYSTORE_PASSWORD='…'\n\
                     \x20   export AN_ANDROID_KEY_PASSWORD='…'";

    let keystore = settings
        .file("android", "keystore", "AN_ANDROID_KEYSTORE")
        .ok_or_else(|| settings.missing("android", "an android --sign", block, variables))?;
    settings.check_not_committed(&keystore, "release keystore")?;
    if !keystore.is_file() {
        bail!(
            "{} is missing, and it is the release keystore `an android --sign` signs \
             with.\n\
             If this app has never been published, create it —once, and keep it \
             forever: Play ties the app to this key and it cannot be swapped without \
             asking Google:\n\n\
             \x20   keytool -genkeypair -v -keystore {} \\\n\
             \x20       -alias upload -keyalg RSA -keysize 2048 -validity 10000\n\n\
             If it has, the keystore is the one you signed the first release with. \
             There is no copy of it anywhere else.\nSee {DOCS}",
            keystore.display(),
            keystore.display()
        );
    }

    let key_alias = settings
        .value("android", "keyAlias", "AN_ANDROID_KEY_ALIAS")
        .ok_or_else(|| {
            anyhow::anyhow!(
                "{} says which keystore to sign with but not which key inside it.\n\
                 Add \"keyAlias\" to signing.android, or export AN_ANDROID_KEY_ALIAS.\n\
                 `keytool -list -keystore {}` prints the aliases it holds.\nSee {DOCS}",
                settings.where_from(),
                keystore.display()
            )
        })?;

    let store_password = settings.password("android", "storePasswordEnv", "AN_ANDROID_KEYSTORE_PASSWORD")?;
    // The key password defaults to the keystore's, which is what `keytool`
    // itself does when you press return at its prompt, and what almost every
    // keystore in existence ends up with.
    let key_password = settings
        .password("android", "keyPasswordEnv", "AN_ANDROID_KEY_PASSWORD")
        .unwrap_or_else(|_| store_password.clone());

    Ok(Android { keystore, key_alias, store_password, key_password })
}

// ---------------------------------------------------------------------------
// Reading a plist by hand
// ---------------------------------------------------------------------------

/// The text of `<key>NAME</key><string>…</string>`.
fn plist_string(xml: &str, key: &str) -> Option<String> {
    let rest = after_key(xml, key)?;
    element(rest, "string")
}

/// The text of `<key>NAME</key><date>…</date>`.
fn plist_date(xml: &str, key: &str) -> Option<String> {
    let rest = after_key(xml, key)?;
    element(rest, "date")
}

/// The strings of `<key>NAME</key><array><string>…</string>…</array>`.
fn plist_string_array(xml: &str, key: &str) -> Vec<String> {
    let Some(rest) = after_key(xml, key) else { return Vec::new() };
    let Some(start) = rest.find("<array>") else { return Vec::new() };
    let rest = &rest[start + "<array>".len()..];
    let Some(end) = rest.find("</array>") else { return Vec::new() };
    let mut found = Vec::new();
    let mut inner = &rest[..end];
    while let Some(text) = element(inner, "string") {
        let cut = inner.find("</string>").map(|i| i + "</string>".len()).unwrap_or(inner.len());
        inner = &inner[cut..];
        found.push(text);
    }
    found
}

/// The `<key>…</key><string>…</string>` pairs of a dictionary, plus its
/// booleans and its arrays of strings.
///
/// It is the entitlements dictionary, and those three are what an entitlement
/// can be. Anything else in there —a `<data>`, a nested dictionary— is skipped
/// rather than guessed at: an entitlement written wrong is better missing than
/// invented.
fn plist_string_dict(xml: &str, key: &str) -> Map<String, Value> {
    let mut found = Map::new();
    let Some(rest) = after_key(xml, key) else { return found };
    let Some(start) = rest.find("<dict>") else { return found };
    let mut inner = &rest[start + "<dict>".len()..];
    // Where this dictionary ends: the first `</dict>` that is not closing a
    // nested one.
    let mut depth = 0usize;
    let mut end = inner.len();
    let mut scan = 0usize;
    while scan < inner.len() {
        let open = inner[scan..].find("<dict>").map(|i| scan + i);
        let close = inner[scan..].find("</dict>").map(|i| scan + i);
        match (open, close) {
            (Some(o), Some(c)) if o < c => {
                depth += 1;
                scan = o + "<dict>".len();
            }
            (_, Some(c)) => {
                if depth == 0 {
                    end = c;
                    break;
                }
                depth -= 1;
                scan = c + "</dict>".len();
            }
            _ => break,
        }
    }
    inner = &inner[..end];

    while let Some(open) = inner.find("<key>") {
        let after_open = &inner[open + "<key>".len()..];
        let Some(close) = after_open.find("</key>") else { break };
        let name = unescape(&after_open[..close]);
        let mut value_part = &after_open[close + "</key>".len()..];
        // Only as far as the next key, so a missing value does not steal the
        // one belonging to the entry after it.
        let limit = value_part.find("<key>").unwrap_or(value_part.len());
        value_part = &value_part[..limit];
        // In this order, and the order is the whole of it: an `<array>` of
        // strings begins with an `<array>` and *contains* a `<string>`, so
        // looking for the string first turns a list of keychain groups into the
        // first group on its own. Signed into the app, that is an entitlement
        // the profile does not grant and an app the system kills on launch.
        let trimmed = value_part.trim_start();
        let value = if trimmed.starts_with("<array>") {
            let items: Vec<Value> = plist_string_array(&format!("<key>x</key>{value_part}"), "x")
                .into_iter()
                .map(Value::String)
                .collect();
            Some(Value::Array(items))
        } else if trimmed.starts_with("<true/>") {
            Some(Value::Bool(true))
        } else if trimmed.starts_with("<false/>") {
            Some(Value::Bool(false))
        } else {
            element(value_part, "string").map(Value::String)
        };
        if let Some(value) = value {
            found.insert(name, value);
        }
        inner = &inner[open + "<key>".len() + close + "</key>".len()..];
    }
    found
}

fn after_key<'a>(xml: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("<key>{key}</key>");
    let at = xml.find(&needle)?;
    Some(&xml[at + needle.len()..])
}

fn element(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(unescape(&xml[start..end]))
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

// ---------------------------------------------------------------------------
// Today, in the only format a profile uses
// ---------------------------------------------------------------------------

/// The current instant as `YYYY-MM-DDTHH:MM:SSZ`.
///
/// Written out by hand instead of pulling in a date library, because the only
/// thing anybody does with it is compare it against a profile's
/// `ExpirationDate`, which comes in exactly this format and in UTC.
pub fn now_iso8601() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0);
    iso8601(seconds)
}

/// Seconds since the epoch to a civil date. Howard Hinnant's `civil_from_days`:
/// it shifts the year so it starts in March, which puts the leap day at the end
/// and makes the month lengths a straight line.
pub fn iso8601(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let rest = seconds % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rest / 3_600,
        (rest % 3_600) / 60,
        rest % 60
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// What is tested here is the half of signing that does not need a credential:
/// reading a profile, deciding whether it is expired, deciding whether it is
/// about this app, and refusing a manifest that carries a password. All four run
/// on any machine, which is the point — the other half cannot be tested without
/// an Apple Developer account, and a test that only passes for one person is
/// worse than none.
#[cfg(test)]
mod tests {
    use super::*;

    /// A decoded profile, the way `security cms -D` prints one.
    fn profile(app_id: &str, expires: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
    <key>Name</key>
    <string>angular-native development</string>
    <key>TeamIdentifier</key>
    <array>
        <string>ABCDE12345</string>
    </array>
    <key>DeveloperCertificates</key>
    <array>
        <data>bm90IHJlYWxseSBhIGNlcnRpZmljYXRl</data>
    </array>
    <key>Entitlements</key>
    <dict>
        <key>application-identifier</key>
        <string>{app_id}</string>
        <key>com.apple.developer.team-identifier</key>
        <string>ABCDE12345</string>
        <key>get-task-allow</key>
        <true/>
        <key>keychain-access-groups</key>
        <array>
            <string>ABCDE12345.*</string>
        </array>
    </dict>
    <key>ExpirationDate</key>
    <date>{expires}</date>
</dict>
</plist>
"#
        )
    }

    fn facts(app_id: &str, expires: &str) -> ProfileFacts {
        ProfileFacts::parse(&profile(app_id, expires), Path::new("test.mobileprovision"))
            .expect("the test profile parses")
    }

    #[test]
    fn a_profile_is_read_field_by_field() {
        let parsed = facts("ABCDE12345.com.example.myapp", "2999-01-01T00:00:00Z");
        assert_eq!(parsed.name, "angular-native development");
        assert_eq!(parsed.teams, vec!["ABCDE12345".to_owned()]);
        assert_eq!(parsed.expires, "2999-01-01T00:00:00Z");
        assert_eq!(
            parsed.application_identifier(),
            Some("ABCDE12345.com.example.myapp")
        );
        // The booleans and the arrays come through too: they are entitlements
        // like any other and they get signed into the app.
        assert_eq!(parsed.entitlements.get("get-task-allow"), Some(&Value::Bool(true)));
        assert_eq!(
            parsed.entitlements.get("keychain-access-groups"),
            Some(&Value::Array(vec![Value::String("ABCDE12345.*".to_owned())]))
        );
        // And the `<data>` above it is skipped rather than turned into
        // something. It is not an entitlement and it is not in the dictionary.
        assert!(parsed.entitlements.get("DeveloperCertificates").is_none());
    }

    #[test]
    fn an_expired_profile_is_refused_and_says_when() {
        let parsed = facts("ABCDE12345.com.example.myapp", "2020-03-04T10:00:00Z");
        let error = parsed
            .check_not_expired(Path::new("dev.mobileprovision"), "2026-09-03T00:00:00Z")
            .expect_err("an expired profile has to be refused");
        let text = format!("{error}");
        assert!(text.contains("2020-03-04"), "it says when it expired: {text}");
        assert!(text.contains("developer.apple.com"), "and where to renew it: {text}");
    }

    #[test]
    fn a_profile_that_expires_today_is_still_good() {
        let parsed = facts("ABCDE12345.com.example.myapp", "2026-09-03T23:59:59Z");
        assert!(parsed
            .check_not_expired(Path::new("dev.mobileprovision"), "2026-09-03T09:00:00Z")
            .is_ok());
    }

    #[test]
    fn a_profile_for_another_app_is_refused_and_names_both() {
        let parsed = facts("ABCDE12345.com.example.other", "2999-01-01T00:00:00Z");
        let error = parsed
            .check_bundle_id(Path::new("dev.mobileprovision"), "com.example.myapp")
            .expect_err("a profile for another app has to be refused");
        let text = format!("{error}");
        assert!(text.contains("com.example.other"), "it names the profile's: {text}");
        assert!(text.contains("com.example.myapp"), "and the app's: {text}");
    }

    #[test]
    fn a_wildcard_profile_covers_the_app() {
        let parsed = facts("ABCDE12345.*", "2999-01-01T00:00:00Z");
        assert!(parsed
            .check_bundle_id(Path::new("dev.mobileprovision"), "com.example.myapp")
            .is_ok());
        assert!(matches("com.example.*", "com.example.myapp"));
        assert!(!matches("com.example.*", "com.other.myapp"));
        assert!(!matches("com.example.myapp", "com.example.myapp.tv"));
    }

    #[test]
    fn a_password_in_the_manifest_is_refused_naming_the_key() {
        let sections: Map<String, Value> = serde_json::from_str(
            r#"{ "android": { "keystore": "android/release.keystore", "storePassword": "hunter2" } }"#,
        )
        .expect("valid JSON");
        let error = check_for_secrets(Path::new("angular-native.json"), &sections)
            .expect_err("a literal password has to be refused");
        let text = format!("{error}");
        assert!(text.contains("signing.android.storePassword"), "it names the key: {text}");
        assert!(text.contains("storePasswordEnv"), "and what to write instead: {text}");
    }

    #[test]
    fn the_env_form_of_the_same_key_is_fine() {
        let sections: Map<String, Value> = serde_json::from_str(
            r#"{ "android": { "storePasswordEnv": "AN_ANDROID_KEYSTORE_PASSWORD" } }"#,
        )
        .expect("valid JSON");
        assert!(check_for_secrets(Path::new("angular-native.json"), &sections).is_ok());
    }

    #[test]
    fn identities_are_read_off_securitys_listing() {
        let listing = "\
  1) 1A2B3C4D5E6F708192A3B4C5D6E7F80910111213 \"Apple Development: Jane Doe (7A8B9C0D1E)\"
  2) 0102030405060708090A0B0C0D0E0F1011121314 \"Developer ID Application: Acme S.L. (ABCDE12345)\"
     2 valid identities found";
        let found = parse_identities(listing);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].0, "1A2B3C4D5E6F708192A3B4C5D6E7F80910111213");
        assert_eq!(found[1].1, "Developer ID Application: Acme S.L. (ABCDE12345)");
    }

    #[test]
    fn an_empty_keychain_reads_as_empty_and_not_as_a_parse_error() {
        assert!(parse_identities("     0 valid identities found").is_empty());
    }

    #[test]
    fn the_clock_agrees_with_a_profiles_date_format() {
        assert_eq!(iso8601(1_788_438_896), "2026-09-03T12:34:56Z");
        assert_eq!(iso8601(0), "1970-01-01T00:00:00Z");
        // A leap day, which is the one the shifted-year arithmetic exists for.
        assert_eq!(iso8601(1_709_164_800), "2024-02-29T00:00:00Z");
        // And the shape is the profile's, so the string comparison holds.
        let now = now_iso8601();
        assert_eq!(now.len(), 20, "{now}");
        assert!(now.ends_with('Z'), "{now}");
    }
}
