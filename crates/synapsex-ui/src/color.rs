//! Colour helpers for the lighting page: swatches, presets and the editor dialog.

use adw::prelude::*;
use gtk::gdk;
#[allow(deprecated)]
use gtk::prelude::ColorChooserExt;
use razer_protocol::Rgb;
use synapsex_i18n as i18n;

pub const PRESETS: &[(u8, u8, u8)] = &[
    (0x00, 0xff, 0x66),
    (0xff, 0x00, 0x44),
    (0xff, 0x6a, 0x00),
    (0xff, 0xd4, 0x00),
    (0x00, 0xc2, 0xff),
    (0x3d, 0x5a, 0xfe),
    (0xb3, 0x88, 0xff),
    (0xff, 0xff, 0xff),
    (0x22, 0x22, 0x22),
];

pub fn install_css() {
    static DONE: std::sync::Once = std::sync::Once::new();
    DONE.call_once(|| {
        let mut css = String::from(
            r#"
            .synapsex-swatch {
                border-radius: 9999px;
                border: 1px solid alpha(currentColor, 0.25);
                min-width: 32px;
                min-height: 32px;
                padding: 0;
            }
            .synapsex-chip {
                border-radius: 9999px;
                border: 1px solid alpha(currentColor, 0.2);
                min-width: 22px;
                min-height: 22px;
                padding: 0;
            }
            .synapsex-chip.synapsex-dot {
                min-width: 22px;
                min-height: 22px;
            }
            /* Tabs sit in the page, not fused to the header. */
            .synapsex-switcher {
                margin-top: 16px;
                margin-bottom: 8px;
                margin-start: 12px;
                margin-end: 12px;
            }
            /* Give the colour plane room so GTK doesn't wrap it in a cramped scroll. */
            .synapsex-color-editor colorchooser { min-width: 300px; min-height: 340px; }
            .synapsex-color-editor colorchooser scrolledwindow { min-height: 280px; }
            .synapsex-color-editor colorchooser scrollbar { opacity: 0; min-width: 0; min-height: 0; }
            "#,
        );
        for &(r, g, b) in PRESETS {
            css.push_str(&format!(
                ".synapsex-chip-{r:02x}{g:02x}{b:02x} {{ background: #{r:02x}{g:02x}{b:02x}; }}\n"
            ));
        }
        let provider = gtk::CssProvider::new();
        provider.load_from_string(&css);
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
}

/// Non-interactive colour circle (status rows, etc.).
pub struct Dot {
    pub widget: gtk::Box,
    provider: gtk::CssProvider,
}

impl Dot {
    pub fn new() -> Self {
        let provider = gtk::CssProvider::new();
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
            );
        }
        let widget = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        widget.add_css_class("synapsex-chip");
        widget.add_css_class("synapsex-dot");
        widget.set_valign(gtk::Align::Center);
        widget.set_size_request(22, 22);
        Self { widget, provider }
    }

    pub fn set_rgb(&self, c: Rgb) {
        let hex = format!("{:02x}{:02x}{:02x}", c.r, c.g, c.b);
        self.widget.set_widget_name(&format!("c{hex}"));
        self.provider.load_from_string(&format!(
            "#c{hex}.synapsex-chip {{ background: #{hex}; min-width: 22px; min-height: 22px; }}"
        ));
    }
}

/// Round button showing one colour; its CSS is owned by `provider`.
pub struct Swatch {
    pub button: gtk::Button,
    provider: gtk::CssProvider,
}

impl Swatch {
    pub fn new() -> Self {
        let provider = gtk::CssProvider::new();
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
            );
        }
        let button = gtk::Button::builder()
            .tooltip_text(i18n::color_dialog_title())
            .valign(gtk::Align::Center)
            .css_classes(["synapsex-swatch"])
            .build();
        Self { button, provider }
    }

    pub fn set(&self, c: &gdk::RGBA) {
        self.provider.load_from_string(&format!(
            ".synapsex-swatch {{ background: {}; }}",
            format_hex(c)
        ));
    }
}

pub fn preset_chip(r: u8, g: u8, b: u8) -> gtk::Button {
    gtk::Button::builder()
        .valign(gtk::Align::Center)
        .tooltip_text(format!("#{r:02x}{g:02x}{b:02x}"))
        .css_classes(["synapsex-chip", &format!("synapsex-chip-{r:02x}{g:02x}{b:02x}")])
        .build()
}

pub fn format_hex(c: &gdk::RGBA) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        (c.red() * 255.0).round() as u8,
        (c.green() * 255.0).round() as u8,
        (c.blue() * 255.0).round() as u8
    )
}

pub fn rgba(r: u8, g: u8, b: u8) -> gdk::RGBA {
    gdk::RGBA::new(
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
        1.0,
    )
}

pub fn rgb_from_rgba(c: &gdk::RGBA) -> Rgb {
    Rgb::new(
        (c.red() * 255.0).round().clamp(0.0, 255.0) as u8,
        (c.green() * 255.0).round().clamp(0.0, 255.0) as u8,
        (c.blue() * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

pub fn rgba_from_rgb(c: Rgb) -> gdk::RGBA {
    rgba(c.r, c.g, c.b)
}

#[allow(deprecated)] // ColorDialog scrolls badly in the editor view; the widget does not.
pub fn open_editor(
    parent: &impl IsA<gtk::Widget>,
    initial: gdk::RGBA,
    on_select: impl Fn(gdk::RGBA) + 'static,
) {
    let dialog = adw::Dialog::builder()
        .title(i18n::color_dialog_title())
        .content_width(360)
        .content_height(460)
        .build();

    let chooser = gtk::ColorChooserWidget::builder()
        .show_editor(true)
        .hexpand(true)
        .vexpand(true)
        .css_classes(["synapsex-color-editor"])
        .build();
    chooser.set_use_alpha(false);
    chooser.set_rgba(&initial);

    let cancel = gtk::Button::with_label(i18n::cancel());
    let select = gtk::Button::with_label(i18n::select());
    select.add_css_class("suggested-action");

    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);
    header.set_show_start_title_buttons(false);
    header.pack_start(&cancel);
    header.pack_end(&select);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&chooser));
    dialog.set_child(Some(&toolbar));

    cancel.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            dialog.close();
        }
    });
    select.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            on_select(chooser.rgba());
            dialog.close();
        }
    });

    dialog.present(Some(parent));
}
