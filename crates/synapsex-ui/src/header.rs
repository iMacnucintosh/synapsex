//! Header bar: device title, onboard profile selector and app menu.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::gio;
use razer_device::{AppConfig, MouseState};
use synapsex_i18n as i18n;

use crate::ctx::Ctx;

pub struct Header {
    pub bar: adw::HeaderBar,
    pub keep_background: gio::SimpleAction,
    title: adw::WindowTitle,
    refresh_btn: gtk::Button,
    profiles: gtk::StringList,
    dropdown: gtk::DropDown,
    slots: RefCell<Vec<u8>>,
    updating: Cell<bool>,
}

impl Header {
    pub fn new(ctx: &Rc<Ctx>) -> Rc<Self> {
        let bar = adw::HeaderBar::new();
        let title = adw::WindowTitle::new(i18n::app_title(), i18n::device_subtitle_loading());
        bar.set_title_widget(Some(&title));

        let profiles = gtk::StringList::new(&[]);
        let dropdown = gtk::DropDown::builder()
            .model(&profiles)
            .sensitive(false)
            .tooltip_text(i18n::profiles_on_device())
            .build();
        dropdown.set_size_request(200, -1);

        let profile_menu = gio::Menu::new();
        profile_menu.append(Some(i18n::create_profile()), Some("win.profile-create"));
        profile_menu.append(Some(i18n::rename()), Some("win.profile-rename"));
        profile_menu.append(Some(i18n::delete_profile()), Some("win.profile-delete"));
        let profile_menu_btn = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .tooltip_text(i18n::profile_actions())
            .menu_model(&profile_menu)
            .build();

        let profile_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        profile_box.add_css_class("linked");
        profile_box.append(&dropdown);
        profile_box.append(&profile_menu_btn);
        bar.pack_start(&profile_box);

        let app_menu = gio::Menu::new();
        app_menu.append(Some(i18n::keep_background()), Some("win.keep-background"));
        bar.pack_end(
            &gtk::MenuButton::builder()
                .icon_name("open-menu-symbolic")
                .tooltip_text(i18n::menu_app())
                .menu_model(&app_menu)
                .primary(true)
                .build(),
        );

        let refresh_btn = gtk::Button::from_icon_name("view-refresh-symbolic");
        refresh_btn.set_tooltip_text(Some(i18n::looking_for_mouse()));
        bar.pack_end(&refresh_btn);
        refresh_btn.connect_clicked({
            let ctx = Rc::clone(ctx);
            move |_| ctx.refresh()
        });

        let keep_background = gio::SimpleAction::new_stateful(
            "keep-background",
            None,
            &AppConfig::load().keep_running_in_background.to_variant(),
        );
        keep_background.connect_activate(|action, _| {
            let next = !action.state().and_then(|v| v.get::<bool>()).unwrap_or(false);
            action.set_state(&next.to_variant());
            let mut cfg = AppConfig::load();
            cfg.keep_running_in_background = next;
            let _ = cfg.save();
        });
        ctx.window.add_action(&keep_background);

        let header = Rc::new(Self {
            bar,
            keep_background,
            title,
            refresh_btn,
            profiles,
            dropdown,
            slots: RefCell::new(Vec::new()),
            updating: Cell::new(false),
        });
        header.wire_switch(ctx);
        install_profile_actions(ctx);
        header
    }

    pub fn set_loading(&self) {
        self.refresh_btn.set_sensitive(false);
        self.title.set_subtitle(i18n::loading_device());
    }

    pub fn set_error(&self, err: &str) {
        self.refresh_btn.set_sensitive(true);
        self.title.set_title(i18n::app_title());
        self.title.set_subtitle(&i18n::no_mouse(err));
    }

    pub fn apply(&self, st: &MouseState) {
        self.refresh_btn.set_sensitive(true);
        self.title.set_title(&st.name);
        self.title.set_subtitle(&format!(
            "{} · {} · fw {}",
            i18n::battery_short(st.battery_percent, st.charging),
            i18n::connection_label(&st.connection),
            st.firmware
        ));

        self.updating.set(true);
        let names: Vec<String> = st
            .profiles
            .iter()
            .map(|p| i18n::profile_dropdown_item(p.slot, &p.name, p.is_active))
            .collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        self.profiles.splice(0, self.profiles.n_items(), &names);
        *self.slots.borrow_mut() = st.profiles.iter().map(|p| p.slot).collect();
        let selected = st
            .profiles
            .iter()
            .position(|p| p.slot == st.active_slot)
            .unwrap_or(0);
        self.dropdown.set_selected(selected as u32);
        self.dropdown.set_sensitive(!st.profiles.is_empty());
        self.updating.set(false);
    }

