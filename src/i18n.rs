// Gosh AppImage Manager — localization.
//
// The AppStream metadata used to declare a gettext domain while no catalog,
// no dependency and no lookup existed anywhere; every string was an inline
// English literal. This is the machinery that claim needed.
//
// Design: the English text stays at the call site as the fallback, so the
// source reads normally and a missing translation degrades to English rather
// than to a key. Catalogs are plain JSON keyed by message id, discovered at
// runtime, so a translation can be added without rebuilding.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// A resolved locale: language, optional region, and writing direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locale {
    /// Lowercase ISO-639 language, e.g. "de".
    pub language: String,
    /// Uppercase region if the environment named one, e.g. "AT".
    pub region: Option<String>,
    pub direction: Direction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    LeftToRight,
    RightToLeft,
}

/// Languages written right to left.
///
/// Layout mirroring is the toolkit's job, but knowing the direction is ours:
/// nothing could even ask before, which is why "no RTL consideration" was a
/// fair description.
const RTL_LANGUAGES: &[&str] = &["ar", "he", "fa", "ur", "ps", "sd", "ug", "yi", "dv", "ckb"];

impl Locale {
    /// Read the locale from the environment, POSIX precedence.
    ///
    /// `LC_ALL` overrides `LC_MESSAGES`, which overrides `LANG`. The C and
    /// POSIX locales mean "no localization", not a language called "c".
    pub fn from_env() -> Self {
        for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(value) = std::env::var(key) {
                if let Some(locale) = Self::parse(&value) {
                    return locale;
                }
            }
        }
        Self::english()
    }

    pub fn english() -> Self {
        Self {
            language: "en".to_string(),
            region: None,
            direction: Direction::LeftToRight,
        }
    }

    /// Parse a POSIX locale string such as `de_AT.UTF-8@euro`.
    pub fn parse(raw: &str) -> Option<Self> {
        // Strip the codeset and any modifier; neither selects a translation.
        let base = raw.split(['.', '@']).next().unwrap_or("").trim();
        if base.is_empty() {
            return None;
        }
        let lower = base.to_ascii_lowercase();
        if lower == "c" || lower == "posix" {
            return None;
        }
        let mut parts = base.split(['_', '-']);
        let language = parts.next()?.to_ascii_lowercase();
        if language.len() < 2 || !language.chars().all(|c| c.is_ascii_alphabetic()) {
            return None;
        }
        let region = parts
            .next()
            .filter(|r| r.len() == 2 && r.chars().all(|c| c.is_ascii_alphabetic()))
            .map(|r| r.to_ascii_uppercase());
        let direction = if RTL_LANGUAGES.contains(&language.as_str()) {
            Direction::RightToLeft
        } else {
            Direction::LeftToRight
        };
        Some(Self {
            language,
            region,
            direction,
        })
    }

    /// Catalog names to try, most specific first: `de-AT`, then `de`.
    pub fn candidates(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(region) = &self.region {
            out.push(format!("{}-{}", self.language, region));
        }
        out.push(self.language.clone());
        out
    }

    pub fn is_rtl(&self) -> bool {
        self.direction == Direction::RightToLeft
    }
}

/// Message id to translated text.
#[derive(Debug, Default, Clone)]
pub struct Catalog {
    locale: Option<Locale>,
    messages: BTreeMap<String, String>,
}

impl Catalog {
    /// An empty catalog: every lookup falls back to the source English.
    pub fn source_only() -> Self {
        Self {
            locale: Some(Locale::english()),
            messages: BTreeMap::new(),
        }
    }

    pub fn from_json(locale: Locale, body: &str) -> Result<Self, String> {
        let messages: BTreeMap<String, String> =
            serde_json::from_str(body).map_err(|e| format!("Invalid catalog: {e}"))?;
        Ok(Self {
            locale: Some(locale),
            messages,
        })
    }

    /// Load the best catalog for `locale` from a directory of `<tag>.json`.
    pub fn load(dir: &Path, locale: &Locale) -> Option<Self> {
        for tag in locale.candidates() {
            let path = dir.join(format!("{tag}.json"));
            if let Ok(body) = std::fs::read_to_string(&path) {
                if let Ok(catalog) = Self::from_json(locale.clone(), &body) {
                    return Some(catalog);
                }
            }
        }
        None
    }

    pub fn locale(&self) -> Locale {
        self.locale.clone().unwrap_or_else(Locale::english)
    }

