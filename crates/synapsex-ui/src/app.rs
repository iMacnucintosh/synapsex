use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use adw::prelude::*;
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::prelude::EditableExt;
#[allow(deprecated)]
use gtk::prelude::ColorChooserExt;
use razer_device::{AppConfig, ButtonState, DeviceSession, MouseState};
use razer_protocol::{onboard, LightingEffect, LightingZone, Rgb};
use synapsex_i18n as i18n;

pub(crate) struct Shared {
    pub(crate) session: Mutex<Option<DeviceSession>>,
}

fn combo_string_expression() -> gtk::PropertyExpression {
    gtk::PropertyExpression::new(
        gtk::StringObject::static_type(),
        gtk::Expression::NONE,
        "string",
    )
}

fn apply_button_row(title: &str) -> adw::ButtonRow {
    adw::ButtonRow::builder()
        .title(title)
        .css_classes(["suggested-action"])
        .build()
}

pub fn build_ui(app: &adw::Application) {
    let shared = Arc::new(Shared {
        session: Mutex::new(None),
    });

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title(i18n::app_title())
        .default_width(880)
        .default_height(720)
        .build();

    install_app_css();

    let toast_overlay = adw::ToastOverlay::new();
    let toolbar = adw::ToolbarView::new();

    let header = adw::HeaderBar::new();
    let win_title = adw::WindowTitle::new(i18n::app_title(), i18n::device_subtitle_loading());
    header.set_title_widget(Some(&win_title));

    let refresh_btn = gtk::Button::from_icon_name("view-refresh-symbolic");
    refresh_btn.set_tooltip_text(Some(i18n::looking_for_mouse()));
    header.pack_end(&refresh_btn);

    let app_menu = gio::Menu::new();
    app_menu.append(Some(i18n::keep_background()), Some("win.keep-background"));
    let app_menu_btn = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .tooltip_text(i18n::menu_app())
        .menu_model(&app_menu)
        .build();
    header.pack_end(&app_menu_btn);

    let bg_action = gio::SimpleAction::new_stateful(
        "keep-background",
        None,
        &AppConfig::load().keep_running_in_background.to_variant(),
    );
    bg_action.connect_activate(|action, _| {
        let current = action.state().and_then(|v| v.get::<bool>()).unwrap_or(false);
        let next = !current;
        action.set_state(&next.to_variant());
        let mut cfg = AppConfig::load();
        cfg.keep_running_in_background = next;
        let _ = cfg.save();
    });
    window.add_action(&bg_action);

    let profile_string_list = gtk::StringList::new(&[]);
    let profile_dropdown = gtk::DropDown::builder()
        .model(&profile_string_list)
        .build();
    profile_dropdown.set_size_request(200, -1);
    profile_dropdown.set_sensitive(false);
    profile_dropdown.set_tooltip_text(Some(i18n::profiles_on_device()));

    let profile_menu = gio::Menu::new();
    profile_menu.append(Some(i18n::create_profile()), Some("win.profile-create"));
    profile_menu.append(Some(i18n::rename()), Some("win.profile-rename"));
    profile_menu.append(Some(i18n::delete_profile()), Some("win.profile-delete"));
    let profile_menu_btn = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .tooltip_text(i18n::profile_actions())
        .menu_model(&profile_menu)
        .build();

    let profile_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    profile_box.set_margin_start(6);
    profile_box.append(&profile_dropdown);
    profile_box.append(&profile_menu_btn);
    header.pack_start(&profile_box);
    toolbar.add_top_bar(&header);

    let stack = adw::ViewStack::new();
    stack.set_vexpand(true);

    let switcher = adw::ViewSwitcher::builder()
        .stack(&stack)
        .policy(adw::ViewSwitcherPolicy::Wide)
        .build();
    let switcher_bar_top = adw::HeaderBar::new();
    switcher_bar_top.set_show_end_title_buttons(false);
    switcher_bar_top.set_show_start_title_buttons(false);
    switcher_bar_top.set_title_widget(Some(&switcher));
    toolbar.add_top_bar(&switcher_bar_top);

    let switcher_bar = adw::ViewSwitcherBar::new();
    switcher_bar.set_stack(Some(&stack));
    toolbar.add_bottom_bar(&switcher_bar);

    let bp = adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 600sp").unwrap());
    bp.add_setter(&switcher_bar, "reveal", Some(&true.to_value()));
    bp.add_setter(&switcher_bar_top, "visible", Some(&false.to_value()));
    window.add_breakpoint(bp);

    let updating_profiles = Rc::new(RefCell::new(false));
    let profile_slots: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
    let state_cell: Rc<RefCell<Option<MouseState>>> = Rc::new(RefCell::new(None));
    let showing_hypershift = Rc::new(RefCell::new(false));
    let updating_smart_dim = Rc::new(RefCell::new(false));

    // --- Buttons page ---
    let layer_seg = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    layer_seg.add_css_class("linked");
    layer_seg.set_halign(gtk::Align::Center);
    let layer_standard = gtk::ToggleButton::with_label(i18n::layer_standard());
    let layer_hypershift = gtk::ToggleButton::with_label(i18n::layer_hypershift());
    layer_hypershift.set_group(Some(&layer_standard));
    layer_standard.set_active(true);
    layer_seg.append(&layer_standard);
    layer_seg.append(&layer_hypershift);

    let layer_hint = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .css_classes(["dim-label"])
        .build();
    layer_hint.set_visible(false);

    let buttons_list = gtk::ListBox::new();
    buttons_list.add_css_class("boxed-list");
    buttons_list.set_selection_mode(gtk::SelectionMode::None);

    let buttons_inner = gtk::Box::new(gtk::Orientation::Vertical, 12);
    buttons_inner.set_margin_top(18);
    buttons_inner.set_margin_bottom(28);
    buttons_inner.set_margin_start(12);
    buttons_inner.set_margin_end(12);
    buttons_inner.append(
        &gtk::Label::builder()
            .label(i18n::section_buttons())
            .halign(gtk::Align::Start)
            .css_classes(["title-2"])
            .build(),
    );
    buttons_inner.append(
        &gtk::Label::builder()
            .label(i18n::buttons_page_desc())
            .halign(gtk::Align::Start)
            .wrap(true)
            .css_classes(["dim-label"])
            .build(),
    );
    buttons_inner.append(&layer_seg);
    buttons_inner.append(&layer_hint);
    buttons_inner.append(&buttons_list);

    let buttons_clamp = adw::Clamp::builder()
        .maximum_size(680)
        .tightening_threshold(400)
        .child(&buttons_inner)
        .build();
    let buttons_scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&buttons_clamp)
        .build();
    stack.add_titled_with_icon(
        &buttons_scroll,
        Some("buttons"),
        i18n::tab_buttons(),
        "input-mouse-symbolic",
    );

    // --- Performance ---
    let perf_page = adw::PreferencesPage::new();
    let dpi_group = adw::PreferencesGroup::builder()
        .title(i18n::section_dpi())
        .build();
    let dpi_spin = adw::SpinRow::with_range(100.0, 50_000.0, 50.0);
    dpi_spin.set_title(i18n::dpi_current_title());
    dpi_spin.set_subtitle("—");
    let stages_entry = adw::EntryRow::builder()
        .title(i18n::dpi_stages_title())
        .build();
    let dpi_apply = apply_button_row(i18n::apply());
    let stages_apply = apply_button_row(i18n::apply_stages());
    dpi_group.add(&dpi_spin);
    dpi_group.add(&dpi_apply);
    dpi_group.add(&stages_entry);
    dpi_group.add(&stages_apply);
    perf_page.add(&dpi_group);

    let poll_group = adw::PreferencesGroup::builder()
        .title(i18n::section_poll())
        .build();
    let poll_model = gtk::StringList::new(&["125 Hz", "250 Hz", "500 Hz", "1000 Hz", "2000 Hz", "4000 Hz", "8000 Hz"]);
    let poll_row = adw::ComboRow::builder()
        .title(i18n::section_poll())
        .model(&poll_model)
        .expression(&combo_string_expression())
        .build();
    let poll_apply = apply_button_row(i18n::apply());
    poll_group.add(&poll_row);
    poll_group.add(&poll_apply);
    perf_page.add(&poll_group);
    stack.add_titled_with_icon(
        &perf_page,
        Some("performance"),
        i18n::tab_performance(),
        "speedometer-symbolic",
    );

    // --- Lighting ---
    let light_page = adw::PreferencesPage::new();
    let light_group = adw::PreferencesGroup::builder()
        .title(i18n::lighting_group_title())
        .description(i18n::lighting_group_desc())
        .build();

    let zone_model = gtk::StringList::new(&[
        i18n::zone_all(),
        i18n::zone_logo(),
        i18n::zone_wheel(),
        i18n::zone_underglow(),
    ]);
    let zone_row = adw::ComboRow::builder()
        .title(i18n::zone_title())
        .model(&zone_model)
        .expression(&combo_string_expression())
        .build();
    let effect_model = gtk::StringList::new(&[
        i18n::effect_static(),
        i18n::effect_breathing(),
        i18n::effect_spectrum(),
        i18n::effect_reactive(),
        i18n::effect_wave(),
        i18n::effect_off(),
    ]);
    let effect_row = adw::ComboRow::builder()
        .title(i18n::effect_title())
        .model(&effect_model)
        .expression(&combo_string_expression())
        .build();

    let initial = gdk::RGBA::new(0.0, 1.0, 0.4, 1.0);
    let color_state = Rc::new(RefCell::new(initial));
    let swatch_css = gtk::CssProvider::new();
    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().expect("display"),
        &swatch_css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    let color_btn = gtk::Button::builder()
        .tooltip_text(i18n::color_dialog_title())
        .css_classes(["synapsex-color-swatch", "flat"])
        .build();
    color_btn.set_size_request(56, 36);
    set_swatch_color(&swatch_css, &initial);
    let hex_label = gtk::Label::builder()
        .label("#00ff66")
        .selectable(true)
        .css_classes(["monospace", "dim-label"])
        .build();
    let color_row = adw::ActionRow::builder().title(i18n::color_label()).build();
    color_row.add_suffix(&hex_label);
    color_row.add_suffix(&color_btn);
    color_row.set_activatable_widget(Some(&color_btn));
    color_btn.connect_clicked({
        let window = window.clone();
        let color_state = Rc::clone(&color_state);
        let hex_label = hex_label.clone();
        let swatch_css = swatch_css.clone();
        move |_| {
            open_color_editor(&window, *color_state.borrow(), {
                let color_state = Rc::clone(&color_state);
                let hex_label = hex_label.clone();
                let swatch_css = swatch_css.clone();
                move |rgba| {
                    *color_state.borrow_mut() = rgba;
                    set_swatch_color(&swatch_css, &rgba);
                    hex_label.set_label(&format_hex(&rgba));
                }
            });
        }
    });

    let presets_row = adw::ActionRow::builder()
        .title(i18n::color_presets_label())
        .build();
    let presets_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    for &(r, g, b) in &[
        (0x00u8, 0xffu8, 0x66u8),
        (0xff, 0x00, 0x44),
        (0xff, 0x6a, 0x00),
        (0xff, 0xd4, 0x00),
        (0x00, 0xc2, 0xff),
        (0x3d, 0x5a, 0xfe),
        (0xb3, 0x88, 0xff),
        (0xff, 0xff, 0xff),
        (0x22, 0x22, 0x22),
    ] {
        let chip = gtk::Button::new();
        chip.set_size_request(28, 28);
        chip.set_tooltip_text(Some(&format!("#{r:02x}{g:02x}{b:02x}")));
        let class = format!("color-chip-{r:02x}{g:02x}{b:02x}");
        chip.add_css_class(&class);
        let provider = gtk::CssProvider::new();
        provider.load_from_string(&format!(
            ".{class} {{ background: #{r:02x}{g:02x}{b:02x}; border-radius: 9999px; border: 1px solid alpha(@borders, 0.6); min-width: 24px; min-height: 24px; padding: 0; }}"
        ));
        gtk::style_context_add_provider_for_display(
            &gdk::Display::default().expect("display"),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        let color_state_p = Rc::clone(&color_state);
        let hex_label_p = hex_label.clone();
        let swatch_css_p = swatch_css.clone();
        chip.connect_clicked(move |_| {
            let rgba = gdk::RGBA::new(f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, 1.0);
            *color_state_p.borrow_mut() = rgba;
            set_swatch_color(&swatch_css_p, &rgba);
            hex_label_p.set_label(&format!("#{r:02x}{g:02x}{b:02x}"));
        });
        presets_box.append(&chip);
    }
    presets_row.add_suffix(&presets_box);

    let bright_row = adw::ActionRow::builder().title(i18n::brightness_label()).build();
    let bright_scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
    bright_scale.set_value(80.0);
    bright_scale.set_hexpand(true);
    bright_scale.set_draw_value(true);
    bright_scale.set_size_request(160, -1);
    bright_row.add_suffix(&bright_scale);

    let smart_dimming = adw::SwitchRow::builder()
        .title(i18n::smart_dimming())
        .subtitle(i18n::smart_dimming_hint())
        .build();
    let light_apply = apply_button_row(i18n::apply());

    light_group.add(&zone_row);
    light_group.add(&effect_row);
    light_group.add(&color_row);
    light_group.add(&presets_row);
    light_group.add(&bright_row);
    light_group.add(&smart_dimming);
    light_group.add(&light_apply);
    light_page.add(&light_group);

    let light_status_group = adw::PreferencesGroup::new();
    let light_status_label_row = adw::ActionRow::builder()
        .title(i18n::lighting_group_title())
        .subtitle("—")
        .activatable(false)
        .selectable(false)
        .build();
    light_status_group.add(&light_status_label_row);
    light_page.add(&light_status_group);

    stack.add_titled_with_icon(
        &light_page,
        Some("lighting"),
        i18n::tab_lighting(),
        "display-brightness-symbolic",
    );

    // --- More ---
    let more_page = adw::PreferencesPage::new();
    let wheel_group = adw::PreferencesGroup::builder()
        .title(i18n::section_wheel())
        .description(i18n::wheel_group_desc())
        .build();
    let smart_reel = adw::SwitchRow::builder().title(i18n::smart_reel()).build();
    let scroll_accel = adw::SwitchRow::builder().title(i18n::scroll_accel()).build();
    let wheel_apply = apply_button_row(i18n::apply());
    wheel_group.add(&smart_reel);
    wheel_group.add(&scroll_accel);
    wheel_group.add(&wheel_apply);
    more_page.add(&wheel_group);

    let power_group = adw::PreferencesGroup::builder()
        .title(i18n::section_power())
        .description(i18n::power_group_desc())
        .build();
    let idle_spin = adw::SpinRow::with_range(60.0, 900.0, 30.0);
    idle_spin.set_title(i18n::sleep_seconds());
    let low_spin = adw::SpinRow::with_range(5.0, 50.0, 1.0);
    low_spin.set_title(i18n::low_battery_pct());
    let power_apply = apply_button_row(i18n::apply());
    power_group.add(&idle_spin);
    power_group.add(&low_spin);
    power_group.add(&power_apply);
    more_page.add(&power_group);
    stack.add_titled_with_icon(
        &more_page,
        Some("more"),
        i18n::tab_more(),
        "preferences-other-symbolic",
    );

    stack.set_visible_child_name("buttons");
    toolbar.set_content(Some(&stack));
    toast_overlay.set_child(Some(&toolbar));
    window.set_content(Some(&toast_overlay));

    let follow_dpi = Rc::new(RefCell::new(0u16));
    let follow_dpi_stage = Rc::new(RefCell::new(usize::MAX));
    let refresh_busy = Rc::new(RefCell::new(false));
    let status = win_title.clone();

    let refresh = Rc::new({
        let shared = Arc::clone(&shared);
        let status = status.clone();
        let dpi_spin = dpi_spin.clone();
        let stages_entry = stages_entry.clone();
        let poll_row = poll_row.clone();
        let light_status_label_row = light_status_label_row.clone();
        let color_state = Rc::clone(&color_state);
        let swatch_css = swatch_css.clone();
        let hex_label = hex_label.clone();
        let bright_scale = bright_scale.clone();
        let smart_dimming = smart_dimming.clone();
        let updating_smart_dim = Rc::clone(&updating_smart_dim);
        let smart_reel = smart_reel.clone();
        let scroll_accel = scroll_accel.clone();
        let idle_spin = idle_spin.clone();
        let low_spin = low_spin.clone();
        let buttons_list = buttons_list.clone();
        let layer_hint = layer_hint.clone();
        let showing_hypershift = Rc::clone(&showing_hypershift);
        let profile_string_list = profile_string_list.clone();
        let profile_dropdown = profile_dropdown.clone();
        let updating_profiles = Rc::clone(&updating_profiles);
        let profile_slots = Rc::clone(&profile_slots);
        let state_cell = Rc::clone(&state_cell);
        let toast_overlay = toast_overlay.clone();
        let window = window.clone();
        let follow_dpi = Rc::clone(&follow_dpi);
        let follow_dpi_stage = Rc::clone(&follow_dpi_stage);
        let refresh_busy = Rc::clone(&refresh_busy);
        let refresh_btn = refresh_btn.clone();
        move || {
            if *refresh_busy.borrow() {
                return;
            }
            *refresh_busy.borrow_mut() = true;
            refresh_btn.set_sensitive(false);
            status.set_subtitle(i18n::loading_device());

            let shared_bg = Arc::clone(&shared);
            let shared_cb = Arc::clone(&shared);
            let refresh_busy_cb = Rc::clone(&refresh_busy);
            let refresh_btn_cb = refresh_btn.clone();
            let status_cb = status.clone();
            let dpi_spin_cb = dpi_spin.clone();
            let stages_entry_cb = stages_entry.clone();
            let poll_row_cb = poll_row.clone();
            let light_status_cb = light_status_label_row.clone();
            let color_state_cb = Rc::clone(&color_state);
            let swatch_css_cb = swatch_css.clone();
            let hex_label_cb = hex_label.clone();
            let bright_scale_cb = bright_scale.clone();
            let smart_dimming_cb = smart_dimming.clone();
            let updating_smart_dim_cb = Rc::clone(&updating_smart_dim);
            let smart_reel_cb = smart_reel.clone();
            let scroll_accel_cb = scroll_accel.clone();
            let idle_spin_cb = idle_spin.clone();
            let low_spin_cb = low_spin.clone();
            let buttons_list_cb = buttons_list.clone();
            let layer_hint_cb = layer_hint.clone();
            let showing_hypershift_cb = Rc::clone(&showing_hypershift);
            let profile_string_list_cb = profile_string_list.clone();
            let profile_dropdown_cb = profile_dropdown.clone();
            let updating_profiles_cb = Rc::clone(&updating_profiles);
            let profile_slots_cb = Rc::clone(&profile_slots);
            let state_cell_cb = Rc::clone(&state_cell);
            let toast_overlay_cb = toast_overlay.clone();
            let window_cb = window.clone();
            let follow_dpi_cb = Rc::clone(&follow_dpi);
            let follow_dpi_stage_cb = Rc::clone(&follow_dpi_stage);
            crate::hid_async::spawn_hid(
                move || -> Result<MouseState, String> {
                    let mut guard = shared_bg.session.lock().map_err(|e| e.to_string())?;
                    if guard.is_none() {
                        *guard = Some(DeviceSession::open().map_err(|e| e.to_string())?);
                    }
                    guard.as_mut().unwrap().read_state().map_err(|e| e.to_string())
                },
                move |result| {
                    match result {
                        Ok(st) => {
                            status_cb.set_title(&st.name);
                            status_cb.set_subtitle(&format!(
                                "{} · {} · fw {}",
                                i18n::battery_short(st.battery_percent, st.charging),
                                i18n::connection_label(&st.connection),
                                st.firmware
                            ));
                            dpi_spin_cb.set_subtitle(&i18n::current_dpi_with_stage(
                                st.dpi_x,
                                st.dpi_y,
                                st.active_dpi_stage,
                                st.dpi_stages.len(),
                            ));
                            dpi_spin_cb.set_value(st.dpi_x as f64);
                            *follow_dpi_cb.borrow_mut() = st.dpi_x;
                            *follow_dpi_stage_cb.borrow_mut() = st.active_dpi_stage;
                            stages_entry_cb.set_text(
                                &st.dpi_stages.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(","),
                            );
                            if st.poll_hz != 0 {
                                let rates = [125u32, 250, 500, 1000, 2000, 4000, 8000];
                                if let Some(idx) = rates.iter().position(|&r| r == st.poll_hz) {
                                    poll_row_cb.set_selected(idx as u32);
                                }
                                poll_row_cb.set_subtitle(&format!("{} Hz", st.poll_hz));
                            } else {
                                poll_row_cb.set_subtitle(i18n::poll_unknown());
                            }
                            light_status_cb.set_subtitle(
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
                                let rgba = rgba_from_rgb(z.color);
                                *color_state_cb.borrow_mut() = rgba;
                                set_swatch_color(&swatch_css_cb, &rgba);
                                hex_label_cb.set_label(&format_hex(&rgba));
                                if z.brightness > 0 {
                                    bright_scale_cb.set_value(z.brightness as f64);
                                }
                            }
                            *updating_smart_dim_cb.borrow_mut() = true;
                            smart_dimming_cb.set_active(st.smart_dimming);
                            *updating_smart_dim_cb.borrow_mut() = false;
                            smart_reel_cb.set_active(st.smart_reel);
                            scroll_accel_cb.set_active(st.scroll_accel);
                            idle_spin_cb.set_value(st.idle_seconds as f64);
                            low_spin_cb.set_value(st.low_battery_percent as f64);

                            *updating_profiles_cb.borrow_mut() = true;
                            while profile_string_list_cb.n_items() > 0 {
                                profile_string_list_cb.remove(0);
                            }
                            *profile_slots_cb.borrow_mut() = Vec::new();
                            let mut selected = 0u32;
                            for (idx, p) in st.profiles.iter().enumerate() {
                                profile_string_list_cb.append(&i18n::profile_dropdown_item(
                                    p.slot, &p.name, p.is_active,
                                ));
                                profile_slots_cb.borrow_mut().push(p.slot);
                                if p.slot == st.active_slot {
                                    selected = idx as u32;
                                }
                            }
                            profile_dropdown_cb.set_selected(selected);
                            profile_dropdown_cb.set_sensitive(!st.profiles.is_empty());
                            *updating_profiles_cb.borrow_mut() = false;

                            *state_cell_cb.borrow_mut() = Some(st);
                            render_button_layer(
                                &buttons_list_cb,
                                &layer_hint_cb,
                                &state_cell_cb,
                                *showing_hypershift_cb.borrow(),
                                &window_cb,
                                &shared_cb,
                                &toast_overlay_cb,
                            );
                        }
                        Err(e) => {
                            status_cb.set_title(i18n::app_title());
                            status_cb.set_subtitle(&i18n::no_mouse(&e));
                        }
                    }
                    *refresh_busy_cb.borrow_mut() = false;
                    refresh_btn_cb.set_sensitive(true);
                },
            );
        }
    });

    // Layer toggles
    {
        let showing_hypershift = Rc::clone(&showing_hypershift);
        let buttons_list = buttons_list.clone();
        let layer_hint = layer_hint.clone();
        let state_cell = Rc::clone(&state_cell);
        let shared = Arc::clone(&shared);
        let toast = toast_overlay.clone();
        let window = window.clone();
        layer_standard.connect_toggled({
            let showing_hypershift = Rc::clone(&showing_hypershift);
            let buttons_list = buttons_list.clone();
            let layer_hint = layer_hint.clone();
            let state_cell = Rc::clone(&state_cell);
            let shared = Arc::clone(&shared);
            let toast = toast.clone();
            let window = window.clone();
            move |btn| {
                if !btn.is_active() { return; }
                *showing_hypershift.borrow_mut() = false;
                render_button_layer(&buttons_list, &layer_hint, &state_cell, false, &window, &shared, &toast);
            }
        });
        layer_hypershift.connect_toggled(move |btn| {
            if !btn.is_active() { return; }
            *showing_hypershift.borrow_mut() = true;
            render_button_layer(&buttons_list, &layer_hint, &state_cell, true, &window, &shared, &toast);
        });
    }

    refresh();
    refresh_btn.connect_clicked({
        let refresh = Rc::clone(&refresh);
        move |_| refresh()
    });

    wire_dpi_apply(&dpi_apply, &dpi_spin, &shared, &refresh, &toast_overlay);
    wire_stages_apply(&stages_apply, &stages_entry, &shared, &refresh, &toast_overlay, &state_cell);
    wire_poll_apply(&poll_apply, &poll_row, &shared, &refresh, &toast_overlay);
    wire_light_apply(
        &light_apply, &effect_row, &color_state, &bright_scale, &zone_row, &shared, &refresh, &toast_overlay,
    );
    wire_smart_dimming_row(&smart_dimming, &updating_smart_dim, &shared, &toast_overlay);
    wire_wheel_apply(&wheel_apply, &smart_reel, &scroll_accel, &shared, &refresh, &toast_overlay);
    wire_power_apply(&power_apply, &idle_spin, &low_spin, &shared, &refresh, &toast_overlay);

    // Profile switch
    profile_dropdown.connect_selected_notify({
        let shared = Arc::clone(&shared);
        let refresh = Rc::clone(&refresh);
        let toast = toast_overlay.clone();
        let updating_profiles = Rc::clone(&updating_profiles);
        let profile_slots = Rc::clone(&profile_slots);
        let refresh_busy = Rc::clone(&refresh_busy);
        let state_cell = Rc::clone(&state_cell);
        let status = status.clone();
        let refresh_btn = refresh_btn.clone();
        move |dd| {
            if *updating_profiles.borrow() || *refresh_busy.borrow() {
                return;
            }
            let idx = dd.selected() as usize;
            let Some(&slot) = profile_slots.borrow().get(idx) else { return; };
            let current = state_cell.borrow().as_ref().map(|s| s.active_slot);
            if current == Some(slot) { return; }
            *refresh_busy.borrow_mut() = true;
            refresh_btn.set_sensitive(false);
            status.set_subtitle(i18n::loading_device());
            let shared_bg = Arc::clone(&shared);
            let refresh = Rc::clone(&refresh);
            let toast = toast.clone();
            let refresh_busy_cb = Rc::clone(&refresh_busy);
            let refresh_btn_cb = refresh_btn.clone();
            let status_cb = status.clone();
            crate::hid_async::spawn_hid(
                move || -> Result<u8, String> {
                    let mut guard = shared_bg.session.lock().map_err(|e| e.to_string())?;
                    let s = guard.as_mut().ok_or_else(|| "no session".to_string())?;
                    s.activate_slot(slot).map_err(|e| e.to_string())?;
                    Ok(slot)
                },
                move |result| match result {
                    Ok(slot) => {
                        toast.add_toast(adw::Toast::new(&i18n::toast_activated_reload(slot)));
                        *refresh_busy_cb.borrow_mut() = false;
                        refresh();
                    }
                    Err(e) => {
                        *refresh_busy_cb.borrow_mut() = false;
                        refresh_btn_cb.set_sensitive(true);
                        status_cb.set_subtitle(&i18n::toast_error(&e));
                        toast.add_toast(adw::Toast::new(&i18n::toast_error(&e)));
                    }
                },
            );
        }
    });

    // Profile actions
    {
        let shared = Arc::clone(&shared);
        let refresh: Rc<dyn Fn()> = Rc::clone(&refresh) as Rc<dyn Fn()>;
        let toast = toast_overlay.clone();
        let state_cell = Rc::clone(&state_cell);
        let window = window.clone();

        let create = gio::SimpleAction::new("profile-create", None);
        create.connect_activate({
            let shared = Arc::clone(&shared);
            let refresh = Rc::clone(&refresh);
            let toast = toast.clone();
            let state_cell = Rc::clone(&state_cell);
            move |_, _| do_create_profile(&shared, &refresh, &toast, &state_cell)
        });
        window.add_action(&create);

        let delete = gio::SimpleAction::new("profile-delete", None);
        delete.connect_activate({
            let shared = Arc::clone(&shared);
            let refresh = Rc::clone(&refresh);
            let toast = toast.clone();
            let state_cell = Rc::clone(&state_cell);
            let window = window.clone();
            move |_, _| {
                let slot = state_cell.borrow().as_ref().map(|s| s.active_slot).unwrap_or(1);
                if slot == 1 {
                    toast.add_toast(adw::Toast::new(i18n::toast_cannot_delete_slot1()));
                    return;
                }
                let dialog = adw::AlertDialog::builder()
                    .heading(i18n::delete_dialog_title())
                    .body(&i18n::delete_dialog_body(slot))
                    .build();
                dialog.add_response("cancel", i18n::cancel());
                dialog.add_response("delete", i18n::delete_profile());
                dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
                dialog.set_default_response(Some("cancel"));
                dialog.set_close_response("cancel");
                let shared = Arc::clone(&shared);
                let refresh = Rc::clone(&refresh);
                let toast = toast.clone();
                let state_cell = Rc::clone(&state_cell);
                dialog.connect_response(None, move |_, response| {
                    if response == "delete" {
                        do_delete_profile(&shared, &refresh, &toast, &state_cell);
                    }
                });
                dialog.present(Some(&window));
            }
        });
        window.add_action(&delete);

        let rename = gio::SimpleAction::new("profile-rename", None);
        rename.connect_activate({
            let shared = Arc::clone(&shared);
            let refresh = Rc::clone(&refresh);
            let toast = toast.clone();
            let state_cell = Rc::clone(&state_cell);
            let window = window.clone();
            move |_, _| {
                let current = state_cell
                    .borrow()
                    .as_ref()
                    .map(|s| s.slot_name.clone())
                    .unwrap_or_default();
                let entry = gtk::Entry::builder()
                    .text(&current)
                    .placeholder_text(i18n::rename_placeholder())
                    .hexpand(true)
                    .build();
                let dialog = adw::AlertDialog::builder()
                    .heading(i18n::rename_dialog_title())
                    .body(i18n::rename_dialog_body())
                    .extra_child(&entry)
                    .build();
                dialog.add_response("cancel", i18n::cancel());
                dialog.add_response("rename", i18n::rename());
                dialog.set_response_appearance("rename", adw::ResponseAppearance::Suggested);
                dialog.set_default_response(Some("rename"));
                dialog.set_close_response("cancel");
                let shared = Arc::clone(&shared);
                let refresh = Rc::clone(&refresh);
                let toast = toast.clone();
                dialog.connect_response(None, move |_, response| {
                    if response == "rename" {
                        do_rename_profile(entry.text().to_string(), &shared, &refresh, &toast);
                    }
                });
                dialog.present(Some(&window));
            }
        });
        window.add_action(&rename);
    }

    // Follow onboard profile/DPI changes (HID off GTK thread)
    {
        let shared = Arc::clone(&shared);
        let refresh = Rc::clone(&refresh);
        let toast = toast_overlay.clone();
        let dpi_spin = dpi_spin.clone();
        let state_cell = Rc::clone(&state_cell);
        let follow_dpi = Rc::clone(&follow_dpi);
        let follow_dpi_stage = Rc::clone(&follow_dpi_stage);
        let last_slot = Rc::new(RefCell::new(0u8));
        let refresh_busy = Rc::clone(&refresh_busy);
        let poll_inflight = Rc::new(RefCell::new(false));
        glib::timeout_add_local(std::time::Duration::from_millis(400), move || {
            if *refresh_busy.borrow() || *poll_inflight.borrow() {
                return glib::ControlFlow::Continue;
            }
            *poll_inflight.borrow_mut() = true;
            let shared_bg = Arc::clone(&shared);
            let prev_slot = *last_slot.borrow();
            let prev_dpi = *follow_dpi.borrow();
            let prev_stage = *follow_dpi_stage.borrow();
            let last_slot_cb = Rc::clone(&last_slot);
            let follow_dpi_cb = Rc::clone(&follow_dpi);
            let follow_dpi_stage_cb = Rc::clone(&follow_dpi_stage);
            let dpi_spin_cb = dpi_spin.clone();
            let state_cell_cb = Rc::clone(&state_cell);
            let toast_cb = toast.clone();
            let refresh_cb = Rc::clone(&refresh);
            let poll_inflight_cb = Rc::clone(&poll_inflight);
            crate::hid_async::spawn_hid(
                move || -> Result<(bool, Option<(u16, usize, usize)>, u8), String> {
                    let mut guard = match shared_bg.session.try_lock() {
                        Ok(g) => g,
                        Err(_) => return Ok((false, None, prev_slot)),
                    };
                    let Some(s) = guard.as_mut() else {
                        return Ok((false, None, prev_slot));
                    };
                    let mut slot_changed = false;
                    let mut active = s.active_slot;
                    match s.poll_slot_change() {
                        Ok(true) => {
                            active = s.active_slot;
                            slot_changed = true;
                        }
                        Ok(false) if prev_slot == 0 => {
                            active = s.active_slot;
                        }
                        _ => {}
                    }
                    let dpi_update = if slot_changed {
                        None
                    } else {
                        s.poll_dpi_change(prev_dpi, prev_stage).ok().flatten()
                    };
                    Ok((slot_changed, dpi_update, active))
                },
                move |result| {
                    *poll_inflight_cb.borrow_mut() = false;
                    let Ok((slot_changed, dpi_update, active)) = result else { return; };
                    if slot_changed {
                        *last_slot_cb.borrow_mut() = active;
                        toast_cb.add_toast(adw::Toast::new(i18n::toast_profile_changed()));
                        refresh_cb();
                    } else {
                        if prev_slot == 0 {
                            *last_slot_cb.borrow_mut() = active;
                        }
                        if let Some((dpi, stage, count)) = dpi_update {
                            *follow_dpi_cb.borrow_mut() = dpi;
                            *follow_dpi_stage_cb.borrow_mut() = stage;
                            dpi_spin_cb.set_subtitle(&i18n::current_dpi_with_stage(dpi, dpi, stage, count));
                            dpi_spin_cb.set_value(dpi as f64);
                            if let Some(st) = state_cell_cb.borrow_mut().as_mut() {
                                st.dpi_x = dpi;
                                st.dpi_y = dpi;
                                st.active_dpi_stage = stage;
                            }
                        }
                    }
                },
            );
            glib::ControlFlow::Continue
        });
    }

    window.connect_close_request({
        let bg_action = bg_action.clone();
        let window = window.clone();
        move |_| {
            let keep = bg_action.state().and_then(|v| v.get::<bool>()).unwrap_or(false);
            if keep {
                window.set_visible(false);
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        }
    });

    window.present();
}


fn install_app_css() {
    static DONE: std::sync::Once = std::sync::Once::new();
    DONE.call_once(|| {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(
            r#"
            .synapsex-color-swatch {
                border-radius: 8px;
                border: 1px solid alpha(@borders, 0.7);
                min-width: 56px;
                min-height: 36px;
                padding: 0;
            }
            /* Color editor: give the plane room so GTK doesn't wrap it in a cramped scroll */
            .synapsex-color-editor colorchooser {
                min-width: 300px;
                min-height: 340px;
            }
            .synapsex-color-editor colorchooser scrolledwindow {
                min-height: 280px;
            }
            .synapsex-color-editor colorchooser scrollbar {
                opacity: 0;
                min-width: 0;
                min-height: 0;
            }
            "#,
        );
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
}

fn format_hex(c: &gdk::RGBA) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        (c.red() * 255.0).round() as u8,
        (c.green() * 255.0).round() as u8,
        (c.blue() * 255.0).round() as u8
    )
}

fn set_swatch_color(provider: &gtk::CssProvider, c: &gdk::RGBA) {
    let hex = format_hex(c);
    provider.load_from_string(&format!(
        ".synapsex-color-swatch {{ background: {hex}; border-radius: 8px; border: 1px solid alpha(@borders, 0.7); min-width: 56px; min-height: 36px; padding: 0; }}"
    ));
}

#[allow(deprecated)] // ColorChooserWidget deprecated for ColorDialog, which scrolls badly in the editor
fn open_color_editor(
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
        .halign(gtk::Align::Fill)
        .valign(gtk::Align::Fill)
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
        let chooser = chooser.clone();
        move |_| {
            on_select(chooser.rgba());
            dialog.close();
        }
    });

    dialog.present(Some(parent));
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

