//! Zone lighting: effect, colour, brightness and Smart Dimming.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::gdk;
use razer_device::MouseState;
use razer_protocol::{LightingEffect, LightingZone};
use synapsex_i18n as i18n;

use super::{apply_button, string_combo};
use crate::color::{self, Dot, Swatch};
use crate::ctx::Ctx;

const EFFECTS: [LightingEffect; 6] = [
    LightingEffect::Static,
    LightingEffect::Breathing,
    LightingEffect::Spectrum,
    LightingEffect::Reactive,
    LightingEffect::Wave,
    LightingEffect::Off,
];

const ZONES: [LightingZone; 3] = [
    LightingZone::Logo,
    LightingZone::ScrollWheel,
    LightingZone::Underglow,
];

pub struct LightingPage {
    pub widget: adw::PreferencesPage,
    color: RefCell<gdk::RGBA>,
    swatch: Swatch,
    hex: gtk::Label,
    brightness: gtk::Scale,
    smart_dimming: adw::SwitchRow,
    /// Suppresses the write handler while `apply` syncs the switch.
    syncing: Cell<bool>,
    status_group: adw::PreferencesGroup,
    status_rows: RefCell<Vec<ZoneRow>>,
}

struct ZoneRow {
    row: adw::ActionRow,
    _dot: Dot,
    _pct: gtk::Label,
}

impl LightingPage {
    pub fn new(ctx: &Rc<Ctx>) -> Rc<Self> {
        color::install_css();
        let widget = adw::PreferencesPage::new();

        let light_apply = apply_button(i18n::apply());
        let group = adw::PreferencesGroup::builder()
            .title(i18n::lighting_group_title())
            .description(i18n::lighting_group_desc())
            .header_suffix(&light_apply)
            .build();

        let zone = string_combo(
            i18n::zone_title(),
            &[
                i18n::zone_all(),
                i18n::zone_logo(),
                i18n::zone_wheel(),
                i18n::zone_underglow(),
            ],
        );
        let effect = string_combo(
            i18n::effect_title(),
            &[
                i18n::effect_static(),
                i18n::effect_breathing(),
                i18n::effect_spectrum(),
                i18n::effect_reactive(),
                i18n::effect_wave(),
                i18n::effect_off(),
            ],
        );

        let swatch = Swatch::new();
        let hex = gtk::Label::builder()
            .selectable(true)
            .css_classes(["monospace", "dim-label"])
            .build();
        let color_row = adw::ActionRow::builder()
            .title(i18n::color_label())
            .activatable(true)
            .build();
        color_row.add_suffix(&hex);
        color_row.add_suffix(&swatch.button);
        color_row.set_activatable_widget(Some(&swatch.button));

        let presets_row = adw::ActionRow::builder()
            .title(i18n::color_presets_label())
            .build();
        let presets = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        presets_row.add_suffix(&presets);

        let brightness_row = adw::ActionRow::builder()
            .title(i18n::brightness_label())
            .build();
        let brightness = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
        brightness.set_value(80.0);
        brightness.set_draw_value(true);
        brightness.set_value_pos(gtk::PositionType::Left);
        brightness.set_hexpand(true);
        brightness.set_size_request(200, -1);
        brightness_row.add_suffix(&brightness);

        group.add(&zone);
        group.add(&effect);
        group.add(&color_row);
        group.add(&presets_row);
        group.add(&brightness_row);
        widget.add(&group);

        let dimming_group = adw::PreferencesGroup::new();
        let smart_dimming = adw::SwitchRow::builder()
            .title(i18n::smart_dimming())
            .subtitle(i18n::smart_dimming_hint())
            .build();
        dimming_group.add(&smart_dimming);
        widget.add(&dimming_group);

        let status_group = adw::PreferencesGroup::builder()
            .title(i18n::lighting_current_title())
            .build();
        widget.add(&status_group);

        let page = Rc::new(Self {
            widget,
            color: RefCell::new(color::rgba(0x00, 0xff, 0x66)),
            swatch,
            hex,
            brightness,
            smart_dimming,
            syncing: Cell::new(false),
            status_group,
            status_rows: RefCell::new(Vec::new()),
        });
        page.set_color(color::rgba(0x00, 0xff, 0x66));

        for &(r, g, b) in color::PRESETS {
            let chip = color::preset_chip(r, g, b);
            let page_ref = Rc::downgrade(&page);
            chip.connect_clicked(move |_| {
                if let Some(page) = page_ref.upgrade() {
                    page.set_color(color::rgba(r, g, b));
                }
            });
            presets.append(&chip);
        }

        page.swatch.button.connect_clicked({
            let ctx = Rc::clone(ctx);
            let page_ref = Rc::downgrade(&page);
            move |_| {
                let Some(page) = page_ref.upgrade() else {
                    return;
                };
                let initial = *page.color.borrow();
                let page_ref = Rc::downgrade(&page);
                color::open_editor(&ctx.window, initial, move |rgba| {
                    if let Some(page) = page_ref.upgrade() {
                        page.set_color(rgba);
                    }
                });
            }
        });

        light_apply.connect_clicked({
            let ctx = Rc::clone(ctx);
            let page = Rc::clone(&page);
            move |_| {
                let effect = EFFECTS
                    .get(effect.selected() as usize)
                    .copied()
                    .unwrap_or(LightingEffect::Static);
                let rgb = color::rgb_from_rgba(&page.color.borrow());
                let pct = page.brightness.value() as u8;
                let zone_idx = zone.selected() as usize;
                ctx.write_and_refresh(i18n::toast_lighting_saved(), move |s| {
                    if zone_idx == 0 {
                        s.set_all_zones(effect, rgb, pct)?;
                    } else {
                        s.set_zone_lighting(ZONES[(zone_idx - 1).min(2)], effect, rgb, pct)?;
                    }
                    Ok(())
                });
            }
        });

        page.smart_dimming.connect_active_notify({
            let ctx = Rc::clone(ctx);
            let page_ref = Rc::downgrade(&page);
            move |row| {
                if page_ref.upgrade().is_none_or(|p| p.syncing.get()) {
                    return;
                }
                let on = row.is_active();
                ctx.hid(
                    move |s| Ok(s.set_smart_dimming(on)?),
                    move |ctx, ()| {
                        ctx.toast(if on {
                            i18n::toast_smart_dimming_on()
                        } else {
                            i18n::toast_smart_dimming_off()
                        });
                    },
                );
            }
        });

        page
    }

