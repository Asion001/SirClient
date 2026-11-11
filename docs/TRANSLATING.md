# Translating SirClient

SirClient uses [Project Fluent](https://projectfluent.org/) (`.ftl` files).
Fluent handles plurals and other grammar per language, and translation
platforms such as Weblate and Crowdin support it directly.

## Add a language

1. Copy `crates/sirclient-gui/locales/en-US.ftl` to
   `crates/sirclient-gui/locales/<language-id>.ftl`, using a BCP 47 id such
   as `de-DE`, `pt-BR`, `uk-UA` or `ja-JP`.
2. Translate the values. Keep the message ids (left of `=`) and
   `{ $variables }` unchanged.
3. Set `language-name` to the language's own name (for example
   `Deutsch`); the language picker shows it.
4. Build. Every `.ftl` file in that folder is compiled in automatically.

Anything you leave out falls back to English, so partial translations work.

## Try changes without rebuilding

Point `SIRCLIENT_LOCALE_DIR` at a folder of `.ftl` files. Files there
override the built-in ones with the same name:

```bash
SIRCLIENT_LOCALE_DIR=./my-locales cargo run -p sirclient-gui -- --mock
```

## Check your file

```bash
cargo test -p sirclient-gui
```

The test fails if a locale has syntax errors or defines ids that do not
exist in `en-US.ftl` (usually a typo).

## Scripts

The bundled font covers Latin, Greek and Cyrillic. Chinese, Japanese, Korean,
Arabic and other scripts need a font that covers them; loading a system font
for those locales is still to do.