fn wire_dpi_apply(
    btn: &adw::ButtonRow,
    spin: &adw::SpinRow,
    shared: &Arc<Shared>,
    refresh: &Rc<impl Fn() + 'static>,
    toast: &adw::ToastOverlay,
) {
    let shared = Arc::clone(shared);
    let spin = spin.clone();
    let refresh = Rc::clone(refresh);
    let toast = toast.clone();
    btn.connect_activated(move |_| {
        let dpi = spin.value() as u16;
        let shared_bg = Arc::clone(&shared);
        let refresh = Rc::clone(&refresh);
        let toast = toast.clone();
        crate::hid_async::spawn_hid(
            move || {
                let mut guard = shared_bg.session.lock().map_err(|e| e.to_string())?;
                let s = guard.as_mut().ok_or_else(|| "no session".to_string())?;
                s.set_dpi(dpi, dpi).map_err(|e| e.to_string())
            },
            move |result| match result {
                Ok(()) => {
                    toast.add_toast(adw::Toast::new(i18n::toast_dpi_written()));
                    refresh();
                }
                Err(e) => toast.add_toast(adw::Toast::new(&i18n::toast_error(&e))),
            },
        );
    });
}

fn wire_stages_apply(
    btn: &adw::ButtonRow,
    entry: &adw::EntryRow,
    shared: &Arc<Shared>,
    refresh: &Rc<impl Fn() + 'static>,
    toast: &adw::ToastOverlay,
    state_cell: &Rc<RefCell<Option<MouseState>>>,
) {
    let shared = Arc::clone(shared);
    let entry = entry.clone();
    let refresh = Rc::clone(refresh);
    let toast = toast.clone();
    let state_cell = Rc::clone(state_cell);
    btn.connect_activated(move |_| {
        let stages: Vec<u16> = entry
            .text()
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter_map(|s| s.parse().ok())
            .collect();
        if stages.is_empty() {
            return;
        }
        let active = state_cell.borrow().as_ref().map_or(0, |st| {
            stages
                .iter()
                .position(|&s| s == st.dpi_x)
                .unwrap_or_else(|| st.active_dpi_stage.min(stages.len() - 1))
        });
        let shared_bg = Arc::clone(&shared);
        let refresh = Rc::clone(&refresh);
        let toast = toast.clone();
        crate::hid_async::spawn_hid(
            move || {
                let mut guard = shared_bg.session.lock().map_err(|e| e.to_string())?;
                let s = guard.as_mut().ok_or_else(|| "no session".to_string())?;
                s.set_dpi_stages(&stages, active).map_err(|e| e.to_string())
            },
            move |result| match result {
                Ok(()) => {
                    toast.add_toast(adw::Toast::new(i18n::toast_stages_written()));
                    refresh();
                }
                Err(e) => toast.add_toast(adw::Toast::new(&i18n::toast_error(&e))),
            },
        );
    });
}

