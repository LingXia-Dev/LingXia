//! `Intl` in View isolates: rong's ECMA-402 over the locale data every QuickJS host embeds, so a
//! page has `Intl` as it would on JavaScriptCore, with its defaults on the page's language.
use rong_quickjs::intl::{HourCycle, IntlData, IntlSettings};

/// What a page's `Intl` defaults follow: the page language and the clock setting.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Settings {
    pub language: Option<String>,
    pub hour_cycle: Option<String>,
}

thread_local! {
    /// Parsed once per isolate thread; every page context shares it.
    static DATA: Result<IntlData, String> =
        IntlData::try_from_static_blob(lingxia_i18n::intl::INTL_DATA)
            .map_err(|e| format!("the embedded locale data does not load: {e}"));
}

/// Gives a fresh page context `Intl` before its module runs.
pub(crate) fn install(context: &super::JSContext) -> Result<(), String> {
    let data = DATA.with(Clone::clone)?;
    rong_quickjs::intl::configure(context, data, IntlSettings::default());
    rong_quickjs::intl::init(context).map_err(|e| e.to_string())
}

/// Points the context's defaults at the page's language and clock.
pub(crate) fn update(context: &super::JSContext, settings: &Settings) {
    let hour_cycle = match settings.hour_cycle.as_deref() {
        Some("h12") => Some(HourCycle::H12),
        Some("h23") => Some(HourCycle::H23),
        _ => None,
    };
    rong_quickjs::intl::set_settings(
        context,
        IntlSettings {
            locale: settings.language.clone(),
            time_zone: None,
            hour_cycle,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use rong::{JSEngine, Source};

    #[test]
    fn a_page_has_intl_that_follows_its_language_and_clock() {
        let runtime = super::super::RongJS::runtime();
        let context = runtime.context();
        install(&context).unwrap();
        let read = |script: &str| -> String { context.eval(Source::from_bytes(script)).unwrap() };
        let locale = "new Intl.NumberFormat().resolvedOptions().locale";
        update(
            &context,
            &Settings {
                language: Some("he".into()),
                hour_cycle: None,
            },
        );
        assert_eq!(read(locale), "he");
        update(
            &context,
            &Settings {
                language: Some("zh-Hans".into()),
                hour_cycle: Some("h23".into()),
            },
        );
        assert_eq!(read(locale), "zh-Hans");
        assert_eq!(
            read("new Intl.DateTimeFormat(undefined,{hour:'numeric'}).resolvedOptions().hourCycle"),
            "h23"
        );
        assert_eq!(read("(1234.5).toLocaleString('de')"), "1.234,5");
        assert_eq!(read("new Intl.PluralRules('ar').select(2)"), "two");
    }
}
