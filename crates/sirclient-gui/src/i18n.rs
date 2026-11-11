//! Localization with Project Fluent.
//!
//! Every user-visible string lives in `locales/<language-id>.ftl`. Files in
//! that folder are compiled in; a folder named by `SIRCLIENT_LOCALE_DIR` is
//! read at start-up as well, so translators can try changes without
//! rebuilding. Missing messages fall back to English.

use std::cell::RefCell;

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

include!(concat!(env!("OUT_DIR"), "/locales.rs"));

pub const FALLBACK: &str = "en-US";

struct Locale {
    id: String,
    name: String,
    bundle: FluentBundle<FluentResource>,
}

struct L10n {
    locales: Vec<Locale>,
    current: usize,
    fallback: usize,
}

thread_local! {
    static L10N: RefCell<Option<L10n>> = const { RefCell::new(None) };
}

fn build(id: &str, src: String) -> Option<Locale> {
    let lang: LanguageIdentifier = id.parse().ok()?;
    let res = match FluentResource::try_new(src) {
        Ok(r) => r,
        Err((r, errs)) => {
            log::warn!(
                "{id}.ftl: {} syntax error(s), using what parsed",
                errs.len()
            );
            r
        }
    };
    let mut bundle = FluentBundle::new(vec![lang]);
    // egui would show the Unicode isolation marks as boxes.
    bundle.set_use_isolating(false);
    if let Err(errs) = bundle.add_resource(res) {
        log::warn!("{id}.ftl: {errs:?}");
    }
    let mut l = Locale {
        id: id.to_string(),
        name: id.to_string(),
        bundle,
    };
    if let Some(n) = format(&l.bundle, "language-name", None) {
        l.name = n;
    }
    Some(l)
}

fn format(
    b: &FluentBundle<FluentResource>,
    key: &str,
    args: Option<&FluentArgs>,
) -> Option<String> {
    let msg = b.get_message(key)?;
    let pattern = msg.value()?;
    let mut errs = vec![];
    Some(b.format_pattern(pattern, args, &mut errs).into_owned())
}

fn pick(locales: &[Locale], wanted: &str) -> Option<usize> {
    let wanted = wanted.replace('_', "-");
    let wanted = wanted.split('.').next().unwrap_or("");
    locales
        .iter()
        .position(|l| l.id.eq_ignore_ascii_case(wanted))
        .or_else(|| {
            let lang = wanted.split('-').next()?;
            locales.iter().position(|l| {
                l.id.split('-')
                    .next()
                    .is_some_and(|x| x.eq_ignore_ascii_case(lang))
            })
        })
}

/// Load all locales and select `preferred`, else the system language.
pub fn init(preferred: Option<&str>) {
    let mut sources: Vec<(String, String)> = EMBEDDED
        .iter()
        .map(|(id, src)| (id.to_string(), src.to_string()))
        .collect();
    if let Ok(dir) = std::env::var("SIRCLIENT_LOCALE_DIR")
        && let Ok(rd) = std::fs::read_dir(&dir)
    {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "ftl")
                && let (Some(stem), Ok(src)) = (p.file_stem(), std::fs::read_to_string(&p))
            {
                let id = stem.to_string_lossy().to_string();
                sources.retain(|(i, _)| *i != id);
                sources.push((id, src));
            }
        }
    }
    let mut locales: Vec<Locale> = sources
        .into_iter()
        .filter_map(|(id, src)| build(&id, src))
        .collect();
    locales.sort_by(|a, b| a.name.cmp(&b.name));
    let fallback = locales.iter().position(|l| l.id == FALLBACK).unwrap_or(0);
    let system = sys_locale::get_locale();
    let current = preferred
        .and_then(|p| pick(&locales, p))
        .or_else(|| system.as_deref().and_then(|s| pick(&locales, s)))
        .unwrap_or(fallback);
    L10N.with(|l| {
        *l.borrow_mut() = Some(L10n {
            locales,
            current,
            fallback,
        })
    });
}

/// (id, native name) of every available language.
pub fn available() -> Vec<(String, String)> {
    L10N.with(|l| {
        l.borrow()
            .as_ref()
            .map(|l| {
                l.locales
                    .iter()
                    .map(|x| (x.id.clone(), x.name.clone()))
                    .collect()
            })
            .unwrap_or_default()
    })
}

pub fn current() -> String {
    L10N.with(|l| {
        l.borrow()
            .as_ref()
            .map(|l| l.locales[l.current].id.clone())
            .unwrap_or_default()
    })
}

pub fn set(id: &str) {
    L10N.with(|l| {
        if let Some(l) = l.borrow_mut().as_mut()
            && let Some(i) = pick(&l.locales, id)
        {
            l.current = i;
        }
    });
}

fn lookup(key: &str, args: Option<&FluentArgs>) -> String {
    L10N.with(|l| {
        let l = l.borrow();
        let Some(l) = l.as_ref() else {
            return key.to_string();
        };
        format(&l.locales[l.current].bundle, key, args)
            .or_else(|| format(&l.locales[l.fallback].bundle, key, args))
            .unwrap_or_else(|| {
                log::warn!("missing translation: {key}");
                key.to_string()
            })
    })
}

/// Translate a message.
pub fn t(key: &str) -> String {
    lookup(key, None)
}

/// Translate a message with arguments.
pub fn ta(key: &str, args: &[(&str, FluentValue<'static>)]) -> String {
    let mut a = FluentArgs::new();
    for (k, v) in args {
        a.set(*k, v.clone());
    }
    lookup(key, Some(&a))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn ids(src: &str) -> BTreeSet<String> {
        FluentResource::try_new(src.to_string())
            .unwrap_or_else(|(r, _)| r)
            .entries()
            .filter_map(|e| match e {
                fluent_syntax::ast::Entry::Message(m) => Some(m.id.name.to_string()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn every_locale_parses_and_has_no_unknown_keys() {
        let en = EMBEDDED
            .iter()
            .find(|(id, _)| *id == FALLBACK)
            .expect("en-US.ftl")
            .1;
        let en_ids = ids(en);
        for (id, src) in EMBEDDED {
            assert!(
                FluentResource::try_new(src.to_string()).is_ok(),
                "{id}.ftl has syntax errors"
            );
            let extra: Vec<_> = ids(src).difference(&en_ids).cloned().collect();
            assert!(
                extra.is_empty(),
                "{id}.ftl has keys not in en-US: {extra:?}"
            );
        }
    }

    #[test]
    fn fallback_and_switch() {
        init(Some("en-US"));
        assert_eq!(t("nav-overview"), "Overview");
        assert_eq!(t("no-such-key"), "no-such-key");
    }
}
