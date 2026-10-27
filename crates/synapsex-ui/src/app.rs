use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use adw::prelude::*;
use gtk::gdk;
use gtk::glib;
use razer_device::{AppConfig, DeviceSession, MouseState};
use razer_protocol::{onboard, LightingEffect, LightingZone, Rgb};
use synapsex_i18n as i18n;

struct Shared {
    session: Mutex<Option<DeviceSession>>,
}

pub fn build_ui(app: &adw::Application) {
    let shared = Arc::new(Shared {
        session: Mutex::new(None),
    });

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title(i18n::app_title())
        .default_width(920)
        .default_height(740)
        .build();

    let toast_overlay = adw::ToastOverlay::new();
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let header = adw::HeaderBar::new();
    let refresh_btn = gtk::Button::from_icon_name("view-refresh-symbolic");
    header.pack_end(&refresh_btn);

    let bg_switch = gtk::CheckButton::with_label(i18n::keep_background());
    bg_switch.set_active(AppConfig::load().keep_running_in_background);
    header.pack_end(&bg_switch);
    root.append(&header);

    // --- Perfiles onboard: lo primero de la app ---
    let profile_bar = gtk::Box::new(gtk::Orientation::Vertical, 8);
    profile_bar.set_margin_start(20);
    profile_bar.set_margin_end(20);
    profile_bar.set_margin_top(16);
    profile_bar.set_margin_bottom(4);
    let profile_title = gtk::Label::builder()
        .label(i18n::profiles_on_device())
        .halign(gtk::Align::Start)
        .css_classes(["title-3"])
        .build();
    profile_bar.append(&profile_title);

    let profile_string_list = gtk::StringList::new(&[]);
    let profile_dropdown = gtk::DropDown::builder()
        .model(&profile_string_list)
        .hexpand(true)
        .build();
    profile_dropdown.set_sensitive(false);

    let create_btn = gtk::Button::with_label(i18n::create_profile());
    create_btn.add_css_class("suggested-action");
    let delete_btn = gtk::Button::with_label(i18n::delete_profile());
    delete_btn.add_css_class("destructive-action");
    let rename_entry = gtk::Entry::builder()
        .placeholder_text(i18n::rename_placeholder())
        .hexpand(true)
        .build();
    let rename_btn = gtk::Button::with_label(i18n::rename());

    let profile_row = row();
    profile_row.append(&profile_dropdown);
    profile_row.append(&create_btn);
    profile_row.append(&delete_btn);
    profile_bar.append(&profile_row);

    let rename_row = row();
    rename_row.append(&rename_entry);
    rename_row.append(&rename_btn);
    profile_bar.append(&rename_row);

    let profile_hint = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .css_classes(["dim-label"])
        .build();
    profile_bar.append(&profile_hint);
    root.append(&profile_bar);

    let status = gtk::Label::builder()
        .label(i18n::looking_for_mouse())
        .margin_top(8)
        .margin_bottom(4)
        .margin_start(20)
        .margin_end(20)
        .halign(gtk::Align::Start)
        .css_classes(["heading"])
        .build();
    root.append(&status);

    let battery = gtk::Label::builder()
        .css_classes(["dim-label"])
        .margin_bottom(8)
        .margin_start(20)
        .margin_end(20)
        .halign(gtk::Align::Start)
        .build();
    root.append(&battery);

    let scrolled = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hexpand(true)
        .build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
    content.set_margin_start(20);
    content.set_margin_end(20);
    content.set_margin_bottom(28);

    let updating_profiles = Rc::new(RefCell::new(false));
    let profile_slots: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));

    // DPI
    let dpi_card = section(i18n::section_dpi());
    let dpi_label = gtk::Label::new(Some("—"));
    let dpi_spin = gtk::SpinButton::with_range(100.0, 50_000.0, 50.0);
    let dpi_apply = gtk::Button::with_label(i18n::apply_dpi());
    let dpi_row = row();
    dpi_row.append(&dpi_label);
    dpi_row.append(&dpi_spin);
    dpi_row.append(&dpi_apply);
    dpi_card.append(&dpi_row);
    let stages_entry = gtk::Entry::builder()
        .placeholder_text(i18n::stages_placeholder())
        .hexpand(true)
        .build();
    let stages_apply = gtk::Button::with_label(i18n::apply_stages());
    let stages_row = row();
    stages_row.append(&stages_entry);
    stages_row.append(&stages_apply);
    dpi_card.append(&stages_row);
    content.append(&dpi_card);

    // Poll
    let poll_card = section(i18n::section_poll());
    let poll_label = gtk::Label::new(Some("—"));
    let poll_dropdown =
        gtk::DropDown::from_strings(&["125", "250", "500", "1000", "2000", "4000", "8000"]);
    let poll_apply = gtk::Button::with_label(i18n::apply());
    let poll_row = row();
    poll_row.append(&poll_label);
    poll_row.append(&poll_dropdown);
    poll_row.append(&poll_apply);
    poll_card.append(&poll_row);
    content.append(&poll_card);

    // Lighting
    let light_card = section(i18n::section_lighting());
    let zone_dd = gtk::DropDown::from_strings(&[
        i18n::zone_all(),
        i18n::zone_logo(),
        i18n::zone_wheel(),
        i18n::zone_underglow(),
    ]);
    let effect_dd = gtk::DropDown::from_strings(&[
        i18n::effect_static(),
        i18n::effect_breathing(),
        i18n::effect_spectrum(),
        i18n::effect_reactive(),
        i18n::effect_wave(),
        i18n::effect_off(),
    ]);

    // Color picker: dialog button + hex readout + quick presets
    let color_dialog = gtk::ColorDialog::builder()
        .title(i18n::color_dialog_title())
        .modal(true)
        .with_alpha(false)
        .build();
    let initial = gdk::RGBA::new(0.0, 1.0, 0.4, 1.0);
    let color_btn = gtk::ColorDialogButton::builder()
        .dialog(&color_dialog)
        .rgba(&initial)
        .hexpand(false)
        .halign(gtk::Align::Start)
        .tooltip_text(i18n::color_dialog_title())
        .build();
    color_btn.set_size_request(56, 36);
    let hex_label = gtk::Label::builder()
        .label("#00ff66")
        .selectable(true)
        .css_classes(["monospace", "dim-label"])
        .halign(gtk::Align::Start)
        .build();
    let color_row = row();
    color_row.append(&gtk::Label::new(Some(i18n::color_label())));
    color_row.append(&color_btn);
    color_row.append(&hex_label);

    let presets_label = gtk::Label::builder()
        .label(i18n::color_presets_label())
        .halign(gtk::Align::Start)
        .css_classes(["dim-label"])
        .build();
    let presets_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    presets_box.add_css_class("color-presets");
    let preset_colors: &[(u8, u8, u8)] = &[
        (0x00, 0xff, 0x66), // Razer green
        (0xff, 0x00, 0x44), // red
        (0xff, 0x6a, 0x00), // orange
        (0xff, 0xd4, 0x00), // yellow
        (0x00, 0xc2, 0xff), // cyan
        (0x3d, 0x5a, 0xfe), // blue
        (0xb3, 0x88, 0xff), // violet
        (0xff, 0xff, 0xff), // white
        (0x22, 0x22, 0x22), // dim
    ];
    for &(r, g, b) in preset_colors {
        let chip = gtk::Button::new();
        chip.add_css_class("color-chip");
        chip.set_size_request(32, 32);
        chip.set_tooltip_text(Some(&format!("#{r:02x}{g:02x}{b:02x}")));
        let css = format!(
            ".color-chip-{r:02x}{g:02x}{b:02x} {{ background: #{r:02x}{g:02x}{b:02x}; border-radius: 9999px; border: 1px solid alpha(@borders, 0.6); min-width: 28px; min-height: 28px; padding: 0; }}"
        );
        let class = format!("color-chip-{r:02x}{g:02x}{b:02x}");
        chip.add_css_class(&class);
        let provider = gtk::CssProvider::new();
        provider.load_from_string(&css);
        gtk::style_context_add_provider_for_display(
            &gtk::gdk::Display::default().expect("display"),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        let color_btn_p = color_btn.clone();
        let hex_label_p = hex_label.clone();
        chip.connect_clicked(move |_| {
            let rgba = gdk::RGBA::new(
                f32::from(r) / 255.0,
                f32::from(g) / 255.0,
                f32::from(b) / 255.0,
                1.0,
            );
            color_btn_p.set_rgba(&rgba);
            hex_label_p.set_label(&format!("#{r:02x}{g:02x}{b:02x}"));
        });
        presets_box.append(&chip);
    }

    color_btn.connect_rgba_notify({
        let hex_label = hex_label.clone();
        move |btn| {
            let c = btn.rgba();
            let r = (c.red() * 255.0).round() as u8;
            let g = (c.green() * 255.0).round() as u8;
            let b = (c.blue() * 255.0).round() as u8;
            hex_label.set_label(&format!("#{r:02x}{g:02x}{b:02x}"));
        }
    });

    let bright_scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
    bright_scale.set_value(80.0);
    bright_scale.set_hexpand(true);
    bright_scale.set_draw_value(true);
    let light_apply = gtk::Button::with_label(i18n::apply_lighting());
    let light_info = gtk::Label::builder().wrap(true).halign(gtk::Align::Start).build();
    light_card.append(&zone_dd);
    light_card.append(&effect_dd);
    light_card.append(&color_row);
    light_card.append(&presets_label);
    light_card.append(&presets_box);
    light_card.append(&bright_scale);
    light_card.append(&light_apply);
    light_card.append(&light_info);
    content.append(&light_card);

    // Wheel
    let wheel_card = section(i18n::section_wheel());
    let smart_reel = gtk::CheckButton::with_label(i18n::smart_reel());
    let scroll_accel = gtk::CheckButton::with_label(i18n::scroll_accel());
    let wheel_apply = gtk::Button::with_label(i18n::apply_wheel());
    wheel_card.append(&smart_reel);
    wheel_card.append(&scroll_accel);
    wheel_card.append(&wheel_apply);
    content.append(&wheel_card);

    // Power
    let power_card = section(i18n::section_power());
    let idle_spin = gtk::SpinButton::with_range(60.0, 900.0, 30.0);
    let low_spin = gtk::SpinButton::with_range(5.0, 50.0, 1.0);
    let power_apply = gtk::Button::with_label(i18n::apply_power());
    let power_row = row();
    power_row.append(&gtk::Label::new(Some(i18n::sleep_seconds())));
    power_row.append(&idle_spin);
    power_row.append(&gtk::Label::new(Some(i18n::low_battery_pct())));
    power_row.append(&low_spin);
    power_row.append(&power_apply);
    power_card.append(&power_row);
    content.append(&power_card);

    // Buttons
    let buttons_card = section(i18n::section_buttons());
    let buttons_list = gtk::ListBox::new();
    buttons_list.add_css_class("boxed-list");
    buttons_card.append(&buttons_list);
    let preset_labels: Vec<String> = onboard::remap_presets()
        .iter()
        .map(|(name, _)| i18n::remap_label(name))
        .collect();
    content.append(&buttons_card);

    scrolled.set_child(Some(&content));
    root.append(&scrolled);
    toast_overlay.set_child(Some(&root));
    window.set_content(Some(&toast_overlay));

    let state_cell: Rc<RefCell<Option<MouseState>>> = Rc::new(RefCell::new(None));
    let refresh_holder: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));

    let refresh = Rc::new({
        let shared = Arc::clone(&shared);
        let status = status.clone();
        let battery = battery.clone();
        let dpi_label = dpi_label.clone();
        let dpi_spin = dpi_spin.clone();
        let stages_entry = stages_entry.clone();
        let poll_label = poll_label.clone();
        let light_info = light_info.clone();
        let color_btn = color_btn.clone();
        let hex_label = hex_label.clone();
        let bright_scale = bright_scale.clone();
        let smart_reel = smart_reel.clone();
        let scroll_accel = scroll_accel.clone();
        let idle_spin = idle_spin.clone();
        let low_spin = low_spin.clone();
        let buttons_list = buttons_list.clone();
        let profile_string_list = profile_string_list.clone();
        let profile_dropdown = profile_dropdown.clone();
        let profile_hint = profile_hint.clone();
        let rename_entry = rename_entry.clone();
        let delete_btn = delete_btn.clone();
        let create_btn = create_btn.clone();
        let updating_profiles = Rc::clone(&updating_profiles);
        let profile_slots = Rc::clone(&profile_slots);
        let state_cell = Rc::clone(&state_cell);
        let toast_overlay = toast_overlay.clone();
        let preset_labels = preset_labels.clone();
        move || {
            let result = (|| -> Result<MouseState, String> {
                let mut guard = shared.session.lock().map_err(|e| e.to_string())?;
                if guard.is_none() {
                    *guard = Some(DeviceSession::open().map_err(|e| e.to_string())?);
                }
                guard
                    .as_mut()
                    .unwrap()
                    .read_state()
                    .map_err(|e| e.to_string())
            })();
            match result {
                Ok(st) => {
                    status.set_label(&format!(
                        "{} · {} · {}",
                        st.name,
                        i18n::connection_label(&st.connection),
                        st.firmware
                    ));
                    battery.set_label(&i18n::battery_line(
                        st.battery_percent,
                        st.charging,
                        &st.serial,
                    ));
                    dpi_label.set_label(&i18n::current_dpi(st.dpi_x, st.dpi_y));
                    dpi_spin.set_value(st.dpi_x as f64);
                    stages_entry.set_text(
                        &st.dpi_stages
                            .iter()
                            .map(|s| s.to_string())
                            .collect::<Vec<_>>()
                            .join(","),
                    );
                    poll_label.set_label(&format!("{} Hz", st.poll_hz));
                    light_info.set_label(
                        &st.zones
                            .iter()
                            .map(|z| {
                                format!(
                                    "{}: {} #{:02x}{:02x}{:02x} {}%",
                                    i18n::zone_label(z.zone.name()),
                                    i18n::lighting_effect_label(&format!("{:?}", z.effect)),
                                    z.color.r,
                                    z.color.g,
                                    z.color.b,
                                    z.brightness
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(" · "),
                    );
                    if let Some(z) = st.zones.first() {
                        color_btn.set_rgba(&rgba_from_rgb(z.color));
                        hex_label.set_label(&format!(
                            "#{:02x}{:02x}{:02x}",
                            z.color.r, z.color.g, z.color.b
                        ));
                        if z.brightness > 0 {
                            bright_scale.set_value(z.brightness as f64);
                        }
                    }
                    smart_reel.set_active(st.smart_reel);
                    scroll_accel.set_active(st.scroll_accel);
                    idle_spin.set_value(st.idle_seconds as f64);
                    low_spin.set_value(st.low_battery_percent as f64);

                    // Actualizar desplegable de perfiles (memoria del ratón)
                    *updating_profiles.borrow_mut() = true;
                    while profile_string_list.n_items() > 0 {
                        profile_string_list.remove(0);
                    }
                    *profile_slots.borrow_mut() = Vec::new();
                    let mut selected = 0u32;
                    for (idx, p) in st.profiles.iter().enumerate() {
                        profile_string_list.append(&i18n::profile_dropdown_item(
                            p.slot,
                            &p.name,
                            p.is_active,
                        ));
                        profile_slots.borrow_mut().push(p.slot);
                        if p.slot == st.active_slot {
                            selected = idx as u32;
                        }
                    }
                    profile_dropdown.set_selected(selected);
                    profile_dropdown.set_sensitive(!st.profiles.is_empty());
                    create_btn.set_sensitive(st.profiles.len() < 5);
                    delete_btn.set_sensitive(st.active_slot != 1 && st.profiles.len() > 1);
                    rename_entry.set_text(&st.slot_name);
                    profile_hint.set_label(&i18n::active_profiles(
                        st.active_slot,
                        &st.slot_name,
                        st.profiles.len(),
                    ));
                    *updating_profiles.borrow_mut() = false;

                    while let Some(child) = buttons_list.first_child() {
                        buttons_list.remove(&child);
                    }
                    for b in &st.buttons {
                        let label = i18n::button_label(&b.label);
                        let row = adw::ActionRow::builder()
                            .title(i18n::button_control_name(b.name))
                            .subtitle(&label)
                            .build();
                        // Build dropdown list; if current bind isn't in presets, prepend it.
                        let mut labels = preset_labels.clone();
                        let mut blocks: Vec<[u8; 7]> =
                            onboard::remap_presets().iter().map(|(_, b)| *b).collect();
                        let selected = if let Some(idx) = blocks
                            .iter()
                            .position(|blk| onboard::blocks_equivalent(blk, &b.block))
                        {
                            idx as u32
                        } else {
                            labels.insert(0, format!("★ {label}"));
                            blocks.insert(0, b.block);
                            0
                        };
                        let label_refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
                        let dd = gtk::DropDown::from_strings(&label_refs);
                        dd.set_selected(selected);
                        let table_id = b.table_id;
                        let shared2 = Arc::clone(&shared);
                        let toast2 = toast_overlay.clone();
                        let updating = Rc::new(RefCell::new(true));
                        {
                            let updating = Rc::clone(&updating);
                            glib::idle_add_local_once(move || {
                                *updating.borrow_mut() = false;
                            });
                        }
                        dd.connect_selected_notify(move |dd| {
                            if *updating.borrow() {
                                return;
                            }
                            let idx = dd.selected() as usize;
                            if let Some(block) = blocks.get(idx) {
                                let res = shared2.session.lock().ok().and_then(|g| {
                                    g.as_ref().map(|s| s.set_button(table_id, *block))
                                });
                                match res {
                                    Some(Ok(())) => {
                                        toast2.add_toast(adw::Toast::new(i18n::toast_button_saved()))
                                    }
                                    Some(Err(e)) => {
                                        toast2.add_toast(adw::Toast::new(&i18n::toast_error(
                                            &e.to_string(),
                                        )))
                                    }
                                    None => {}
                                }
                            }
                        });
                        row.add_suffix(&dd);
                        buttons_list.append(&row);
                    }
                    *state_cell.borrow_mut() = Some(st);
                }
                Err(e) => {
                    status.set_label(&i18n::no_mouse(&e));
                }
            }
        }
    });

    {
        let r = Rc::clone(&refresh);
        *refresh_holder.borrow_mut() = Some(Rc::new(move || r()) as Rc<dyn Fn()>);
    }
    refresh();

    refresh_btn.connect_clicked({
        let refresh = Rc::clone(&refresh);
        move |_| refresh()
    });

    wire_dpi(
        &dpi_apply,
        &dpi_spin,
        &shared,
        &refresh,
        &toast_overlay,
    );
    wire_stages(
        &stages_apply,
        &stages_entry,
        &shared,
        &refresh,
        &toast_overlay,
    );
    wire_poll(
        &poll_apply,
        &poll_dropdown,
        &shared,
        &refresh,
        &toast_overlay,
    );
    wire_light(
        &light_apply,
        &effect_dd,
        &color_btn,
        &bright_scale,
        &zone_dd,
        &shared,
        &refresh,
        &toast_overlay,
    );
    wire_wheel(
        &wheel_apply,
        &smart_reel,
        &scroll_accel,
        &shared,
        &refresh,
        &toast_overlay,
    );
    wire_power(
        &power_apply,
        &idle_spin,
        &low_spin,
        &shared,
        &refresh,
        &toast_overlay,
    );
    // Seleccionar perfil del desplegable → activar slot en el ratón
    profile_dropdown.connect_selected_notify({
        let shared = Arc::clone(&shared);
        let refresh = Rc::clone(&refresh);
        let toast = toast_overlay.clone();
        let updating_profiles = Rc::clone(&updating_profiles);
        let profile_slots = Rc::clone(&profile_slots);
        move |dd| {
            if *updating_profiles.borrow() {
                return;
            }
            let idx = dd.selected() as usize;
            let Some(&slot) = profile_slots.borrow().get(idx) else {
                return;
            };
            let current = shared
                .session
                .lock()
                .ok()
                .and_then(|g| g.as_ref().map(|s| s.active_slot));
            if current == Some(slot) {
                return;
            }
            match shared
                .session
                .lock()
                .ok()
                .and_then(|mut g| g.as_mut().map(|s| s.activate_slot(slot)))
            {
                Some(Ok(())) => {
                    toast.add_toast(adw::Toast::new(&i18n::toast_activated_reload(slot)));
                    refresh();
                }
                Some(Err(e)) => toast.add_toast(adw::Toast::new(&i18n::toast_error(&e.to_string()))),
                None => {}
            }
        }
    });

    wire_create(
        &create_btn,
        &shared,
        &refresh,
        &toast_overlay,
        &state_cell,
    );
    wire_delete(
        &delete_btn,
        &shared,
        &refresh,
        &toast_overlay,
        &state_cell,
    );
    wire_rename(
        &rename_btn,
        &rename_entry,
        &shared,
        &refresh,
        &toast_overlay,
    );

    bg_switch.connect_toggled(move |sw| {
        let mut cfg = AppConfig::load();
        cfg.keep_running_in_background = sw.is_active();
        let _ = cfg.save();
    });

    // followSlotSync: poll active slot every 500ms
    {
        let shared = Arc::clone(&shared);
        let refresh = Rc::clone(&refresh);
        let toast = toast_overlay.clone();
        let last_slot = Rc::new(RefCell::new(0u8));
        glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
            let changed = shared
                .session
                .lock()
                .ok()
                .and_then(|mut g| {
                    let s = g.as_mut()?;
                    let prev = *last_slot.borrow();
                    match s.poll_slot_change() {
                        Ok(true) => {
                            *last_slot.borrow_mut() = s.active_slot;
                            Some(true)
                        }
                        Ok(false) => {
                            if prev == 0 {
                                *last_slot.borrow_mut() = s.active_slot;
                            }
                            Some(false)
                        }
                        Err(_) => Some(false),
                    }
                })
                .unwrap_or(false);
            if changed {
                toast.add_toast(adw::Toast::new(i18n::toast_profile_changed()));
                refresh();
            }
            glib::ControlFlow::Continue
        });
    }

    // Hide to tray-ish: if background enabled, closing just hides
    window.connect_close_request({
        let bg_switch = bg_switch.clone();
        let window = window.clone();
        move |_| {
            if bg_switch.is_active() {
                window.set_visible(false);
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        }
    });

    window.present();
}

fn section(title: &str) -> gtk::Box {
    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let label = gtk::Label::builder()
        .label(title)
        .halign(gtk::Align::Start)
        .css_classes(["title-4"])
        .build();
    box_.append(&label);
    box_
}

fn row() -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    b.set_halign(gtk::Align::Fill);
    b
}

