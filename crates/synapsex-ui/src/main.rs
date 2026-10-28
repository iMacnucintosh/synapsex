mod app;
mod color;
mod ctx;
mod header;
mod hid_async;
mod pages;
mod remap_picker;
mod theme;

use adw::prelude::*;
use anyhow::Result;

fn main() -> Result<()> {
    synapsex_i18n::init();
    // Prefer Spanish GTK translations when available.
    if matches!(synapsex_i18n::lang(), synapsex_i18n::Lang::Es) {
        std::env::set_var("LANGUAGE", "es");
    }
    let app = adw::Application::builder()
        .application_id("com.synapsex.Synapsex")
        .build();
    app.connect_activate(app::build_ui);
    app.run();
    Ok(())
}