    fn set_color(&self, c: gdk::RGBA) {
        *self.color.borrow_mut() = c;
        self.swatch.set(&c);
        self.hex.set_label(&color::format_hex(&c));
    }

    pub fn apply(&self, st: &MouseState) {
        {
            let mut rows = self.status_rows.borrow_mut();
            for row in rows.drain(..) {
                self.status_group.remove(&row.row);
            }
        }
        let mut rows = self.status_rows.borrow_mut();
        for z in &st.zones {
            let dot = Dot::new();
            dot.set_rgb(z.color);
            let pct = gtk::Label::builder()
                .label(i18n::brightness_pct(z.brightness))
                .css_classes(["dim-label"])
                .valign(gtk::Align::Center)
                .build();
            let row = adw::ActionRow::builder()
                .title(i18n::zone_label(z.zone.name()))
                .subtitle(i18n::lighting_effect_label(&format!("{:?}", z.effect)))
                .activatable(false)
                .selectable(false)
                .build();
            row.add_prefix(&dot.widget);
            row.add_suffix(&pct);
            self.status_group.add(&row);
            rows.push(ZoneRow {
                row,
                _dot: dot,
                _pct: pct,
            });
        }
        drop(rows);
        if let Some(z) = st.zones.first() {
            self.set_color(color::rgba_from_rgb(z.color));
            if z.brightness > 0 {
                self.brightness.set_value(f64::from(z.brightness));
            }
        }
        self.syncing.set(true);
        self.smart_dimming.set_active(st.smart_dimming);
        self.syncing.set(false);
    }
}