fn rgb_from_rgba(c: &gdk::RGBA) -> Rgb {
    Rgb::new(
        (c.red() * 255.0).round().clamp(0.0, 255.0) as u8,
        (c.green() * 255.0).round().clamp(0.0, 255.0) as u8,
        (c.blue() * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

fn rgba_from_rgb(c: Rgb) -> gdk::RGBA {
    gdk::RGBA::new(
        f32::from(c.r) / 255.0,
        f32::from(c.g) / 255.0,
        f32::from(c.b) / 255.0,
        1.0,
    )
}

fn wire_dpi(
    btn: &gtk::Button,
    spin: &gtk::SpinButton,
    shared: &Arc<Shared>,
    refresh: &Rc<impl Fn() + 'static>,
    toast: &adw::ToastOverlay,
) {
    let shared = Arc::clone(shared);
    let spin = spin.clone();
    let refresh = Rc::clone(refresh);
    let toast = toast.clone();
    btn.connect_clicked(move |_| {
        let dpi = spin.value() as u16;
        match shared
            .session
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|s| s.set_dpi(dpi, dpi)))
        {
            Some(Ok(())) => {
                toast.add_toast(adw::Toast::new(i18n::toast_dpi_written()));
                refresh();
            }
            Some(Err(e)) => toast.add_toast(adw::Toast::new(&i18n::toast_error(&e.to_string()))),
            None => {}
        }
    });
}

fn wire_stages(
    btn: &gtk::Button,
    entry: &gtk::Entry,
    shared: &Arc<Shared>,
    refresh: &Rc<impl Fn() + 'static>,
    toast: &adw::ToastOverlay,
) {
    let shared = Arc::clone(shared);
    let entry = entry.clone();
    let refresh = Rc::clone(refresh);
    let toast = toast.clone();
    btn.connect_clicked(move |_| {
        let stages: Vec<u16> = entry
            .text()
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter_map(|s| s.parse().ok())
            .collect();
        if stages.is_empty() {
            return;
        }
        match shared
            .session
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|s| s.set_dpi_stages(&stages, 0)))
        {
            Some(Ok(())) => {
                toast.add_toast(adw::Toast::new(i18n::toast_stages_written()));
                refresh();
            }
            Some(Err(e)) => toast.add_toast(adw::Toast::new(&i18n::toast_error(&e.to_string()))),
            None => {}
        }
    });
}