fn wire_poll_apply(
    btn: &adw::ButtonRow,
    row: &adw::ComboRow,
    shared: &Arc<Shared>,
    refresh: &Rc<impl Fn() + 'static>,
    toast: &adw::ToastOverlay,
) {
    let shared = Arc::clone(shared);
    let row = row.clone();
    let refresh = Rc::clone(refresh);
    let toast = toast.clone();
    btn.connect_activated(move |_| {
        let rates = [125u32, 250, 500, 1000, 2000, 4000, 8000];
        let hz = rates.get(row.selected() as usize).copied().unwrap_or(1000);
        let shared_bg = Arc::clone(&shared);
        let refresh = Rc::clone(&refresh);
        let toast = toast.clone();
        crate::hid_async::spawn_hid(
            move || {
                let mut guard = shared_bg.session.lock().map_err(|e| e.to_string())?;
                let s = guard.as_mut().ok_or_else(|| "no session".to_string())?;
                s.set_poll(hz).map_err(|e| e.to_string())?;
                Ok::<u32, String>(s.read_poll_hz())
            },
            move |result| match result {
                Err(e) => toast.add_toast(adw::Toast::new(&i18n::toast_error(&e))),
                Ok(0) => {
                    toast.add_toast(adw::Toast::new(i18n::toast_poll_written()));
                    refresh();
                }
                Ok(readback) => {
                    toast.add_toast(adw::Toast::new(&i18n::toast_poll_confirmed(readback)));
                    refresh();
                }
            },
        );
    });
}