    fn wire_switch(self: &Rc<Self>, ctx: &Rc<Ctx>) {
        let header = Rc::downgrade(self);
        let ctx = Rc::clone(ctx);
        self.dropdown.connect_selected_notify(move |dd| {
            let Some(header) = header.upgrade() else {
                return;
            };
            if header.updating.get() || ctx.busy.get() {
                return;
            }
            let Some(&slot) = header.slots.borrow().get(dd.selected() as usize) else {
                return;
            };
            if ctx.state.borrow().as_ref().map(|s| s.active_slot) == Some(slot) {
                return;
            }
            ctx.busy.set(true);
            header.set_loading();
            ctx.hid_then(
                move |s| Ok(s.activate_slot(slot)?),
                move |ctx, res| {
                    ctx.busy.set(false);
                    match res {
                        Ok(()) => {
                            ctx.toast(&i18n::toast_activated_reload(slot));
                            ctx.refresh();
                        }
                        Err(e) => {
                            header.set_error(&e);
                            ctx.toast(&i18n::toast_error(&e));
                        }
                    }
                },
            );
        });
    }
}

fn active_slot(ctx: &Ctx) -> u8 {
    ctx.state.borrow().as_ref().map(|s| s.active_slot).unwrap_or(1)
}

fn install_profile_actions(ctx: &Rc<Ctx>) {
    let create = gio::SimpleAction::new("profile-create", None);
    create.connect_activate({
        let ctx = Rc::clone(ctx);
        move |_, _| {
            let next = ctx
                .state
                .borrow()
                .as_ref()
                .and_then(|s| (1..=5u8).find(|n| !s.slots.contains(n)));
            let Some(slot) = next else {
                ctx.toast(i18n::toast_slots_full());
                return;
            };
            ctx.hid(
                move |s| {
                    s.create_slot(slot)?;
                    s.activate_slot(slot)?;
                    Ok(())
                },
                move |ctx, ()| {
                    ctx.toast(&i18n::toast_created(slot));
                    ctx.refresh();
                },
            );
        }
    });
    ctx.window.add_action(&create);

    let delete = gio::SimpleAction::new("profile-delete", None);
    delete.connect_activate({
        let ctx = Rc::clone(ctx);
        move |_, _| {
            let slot = active_slot(&ctx);
            if slot == 1 {
                ctx.toast(i18n::toast_cannot_delete_slot1());
                return;
            }
            let dialog = adw::AlertDialog::builder()
                .heading(i18n::delete_dialog_title())
                .body(i18n::delete_dialog_body(slot))
                .default_response("cancel")
                .close_response("cancel")
                .build();
            dialog.add_response("cancel", i18n::cancel());
            dialog.add_response("delete", i18n::delete_profile());
            dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
            let ctx_cb = Rc::clone(&ctx);
            dialog.connect_response(Some("delete"), move |_, _| {
                ctx_cb.hid(
                    move |s| Ok(s.remove_slot(slot)?),
                    move |ctx, ()| {
                        ctx.toast(&i18n::toast_deleted(slot));
                        ctx.refresh();
                    },
                );
            });
            dialog.present(Some(&ctx.window));
        }
    });
    ctx.window.add_action(&delete);

    let rename = gio::SimpleAction::new("profile-rename", None);
    rename.connect_activate({
        let ctx = Rc::clone(ctx);
        move |_, _| {
            let current = ctx
                .state
                .borrow()
                .as_ref()
                .map(|s| s.slot_name.clone())
                .unwrap_or_default();
            let entry = gtk::Entry::builder()
                .text(&current)
                .placeholder_text(i18n::rename_placeholder())
                .activates_default(true)
                .hexpand(true)
                .build();
            let dialog = adw::AlertDialog::builder()
                .heading(i18n::rename_dialog_title())
                .body(i18n::rename_dialog_body())
                .extra_child(&entry)
                .default_response("rename")
                .close_response("cancel")
                .build();
            dialog.add_response("cancel", i18n::cancel());
            dialog.add_response("rename", i18n::rename());
            dialog.set_response_appearance("rename", adw::ResponseAppearance::Suggested);
            let ctx_cb = Rc::clone(&ctx);
            dialog.connect_response(Some("rename"), move |_, _| {
                let name = entry.text().trim().to_string();
                if name.is_empty() {
                    return;
                }
                let slot = active_slot(&ctx_cb);
                ctx_cb.hid(
                    move |s| Ok(s.rename_slot(slot, name)?),
                    move |ctx, ()| {
                        ctx.toast(i18n::toast_renamed());
                        ctx.refresh();
                    },
                );
            });
            dialog.present(Some(&ctx.window));
        }
    });
    ctx.window.add_action(&rename);
}