    pub fn is_rtl(&self) -> bool {
        self.locale().is_rtl()
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Translate `id`, falling back to the English written at the call site.
    ///
    /// Falling back to the source text rather than to the id means a partial
    /// catalog produces a partly-translated interface instead of one strewn
    /// with `library.title`.
    pub fn tr<'a>(&'a self, id: &str, source: &'a str) -> std::borrow::Cow<'a, str> {
        match self.messages.get(id) {
            Some(text) if !text.is_empty() => std::borrow::Cow::Borrowed(text.as_str()),
            _ => {
                if self.pseudo() {
                    std::borrow::Cow::Owned(pseudolocalize(source))
                } else {
                    std::borrow::Cow::Borrowed(source)
                }
            }
        }
    }

    /// Is this the pseudolocale used to check localizability?
    fn pseudo(&self) -> bool {
        self.locale.as_ref().is_some_and(|l| l.language == "qps")
    }
}

/// Where catalogs are looked for, in order.
///
/// `GOSHAIM_LOCALE_DIR` first so a translator can point at a working copy,
/// then the installed location beside the binary's data.
fn catalog_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(explicit) = std::env::var_os("GOSHAIM_LOCALE_DIR") {
        dirs.push(PathBuf::from(explicit));
    }
    dirs.push(PathBuf::from("/app/share/gosh-appimage-manager/i18n"));
    dirs.push(PathBuf::from("/usr/share/gosh-appimage-manager/i18n"));
    dirs.push(PathBuf::from("i18n"));
    dirs
}

static ACTIVE: OnceLock<Catalog> = OnceLock::new();

/// The catalog for this process, resolved once.
pub fn active() -> &'static Catalog {
    ACTIVE.get_or_init(|| {
        let locale = Locale::from_env();
        if locale.language == "en" {
            return Catalog {
                locale: Some(locale),
                messages: BTreeMap::new(),
            };
        }
        for dir in catalog_dirs() {
            if let Some(catalog) = Catalog::load(&dir, &locale) {
                return catalog;
            }
        }
        // No catalog for this locale: English, but remember the direction so
        // the interface can still be laid out correctly.
        Catalog {
            locale: Some(locale),
            messages: BTreeMap::new(),
        }
    })
}

/// Install a catalog for tests. Has no effect once one is resolved.
pub fn set_active_for_test(catalog: Catalog) -> bool {
    ACTIVE.set(catalog).is_ok()
}

/// Translate at the call site: `t!("library.title", "Library")`.
///
/// The English stays inline so the source reads as prose and a missing
/// translation degrades to English rather than to a message id.
#[macro_export]
macro_rules! t {
    ($id:expr, $source:expr) => {
        $crate::i18n::active().tr($id, $source).into_owned()
    };
}

/// Transform text so untranslated strings and tight layouts stand out.
///
/// A standard technique: accent every letter so anything left in plain ASCII
/// was never routed through the catalog, and pad the string so a layout that
/// only fits English is visibly too small. Deliberately still readable.
pub fn pseudolocalize(source: &str) -> String {
    let mut out = String::with_capacity(source.len() + source.len() / 3 + 2);
    out.push('[');
    for ch in source.chars() {
        out.push(match ch {
            'a' => 'á',
            'b' => 'ḃ',
            'c' => 'ç',
            'd' => 'ð',
            'e' => 'é',
            'f' => 'ƒ',
            'g' => 'ĝ',
            'h' => 'ĥ',
            'i' => 'í',
            'j' => 'ĵ',
            'k' => 'ķ',
            'l' => 'ļ',
            'm' => 'ɱ',
            'n' => 'ñ',
            'o' => 'ó',
            'p' => 'ƥ',
            'q' => 'ɋ',
            'r' => 'ŕ',
            's' => 'š',
            't' => 'ţ',
            'u' => 'ú',
            'v' => 'ṽ',
            'w' => 'ŵ',
            'x' => 'ẋ',
            'y' => 'ý',
            'z' => 'ž',
            'A' => 'Á',
            'B' => 'Ḃ',
            'C' => 'Ç',
            'D' => 'Ð',
            'E' => 'É',
            'F' => 'Ƒ',
            'G' => 'Ĝ',
            'H' => 'Ĥ',
            'I' => 'Í',
            'J' => 'Ĵ',
            'K' => 'Ķ',
            'L' => 'Ļ',
            'M' => 'Ṁ',
            'N' => 'Ñ',
            'O' => 'Ó',
            'P' => 'Ƥ',
            'Q' => 'Ɋ',
            'R' => 'Ŕ',
            'S' => 'Š',
            'T' => 'Ţ',
            'U' => 'Ú',
            'V' => 'Ṽ',
            'W' => 'Ŵ',
            'X' => 'Ẋ',
            'Y' => 'Ý',
            'Z' => 'Ž',
            other => other,
        });
    }
    // ~30% expansion, roughly what German and Finnish need over English.
    let padding = (source.chars().count() / 3).max(1);
    out.extend(std::iter::repeat_n('·', padding));
    out.push(']');
    out
}