fn wire_light_apply(
    btn: &adw::ButtonRow,
    effect_row: &adw::ComboRow,
    color_state: &Rc<RefCell<gdk::RGBA>>,
    bright: &gtk::Scale,
    zone_row: &adw::ComboRow,
    shared: &Arc<Shared>,
    refresh: &Rc<impl Fn() + 'static>,
    toast: &adw::ToastOverlay,
) {
    let shared = Arc::clone(shared);
    let effect_row = effect_row.clone();
    let color_state = Rc::clone(color_state);
    let bright = bright.clone();
    let zone_row = zone_row.clone();
    let refresh = Rc::clone(refresh);
    let toast = toast.clone();
    btn.connect_activated(move |_| {
        let effects = [
            LightingEffect::Static,
            LightingEffect::Breathing,
            LightingEffect::Spectrum,
            LightingEffect::Reactive,
            LightingEffect::Wave,
            LightingEffect::Off,
        ];
        let effect = effects
            .get(effect_row.selected() as usize)
            .copied()
            .unwrap_or(LightingEffect::Static);
        let color = rgb_from_rgba(&color_state.borrow());
        let pct = bright.value() as u8;
        let zone_idx = zone_row.selected();
        let shared_bg = Arc::clone(&shared);
        let refresh = Rc::clone(&refresh);
        let toast = toast.clone();
        crate::hid_async::spawn_hid(
            move || {
                let mut guard = shared_bg.session.lock().map_err(|e| e.to_string())?;
                let s = guard.as_mut().ok_or_else(|| "no session".to_string())?;
                if zone_idx == 0 {
                    s.set_all_zones(effect, color, pct)
                } else {
                    let zones = [
                        LightingZone::Logo,
                        LightingZone::ScrollWheel,
                        LightingZone::Underglow,
                    ];
                    s.set_zone_lighting(
                        zones[(zone_idx as usize - 1).min(2)],
                        effect,
                        color,
                        pct,
                    )
                }
                .map_err(|e| e.to_string())
            },
            move |result| match result {
                Ok(()) => {
                    toast.add_toast(adw::Toast::new(i18n::toast_lighting_saved()));
                    refresh();
                }
                Err(e) => toast.add_toast(adw::Toast::new(&i18n::toast_error(&e))),
            },
        );
    });
}

