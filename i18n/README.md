# Translations

Message catalogs are JSON objects mapping a message id to translated text:

```json
{
  "nav.library": "Bibliothek",
  "action.launch": "Starten"
}
```

Name the file after the locale — `de.json`, or `de-AT.json` for a
region-specific variant, which is tried first. Anything a catalog omits falls
back to the English written at the call site, so a partial translation gives a
partly-translated interface rather than one strewn with message ids.

Catalogs are read at runtime from, in order:

1. `$GOSHAIM_LOCALE_DIR` — point this at a working copy while translating
2. `/app/share/gosh-appimage-manager/i18n` (Flatpak)
3. `/usr/share/gosh-appimage-manager/i18n`
4. `./i18n`

The locale comes from `LC_ALL`, `LC_MESSAGES`, then `LANG`, in POSIX order.
`C` and `POSIX` mean "no localization" and select the English source.

## Checking a translation

`qps.json` is a pseudolocale, not a language. Running with it accents every
letter and pads each string by about a third:

```sh
GOSHAIM_LOCALE_DIR=./i18n LC_ALL=qps cargo run --features gui
```

Anything still in plain unaccented ASCII was never routed through the catalog
and cannot be translated. Anything clipped or overlapping is a layout that
only fits English — real translations of this interface run 20–35% longer.

## What is not translated

Machine-readable CLI output — the JSON documents, the tab-separated listings,
the exit codes — stays in English by design, because scripts parse it. The
graphical interface is what these catalogs cover.

The product name, "Gosh AppImage Manager", is not translated either.

## Right-to-left

Arabic, Hebrew, Persian, Urdu and the other right-to-left languages are
detected from the locale and reported by `i18n::active().is_rtl()`, so layout
can respond to direction. Mirroring itself is the toolkit's to apply.
