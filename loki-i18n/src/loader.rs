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
const DOMAINS: &[&str] = &["shell", "home", "editor", "ribbon", "errors", "document"];

/// Locale used when the system locale has no embedded translations.
const FALLBACK_LOCALE: &str = "en-US";

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
        let langid: LanguageIdentifier = locale_str
            .parse()
            .unwrap_or_else(|_| FALLBACK_LOCALE.parse().expect("en-US is valid"));

        let bundle = load_locale(&langid);

        let fallback = if !locale_str.starts_with("en-US") {
            let fb_id: LanguageIdentifier = FALLBACK_LOCALE.parse().expect("en-US is valid");
            Some(load_locale(&fb_id))
        } else {
            None
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