fn wire_wheel_apply(
    btn: &adw::ButtonRow,
    smart: &adw::SwitchRow,
    accel: &adw::SwitchRow,
    shared: &Arc<Shared>,
    refresh: &Rc<impl Fn() + 'static>,
    toast: &adw::ToastOverlay,
) {
    let shared = Arc::clone(shared);
    let smart = smart.clone();
    let accel = accel.clone();
    let refresh = Rc::clone(refresh);
    let toast = toast.clone();
    btn.connect_activated(move |_| {
        let sr = smart.is_active();
        let sa = accel.is_active();
        let shared_bg = Arc::clone(&shared);
        let refresh = Rc::clone(&refresh);
        let toast = toast.clone();
        crate::hid_async::spawn_hid(
            move || {
                let mut guard = shared_bg.session.lock().map_err(|e| e.to_string())?;
                let s = guard.as_mut().ok_or_else(|| "no session".to_string())?;
                s.set_smart_reel(sr).map_err(|e| e.to_string())?;
                s.set_scroll_accel(sa).map_err(|e| e.to_string())
            },
            move |result| match result {
                Ok(()) => {
                    toast.add_toast(adw::Toast::new(i18n::toast_wheel_saved()));
                    refresh();
                }
                Err(e) => toast.add_toast(adw::Toast::new(&i18n::toast_error(&e))),
            },
        );
    });
}

