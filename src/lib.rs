pub mod app;
pub mod config;
pub mod localizer;
pub mod model;
pub mod render;
pub mod scan;
slint::include_modules!();
pub fn run() -> Result<(), slint::PlatformError> {
    app::run()
}
