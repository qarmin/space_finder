use i18n_embed::{
    DefaultLocalizer, DesktopLanguageRequester, LanguageLoader, Localizer,
    fluent::{FluentLanguageLoader, fluent_language_loader},
};
use log::error;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "i18n/"]
struct Localizations;

pub static LANGUAGE_LOADER: std::sync::LazyLock<FluentLanguageLoader> = std::sync::LazyLock::new(|| {
    let loader: FluentLanguageLoader = fluent_language_loader!();
    loader
        .load_fallback_language(&Localizations)
        .expect("Failed to load fallback language");
    loader
});

/// Detect the OS locale and load matching translations.
pub fn setup_language() {
    let localizer = Box::from(DefaultLocalizer::new(&*LANGUAGE_LOADER, &Localizations));
    let requested = DesktopLanguageRequester::requested_languages();
    if let Err(e) = localizer.select(&requested) {
        error!("Failed to load language: {e}");
    }
}

#[macro_export]
macro_rules! t {
    ( $($tt:tt)* ) => {{
        i18n_embed_fl::fl!($crate::localizer::LANGUAGE_LOADER, $($tt)*)
    }};
}