fn wire_smart_dimming_row(
    row: &adw::SwitchRow,
    updating: &Rc<RefCell<bool>>,
    shared: &Arc<Shared>,
    toast: &adw::ToastOverlay,
) {
    let shared = Arc::clone(shared);
    let toast = toast.clone();
    let updating = Rc::clone(updating);
    row.connect_active_notify(move |c| {
        if *updating.borrow() {
            return;
        }
        let on = c.is_active();
        let shared_bg = Arc::clone(&shared);
        let toast = toast.clone();
        crate::hid_async::spawn_hid(
            move || {
                let mut guard = shared_bg.session.lock().map_err(|e| e.to_string())?;
                let s = guard.as_mut().ok_or_else(|| "no session".to_string())?;
                s.set_smart_dimming(on).map_err(|e| e.to_string())
            },
            move |result| match result {
                Ok(()) => toast.add_toast(adw::Toast::new(if on {
                    i18n::toast_smart_dimming_on()
                } else {
                    i18n::toast_smart_dimming_off()
                })),
                Err(e) => toast.add_toast(adw::Toast::new(&i18n::toast_error(&e))),
            },
        );
    });
}

fn wire_power_apply(
    btn: &adw::ButtonRow,
    idle: &adw::SpinRow,
    low: &adw::SpinRow,
    shared: &Arc<Shared>,
    refresh: &Rc<impl Fn() + 'static>,
    toast: &adw::ToastOverlay,
) {
    let shared = Arc::clone(shared);
    let idle = idle.clone();
    let low = low.clone();
    let refresh = Rc::clone(refresh);
    let toast = toast.clone();
    btn.connect_activated(move |_| {
        let i = idle.value() as u16;
        let l = low.value() as u8;
        let shared_bg = Arc::clone(&shared);
        let refresh = Rc::clone(&refresh);
        let toast = toast.clone();
        crate::hid_async::spawn_hid(
            move || {
                let mut guard = shared_bg.session.lock().map_err(|e| e.to_string())?;
                let s = guard.as_mut().ok_or_else(|| "no session".to_string())?;
                s.set_idle(i).map_err(|e| e.to_string())?;
                s.set_low_battery(l).map_err(|e| e.to_string())
            },
            move |result| match result {
                Ok(()) => {
                    toast.add_toast(adw::Toast::new(i18n::toast_power_saved()));
                    refresh();
                }
                Err(e) => toast.add_toast(adw::Toast::new(&i18n::toast_error(&e))),
            },
        );
    });
}