fn wire_poll(
    btn: &gtk::Button,
    dd: &gtk::DropDown,
    shared: &Arc<Shared>,
    refresh: &Rc<impl Fn() + 'static>,
    toast: &adw::ToastOverlay,
) {
    let shared = Arc::clone(shared);
    let dd = dd.clone();
    let refresh = Rc::clone(refresh);
    let toast = toast.clone();
    btn.connect_clicked(move |_| {
        let rates = [125u32, 250, 500, 1000, 2000, 4000, 8000];
        let hz = rates.get(dd.selected() as usize).copied().unwrap_or(1000);
        match shared
            .session
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|s| s.set_poll(hz)))
        {
            Some(Ok(())) => {
                toast.add_toast(adw::Toast::new(i18n::toast_poll_written()));
                refresh();
            }
            Some(Err(e)) => toast.add_toast(adw::Toast::new(&i18n::toast_error(&e.to_string()))),
            None => {}
        }
    });
}

fn wire_light(
    btn: &gtk::Button,
    effect_dd: &gtk::DropDown,
    color_btn: &gtk::ColorDialogButton,
    bright: &gtk::Scale,
    zone_dd: &gtk::DropDown,
    shared: &Arc<Shared>,
    refresh: &Rc<impl Fn() + 'static>,
    toast: &adw::ToastOverlay,
) {
    let shared = Arc::clone(shared);
    let effect_dd = effect_dd.clone();
    let color_btn = color_btn.clone();
    let bright = bright.clone();
    let zone_dd = zone_dd.clone();
    let refresh = Rc::clone(refresh);
    let toast = toast.clone();
    btn.connect_clicked(move |_| {
        let effects = [
            LightingEffect::Static,
            LightingEffect::Breathing,
            LightingEffect::Spectrum,
            LightingEffect::Reactive,
            LightingEffect::Wave,
            LightingEffect::Off,
        ];
        let effect = effects
            .get(effect_dd.selected() as usize)
            .copied()
            .unwrap_or(LightingEffect::Static);
        let color = rgb_from_rgba(&color_btn.rgba());
        let pct = bright.value() as u8;
        let zone_idx = zone_dd.selected();
        let res = shared.session.lock().ok().and_then(|g| {
            g.as_ref().map(|s| {
                if zone_idx == 0 {
                    s.set_all_zones(effect, color, pct)
                } else {
                    let zones = [
                        LightingZone::Logo,
                        LightingZone::ScrollWheel,
                        LightingZone::Underglow,
                    ];
                    s.set_zone_lighting(zones[(zone_idx as usize - 1).min(2)], effect, color, pct)
                }
            })
        });
        match res {
            Some(Ok(())) => {
                toast.add_toast(adw::Toast::new(i18n::toast_lighting_saved()));
                refresh();
            }
            Some(Err(e)) => toast.add_toast(adw::Toast::new(&i18n::toast_error(&e.to_string()))),
            None => {}
        }
    });
}

