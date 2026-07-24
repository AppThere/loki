// SPDX-License-Identifier: Apache-2.0

//! Fluent bundle construction and message lookup.
//!
//! Uses `fluent_bundle::concurrent::FluentBundle` so the bundle is `Send +
//! Sync` and can live in a `OnceLock<>` static.  The concurrent variant uses
//! a `Mutex`-based memoizer instead of `RefCell`.

use fluent::{FluentArgs, FluentResource};
use fluent_bundle::concurrent::FluentBundle;
use unic_langid::LanguageIdentifier;

use crate::embed::LokiTranslations;

/// All translation domains; each maps to one `.ftl` file per locale.
const DOMAINS: &[&str] = &[
    "shell", "home", "editor", "ribbon", "errors", "document", "publish", "style", "macros",
];

/// Locale used when the system locale has no embedded translations,
/// constructed at compile time (no runtime parse can fail).
fn fallback_locale() -> LanguageIdentifier {
    unic_langid::langid!("en-US")
}

/// Type alias for the concurrent Fluent bundle.
type Bundle = FluentBundle<FluentResource>;

/// A loaded Fluent bundle for a specific locale, with an `en-US` fallback.
///
/// Both fields use the concurrent memoizer so the struct is `Send + Sync`
/// and can be stored in a `static OnceLock`.
pub struct LokiBundle {
    bundle: Bundle,
    fallback: Option<Bundle>,
}

impl LokiBundle {
    /// Loads the bundle for `locale_str` (e.g. `"fr-FR"`).
    ///
    /// Falls back to `en-US` when the requested locale has no embedded files.
    /// A separate `en-US` fallback bundle is kept so keys missing from the
    /// primary locale still resolve.
    pub fn load(locale_str: &str) -> Self {
        let langid: LanguageIdentifier = locale_str.parse().unwrap_or_else(|_| fallback_locale());

        let bundle = load_locale(&langid);

        // Keep an en-US fallback bundle unless the requested locale resolves
        // *exactly* to en-US. Comparing the parsed `LanguageIdentifier` (rather
        // than a `starts_with("en-US")` string check) is important: variants
        // like `en-US-posix` have no embedded `.ftl` files of their own, so
        // without a fallback every key would render as its raw identifier.
        let fb_id: LanguageIdentifier = fallback_locale();
        let fallback = if langid == fb_id {
            None
        } else {
            Some(load_locale(&fb_id))
        };

        LokiBundle { bundle, fallback }
    }

    /// Translates `key`, interpolating `args` if provided.
    ///
    /// Lookup order:
    /// 1. Primary locale bundle.
    /// 2. `en-US` fallback bundle (when primary is not `en-US`).
    /// 3. The key string itself — so missing translations surface in dev.
    pub fn get(&self, key: &str, args: Option<&FluentArgs<'_>>) -> String {
        if let Some(s) = format_from(&self.bundle, key, args) {
            return s;
        }
        if let Some(fb) = &self.fallback {
            if let Some(s) = format_from(fb, key, args) {
                return s;
            }
        }
        key.to_string()
    }
}

/// Loads a concurrent [`Bundle`] for `langid` from all embedded `.ftl` domains.
fn load_locale(langid: &LanguageIdentifier) -> Bundle {
    let mut bundle = FluentBundle::new_concurrent(vec![langid.clone()]);

    for domain in DOMAINS {
        let path = format!("{langid}/{domain}.ftl");
        if let Some(file) = LokiTranslations::get(&path) {
            let source = String::from_utf8_lossy(&file.data).into_owned();
            match FluentResource::try_new(source) {
                Ok(res) => {
                    // Duplicate-message errors indicate a .ftl authoring mistake.
                    let _ = bundle.add_resource(res);
                }
                Err((res, _)) => {
                    // Partially-parsed resource — add what succeeded.
                    let _ = bundle.add_resource(res);
                }
            }
        }
    }

    bundle
}

/// Formats a single message from `bundle`, returning `None` if the key is absent.
fn format_from(bundle: &Bundle, key: &str, args: Option<&FluentArgs<'_>>) -> Option<String> {
    let msg = bundle.get_message(key)?;
    let pattern = msg.value()?;
    let mut errors = vec![];
    let value = bundle.format_pattern(pattern, args, &mut errors);
    Some(value.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    // A key that exists in en-US/shell.ftl. Used to distinguish a resolved
    // translation from the raw-key fallback.
    const KNOWN_KEY: &str = "shell-app-name";
    const KNOWN_VALUE: &str = "Loki Text";

    #[test]
    fn exact_en_us_resolves() {
        let b = LokiBundle::load("en-US");
        assert_eq!(b.get(KNOWN_KEY, None), KNOWN_VALUE);
    }

    #[test]
    fn en_us_posix_resolves_via_fallback() {
        // Regression: `en-US-posix` has no embedded .ftl files of its own.
        // Before the fix it was treated as en-US (no fallback) and rendered the
        // raw key. It must now resolve through the en-US fallback bundle.
        let b = LokiBundle::load("en-US-posix");
        assert_eq!(b.get(KNOWN_KEY, None), KNOWN_VALUE);
    }

    #[test]
    fn unknown_locale_falls_back_to_en_us() {
        let b = LokiBundle::load("zz-ZZ");
        assert_eq!(b.get(KNOWN_KEY, None), KNOWN_VALUE);
    }

    #[test]
    fn bare_language_subtag_falls_back() {
        // `en` (no region) likewise has no embedded files and must fall back.
        let b = LokiBundle::load("en");
        assert_eq!(b.get(KNOWN_KEY, None), KNOWN_VALUE);
    }

    #[test]
    fn missing_key_returns_the_key_itself() {
        let b = LokiBundle::load("en-US");
        let missing = "this-key-does-not-exist";
        assert_eq!(b.get(missing, None), missing);
    }

    #[test]
    fn garbage_locale_string_does_not_panic() {
        // An unparseable locale must degrade to en-US, not panic.
        let b = LokiBundle::load("!!!not-a-locale!!!");
        assert_eq!(b.get(KNOWN_KEY, None), KNOWN_VALUE);
    }
}