fn do_create_profile(
    shared: &Arc<Shared>,
    refresh: &Rc<dyn Fn()>,
    toast: &adw::ToastOverlay,
    state_cell: &Rc<RefCell<Option<MouseState>>>,
) {
    let next = state_cell
        .borrow()
        .as_ref()
        .and_then(|s| (1..=5u8).find(|n| !s.slots.contains(n)));
    let Some(slot) = next else {
        toast.add_toast(adw::Toast::new(i18n::toast_slots_full()));
        return;
    };
    let shared_bg = Arc::clone(shared);
    let refresh = Rc::clone(refresh);
    let toast = toast.clone();
    crate::hid_async::spawn_hid(
        move || {
            let mut guard = shared_bg.session.lock().map_err(|e| e.to_string())?;
            let s = guard.as_mut().ok_or_else(|| "no session".to_string())?;
            s.create_slot(slot).map_err(|e| e.to_string())?;
            s.activate_slot(slot).map_err(|e| e.to_string())?;
            Ok::<u8, String>(slot)
        },
        move |result| match result {
            Ok(slot) => {
                toast.add_toast(adw::Toast::new(&i18n::toast_created(slot)));
                refresh();
            }
            Err(e) => toast.add_toast(adw::Toast::new(&i18n::toast_error(&e))),
        },
    );
}