fn wire_wheel(
    btn: &gtk::Button,
    smart: &gtk::CheckButton,
    accel: &gtk::CheckButton,
    shared: &Arc<Shared>,
    refresh: &Rc<impl Fn() + 'static>,
    toast: &adw::ToastOverlay,
) {
    let shared = Arc::clone(shared);
    let smart = smart.clone();
    let accel = accel.clone();
    let refresh = Rc::clone(refresh);
    let toast = toast.clone();
    btn.connect_clicked(move |_| {
        let sr = smart.is_active();
        let sa = accel.is_active();
        let res = shared.session.lock().ok().and_then(|g| {
            g.as_ref().map(|s| {
                s.set_smart_reel(sr)?;
                s.set_scroll_accel(sa)
            })
        });
        match res {
            Some(Ok(())) => {
                toast.add_toast(adw::Toast::new(i18n::toast_wheel_saved()));
                refresh();
            }
            Some(Err(e)) => toast.add_toast(adw::Toast::new(&i18n::toast_error(&e.to_string()))),
            None => {}
        }
    });
}

fn wire_power(
    btn: &gtk::Button,
    idle: &gtk::SpinButton,
    low: &gtk::SpinButton,
    shared: &Arc<Shared>,
    refresh: &Rc<impl Fn() + 'static>,
    toast: &adw::ToastOverlay,
) {
    let shared = Arc::clone(shared);
    let idle = idle.clone();
    let low = low.clone();
    let refresh = Rc::clone(refresh);
    let toast = toast.clone();
    btn.connect_clicked(move |_| {
        let i = idle.value() as u16;
        let l = low.value() as u8;
        let res = shared.session.lock().ok().and_then(|g| {
            g.as_ref().map(|s| {
                s.set_idle(i)?;
                s.set_low_battery(l)
            })
        });
        match res {
            Some(Ok(())) => {
                toast.add_toast(adw::Toast::new(i18n::toast_power_saved()));
                refresh();
            }
            Some(Err(e)) => toast.add_toast(adw::Toast::new(&i18n::toast_error(&e.to_string()))),
            None => {}
        }
    });
}

