// Audit finding G-10: the AppStream metadata declared a gettext domain while
// no catalog, no dependency and no lookup existed; every string was an inline
// English literal and nothing could ask which direction a locale is written
// in. These cover the machinery that claim needed.

use goshaim_core::i18n::{pseudolocalize, Catalog, Direction, Locale};

#[test]
fn posix_locale_strings_are_parsed() {
    let de = Locale::parse("de_AT.UTF-8@euro").expect("should parse");
    assert_eq!(de.language, "de");
    assert_eq!(de.region.as_deref(), Some("AT"));
    assert_eq!(de.direction, Direction::LeftToRight);
    // Most specific first, so a regional catalog wins over the language one.
    assert_eq!(de.candidates(), vec!["de-AT".to_string(), "de".to_string()]);

    let plain = Locale::parse("fr").expect("should parse");
    assert_eq!(plain.language, "fr");
    assert_eq!(plain.region, None);
    assert_eq!(plain.candidates(), vec!["fr".to_string()]);

    assert_eq!(
        Locale::parse("pt-BR").unwrap().region.as_deref(),
        Some("BR")
    );

    // C and POSIX mean "no localization", not a language.
    assert!(Locale::parse("C").is_none());
    assert!(Locale::parse("POSIX").is_none());
    assert!(Locale::parse("").is_none());
    assert!(Locale::parse("   ").is_none());
    assert!(Locale::parse("1").is_none());
}

#[test]
fn right_to_left_languages_are_recognised() {
    for rtl in ["ar", "he_IL", "fa_IR.UTF-8", "ur", "ckb"] {
        let locale = Locale::parse(rtl).expect(rtl);
        assert!(locale.is_rtl(), "{rtl} should be right-to-left");
    }
    for ltr in ["en_GB", "de", "ja_JP.UTF-8", "pt-BR", "fi"] {
        let locale = Locale::parse(ltr).expect(ltr);
        assert!(!locale.is_rtl(), "{ltr} should be left-to-right");
    }
}

#[test]
fn a_missing_message_falls_back_to_the_source_english() {
    let catalog = Catalog::from_json(
        Locale::parse("de").unwrap(),
        r#"{"nav.library":"Bibliothek","action.launch":""}"#,
    )
    .expect("catalog should parse");

    assert_eq!(catalog.tr("nav.library", "Library"), "Bibliothek");
    // Absent from the catalog: the interface stays English rather than
    // showing a message id.
    assert_eq!(catalog.tr("nav.updates", "Updates"), "Updates");
    // Present but empty counts as untranslated, not as an empty label.
    assert_eq!(catalog.tr("action.launch", "Launch"), "Launch");
}

#[test]
fn a_malformed_catalog_is_rejected_rather_than_half_loaded() {
    let locale = Locale::parse("de").unwrap();
    assert!(Catalog::from_json(locale.clone(), "{ not json").is_err());
    assert!(Catalog::from_json(locale.clone(), r#"{"k":123}"#).is_err());
    assert!(Catalog::from_json(locale, "{}").is_ok());
}

#[test]
fn catalogs_are_loaded_most_specific_first() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("de.json"), r#"{"k":"language"}"#).unwrap();
    std::fs::write(dir.path().join("de-AT.json"), r#"{"k":"region"}"#).unwrap();

    let regional = Catalog::load(dir.path(), &Locale::parse("de_AT").unwrap()).unwrap();
    assert_eq!(regional.tr("k", "source"), "region");

    // No regional catalog: fall back to the language one.
    let language = Catalog::load(dir.path(), &Locale::parse("de_CH").unwrap()).unwrap();
    assert_eq!(language.tr("k", "source"), "language");

    // No catalog at all for this language.
    assert!(Catalog::load(dir.path(), &Locale::parse("ja").unwrap()).is_none());
}

/// The pseudolocale is how an untranslated string is found: anything still in
/// plain ASCII was never routed through the catalog.
#[test]
fn the_pseudolocale_marks_and_expands_text() {
    let out = pseudolocalize("Launch");
    assert!(out.starts_with('['), "should be bracketed: {out}");
    assert!(out.ends_with(']'), "should be bracketed: {out}");
    assert!(
        !out.contains("Launch"),
        "every letter should be accented, so untranslated text stands out: {out}"
    );
    // Still readable, and visibly longer so a too-tight layout shows up.
    assert!(out.contains('Ļ') && out.contains('á'));
    assert!(
        out.chars().count() > "Launch".chars().count() + 2,
        "should expand to model a longer language: {out}"
    );
    // Punctuation, digits and placeholders survive.
    let mixed = pseudolocalize("3 of 5 — done!");
    assert!(mixed.contains('3') && mixed.contains('5'));
    assert!(mixed.contains('—') && mixed.contains('!'));
}

/// A pseudolocale catalog transforms even the untranslated fallbacks, which
/// is the whole point of it.
#[test]
fn the_pseudolocale_applies_to_fallbacks() {
    let catalog = Catalog::from_json(Locale::parse("qps").unwrap(), "{}").unwrap();
    let text = catalog.tr("anything", "Update all");
    assert!(text.starts_with('['), "got: {text}");
    assert!(!text.contains("Update all"), "got: {text}");
}

/// English needs no catalog and must not be pseudolocalized.
#[test]
fn english_passes_through_untouched() {
    let catalog = Catalog::source_only();
    assert_eq!(catalog.tr("nav.library", "Library"), "Library");
    assert!(catalog.is_empty());
    assert!(!catalog.is_rtl());
}

/// Every message id used in the interface must be unique to one English
/// string: two different strings sharing an id would make one untranslatable.
#[test]
fn message_ids_map_to_exactly_one_source_string() {
    use std::collections::HashMap;
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/gui.rs"))
        .expect("gui.rs should be readable");
    let mut seen: HashMap<String, String> = HashMap::new();
    let mut count = 0;
    // t!("id", "English")
    for capture in source.split("t!(\"").skip(1) {
        let Some((id, rest)) = capture.split_once("\",") else {
            continue;
        };
        let rest = rest.trim_start();
        let Some(text) = rest.strip_prefix('"') else {
            continue;
        };
        let Some((english, _)) = text.split_once("\")") else {
            continue;
        };
        count += 1;
        if let Some(previous) = seen.get(id) {
            assert_eq!(
                previous, english,
                "message id {id} is used for two different strings"
            );
        }
        seen.insert(id.to_string(), english.to_string());
    }
    assert!(
        count >= 40,
        "expected the interface to be routed through the catalog, found {count} call sites"
    );
}