fn do_delete_profile(
    shared: &Arc<Shared>,
    refresh: &Rc<dyn Fn()>,
    toast: &adw::ToastOverlay,
    state_cell: &Rc<RefCell<Option<MouseState>>>,
) {
    let slot = state_cell
        .borrow()
        .as_ref()
        .map(|s| s.active_slot)
        .unwrap_or(1);
    if slot == 1 {
        toast.add_toast(adw::Toast::new(i18n::toast_cannot_delete_slot1()));
        return;
    }
    let shared_bg = Arc::clone(shared);
    let refresh = Rc::clone(refresh);
    let toast = toast.clone();
    crate::hid_async::spawn_hid(
        move || {
            let mut guard = shared_bg.session.lock().map_err(|e| e.to_string())?;
            let s = guard.as_mut().ok_or_else(|| "no session".to_string())?;
            s.remove_slot(slot).map_err(|e| e.to_string())?;
            Ok::<u8, String>(slot)
        },
        move |result| match result {
            Ok(slot) => {
                toast.add_toast(adw::Toast::new(&i18n::toast_deleted(slot)));
                refresh();
            }
            Err(e) => toast.add_toast(adw::Toast::new(&i18n::toast_error(&e))),
        },
    );
}

fn do_rename_profile(
    name: String,
    shared: &Arc<Shared>,
    refresh: &Rc<dyn Fn()>,
    toast: &adw::ToastOverlay,
) {
    if name.is_empty() {
        return;
    }
    let shared_bg = Arc::clone(shared);
    let refresh = Rc::clone(refresh);
    let toast = toast.clone();
    crate::hid_async::spawn_hid(
        move || {
            let mut guard = shared_bg.session.lock().map_err(|e| e.to_string())?;
            let s = guard.as_mut().ok_or_else(|| "no session".to_string())?;
            let slot = s.active_slot;
            s.rename_slot(slot, name).map_err(|e| e.to_string())
        },
        move |result| match result {
            Ok(()) => {
                toast.add_toast(adw::Toast::new(i18n::toast_renamed()));
                refresh();
            }
            Err(e) => toast.add_toast(adw::Toast::new(&i18n::toast_error(&e))),
        },
    );
}

/// Show either the normal or HyperShift hardware binds in the single button list.
fn render_button_layer(
    list: &gtk::ListBox,
    hint: &gtk::Label,
    state_cell: &Rc<RefCell<Option<MouseState>>>,
    hypershift: bool,
    parent: &impl IsA<gtk::Window>,
    shared: &Arc<Shared>,
    toast: &adw::ToastOverlay,
) {
    let clutch_names = state_cell.borrow().as_ref().map(|st| {
        st.buttons
            .iter()
            .filter(|b| onboard::is_hypershift_block(&b.block))
            .map(|b| i18n::button_control_name(b.name))
            .collect::<Vec<_>>()
    });
    if hypershift {
        let clutch = clutch_names
            .as_ref()
            .filter(|v| !v.is_empty())
            .map(|v| v.join(", "));
        hint.set_label(&i18n::hypershift_hint(clutch.as_deref()));
        hint.set_visible(true);
    } else {
        hint.set_label("");
        hint.set_visible(false);
    }
    let buttons: Vec<ButtonState> = state_cell
        .borrow()
        .as_ref()
        .map(|st| {
            if hypershift {
                st.hypershift_buttons.clone()
            } else {
                st.buttons.clone()
            }
        })
        .unwrap_or_default();
    populate_button_list(list, &buttons, parent, shared, toast);
}

/// Fill a button list from hardware state with Serpentine-style nested remap menus.
fn populate_button_list(
    list: &gtk::ListBox,
    buttons: &[ButtonState],
    parent: &impl IsA<gtk::Window>,
    shared: &Arc<Shared>,
    toast: &adw::ToastOverlay,
) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    for b in buttons {
        let label = i18n::button_label(&b.label);
        let factory = onboard::basilisk_v4_pro_buttons()
            .iter()
            .find(|c| c.table_id == b.table_id)
            .map(|c| c.factory)
            .unwrap_or([0x00, 0x01, 0x00, 0, 0, 0, 0]);
        let row = adw::ActionRow::builder()
            .title(i18n::button_control_name(b.name))
            .subtitle(&label)
            .build();
        let menu_btn = crate::remap_menu::remap_menu_button(
            parent,
            shared,
            toast,
            b.table_id,
            b.layer,
            factory,
            b.block,
            &label,
            &row,
        );
        row.add_suffix(&menu_btn);
        list.append(&row);
    }
}
