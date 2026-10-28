//! One module per view-stack page; each owns its widgets and renders from `MouseState`.

pub mod buttons;
pub mod lighting;
#[allow(dead_code)]
pub mod more;
pub mod performance;

use adw::prelude::*;

/// Standard Adwaita accent button, meant as a `PreferencesGroup` header suffix.
pub fn apply_button(title: &str) -> gtk::Button {
    gtk::Button::builder()
        .label(title)
        .valign(gtk::Align::Center)
        .css_classes(["suggested-action", "pill"])
        .build()
}

pub fn string_combo(title: &str, items: &[&str]) -> adw::ComboRow {
    let model = gtk::StringList::new(items);
    adw::ComboRow::builder()
        .title(title)
        .model(&model)
        .expression(gtk::PropertyExpression::new(
            gtk::StringObject::static_type(),
            gtk::Expression::NONE,
            "string",
        ))
        .build()
}