fn wire_create(
    btn: &gtk::Button,
    shared: &Arc<Shared>,
    refresh: &Rc<impl Fn() + 'static>,
    toast: &adw::ToastOverlay,
    state_cell: &Rc<RefCell<Option<MouseState>>>,
) {
    let shared = Arc::clone(shared);
    let refresh = Rc::clone(refresh);
    let toast = toast.clone();
    let state_cell = Rc::clone(state_cell);
    btn.connect_clicked(move |_| {
        let next = state_cell
            .borrow()
            .as_ref()
            .and_then(|s| (1..=5u8).find(|n| !s.slots.contains(n)));
        let Some(slot) = next else {
            toast.add_toast(adw::Toast::new(i18n::toast_slots_full()));
            return;
        };
        match shared.session.lock().ok().and_then(|mut g| {
            g.as_mut().map(|s| {
                s.create_slot(slot)?;
                s.activate_slot(slot)
            })
        }) {
            Some(Ok(())) => {
                toast.add_toast(adw::Toast::new(&i18n::toast_created(slot)));
                refresh();
            }
            Some(Err(e)) => toast.add_toast(adw::Toast::new(&i18n::toast_error(&e.to_string()))),
            None => {}
        }
    });
}

fn wire_delete(
    btn: &gtk::Button,
    shared: &Arc<Shared>,
    refresh: &Rc<impl Fn() + 'static>,
    toast: &adw::ToastOverlay,
    state_cell: &Rc<RefCell<Option<MouseState>>>,
) {
    let shared = Arc::clone(shared);
    let refresh = Rc::clone(refresh);
    let toast = toast.clone();
    let state_cell = Rc::clone(state_cell);
    btn.connect_clicked(move |_| {
        let slot = state_cell
            .borrow()
            .as_ref()
            .map(|s| s.active_slot)
            .unwrap_or(1);
        if slot == 1 {
            toast.add_toast(adw::Toast::new(i18n::toast_cannot_delete_slot1()));
            return;
        }
        match shared
            .session
            .lock()
            .ok()
            .and_then(|mut g| g.as_mut().map(|s| s.remove_slot(slot)))
        {
            Some(Ok(())) => {
                toast.add_toast(adw::Toast::new(&i18n::toast_deleted(slot)));
                refresh();
            }
            Some(Err(e)) => toast.add_toast(adw::Toast::new(&i18n::toast_error(&e.to_string()))),
            None => {}
        }
    });
}

fn wire_rename(
    btn: &gtk::Button,
    entry: &gtk::Entry,
    shared: &Arc<Shared>,
    refresh: &Rc<impl Fn() + 'static>,
    toast: &adw::ToastOverlay,
) {
    let shared = Arc::clone(shared);
    let entry = entry.clone();
    let refresh = Rc::clone(refresh);
    let toast = toast.clone();
    btn.connect_clicked(move |_| {
        let name = entry.text().to_string();
        if name.is_empty() {
            return;
        }
        match shared.session.lock().ok().and_then(|g| {
            g.as_ref().map(|s| {
                let slot = s.active_slot;
                s.rename_slot(slot, name.clone())
            })
        }) {
            Some(Ok(())) => {
                toast.add_toast(adw::Toast::new(i18n::toast_renamed()));
                refresh();
            }
            Some(Err(e)) => toast.add_toast(adw::Toast::new(&i18n::toast_error(&e.to_string()))),
            None => {}
        }
    });
}
