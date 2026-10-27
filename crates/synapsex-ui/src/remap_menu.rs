//! Serpentine-style remap picker: nested menus + optional key-capture dialog.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use adw::prelude::*;
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use razer_protocol::onboard::{self, RemapItem};
use synapsex_i18n as i18n;

use crate::app::Shared;

pub fn block_token(block: &[u8; 7]) -> String {
    block.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn parse_block_token(s: &str) -> Option<[u8; 7]> {
    if s.len() != 14 {
        return None;
    }
    let mut out = [0u8; 7];
    for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
        let hi = hex_nibble(chunk[0])?;
        let lo = hex_nibble(chunk[1])?;
        out[i] = (hi << 4) | lo;
    }
    Some(out)
}

fn hex_nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

fn append_items(menu: &gio::Menu, items: &[RemapItem]) {
    for item in items {
        let label = i18n::media_label(item.label);
        let detail = format!("app.apply('{}')", block_token(&item.block));
        menu.append(Some(&label), Some(&detail));
    }
}

/// Build the Gio menu model for one button row (categories + record / default / disable).
pub fn build_remap_menu_model() -> gio::Menu {
    let root = gio::Menu::new();

    let keyboard = gio::Menu::new();
    keyboard.append(Some(i18n::remap_record()), Some("app.record"));
    let digits = gio::Menu::new();
    append_items(&digits, onboard::remap_digits());
    keyboard.append_submenu(Some(i18n::remap_digits()), &digits);
    let keypad = gio::Menu::new();
    append_items(&keypad, onboard::remap_keypad());
    keyboard.append_submenu(Some(i18n::remap_keypad()), &keypad);
    let fkeys = gio::Menu::new();
    append_items(&fkeys, onboard::remap_f_keys());
    // Section header: F13+ must be bound as XF86* in Hypr/Caelestia (xkb inet map).
    let fkeys_section = gio::Menu::new();
    fkeys_section.append_section(Some(i18n::remap_f_keys_linux_note()), &fkeys);
    keyboard.append_submenu(Some(i18n::remap_f_keys()), &fkeys_section);
    root.append_submenu(Some(i18n::remap_cat_keyboard()), &keyboard);

    for cat in onboard::remap_categories() {
        let sub = gio::Menu::new();
        append_items(&sub, cat.items);
        root.append_submenu(Some(&i18n::remap_category_label(cat.id)), &sub);
    }

    root.append(Some(i18n::remap_factory()), Some("app.factory"));
    root.append(Some(i18n::remap_disabled()), Some("app.disable"));
    root
}

fn write_block(
    shared: &Arc<Shared>,
    table_id: u8,
    layer: u8,
    block: [u8; 7],
    toast: &adw::ToastOverlay,
    on_ok: impl FnOnce(&str) + 'static,
) {
    // Clutch byte 2 must be this button's table id (factory: Side Trigger `0C 01 39`).
    let block = if onboard::is_hypershift_block(&block) {
        onboard::hypershift_block(table_id)
    } else {
        block
    };
    let shared_bg = Arc::clone(shared);
    let toast = toast.clone();
    crate::hid_async::spawn_hid(
        move || {
            shared_bg
                .session
                .lock()
                .map_err(|e| e.to_string())
                .and_then(|mut g| {
                    g.as_mut()
                        .ok_or_else(|| "no session".into())
                        .and_then(|s| {
                            s.set_button(table_id, block, layer)
                                .map_err(|e| e.to_string())
                        })
                })
        },
        move |result| match result {
            Ok(()) => {
                let label = i18n::button_label(&onboard::describe_block(&block));
                on_ok(&label);
                toast.add_toast(adw::Toast::new(i18n::toast_button_saved()));
            }
            Err(e) => toast.add_toast(adw::Toast::new(&i18n::toast_error(&e))),
        },
    );
}

/// Menu button that writes remaps into the mouse (same residency model as Serpentine).
pub fn remap_menu_button(
    parent: &impl IsA<gtk::Window>,
    shared: &Arc<Shared>,
    toast: &adw::ToastOverlay,
    table_id: u8,
    layer: u8,
    factory: [u8; 7],
    current_block: [u8; 7],
    current_label: &str,
    row: &adw::ActionRow,
) -> gtk::MenuButton {
    let btn = gtk::MenuButton::builder()
        .label(current_label)
        .always_show_arrow(true)
        .build();
    btn.set_menu_model(Some(&build_remap_menu_model()));

    let ag = gio::SimpleActionGroup::new();
    let shared = Arc::clone(shared);
    let toast = toast.clone();
    let parent = parent.clone().upcast::<gtk::Window>();
    let btn_label = btn.clone();
    let row = row.clone();

    {
        let shared = Arc::clone(&shared);
        let toast = toast.clone();
        let btn_label = btn_label.clone();
        let row = row.clone();
        let apply = gio::SimpleAction::new("apply", Some(glib::VariantTy::STRING));
        apply.connect_activate(move |_, param| {
            let Some(token) = param.and_then(|v| v.str().map(|s| s.to_string())) else {
                return;
            };
            let Some(block) = parse_block_token(&token) else {
                return;
            };
            let btn_label = btn_label.clone();
            let row = row.clone();
            write_block(&shared, table_id, layer, block, &toast, move |label| {
                btn_label.set_label(label);
                row.set_subtitle(label);
            });
        });
        ag.add_action(&apply);
    }

    {
        let shared = Arc::clone(&shared);
        let toast = toast.clone();
        let btn_label = btn_label.clone();
        let row = row.clone();
        let disable = gio::SimpleAction::new("disable", None);
        disable.connect_activate(move |_, _| {
            let btn_label = btn_label.clone();
            let row = row.clone();
            write_block(
                &shared,
                table_id,
                layer,
                [0x00, 0x01, 0x00, 0, 0, 0, 0],
                &toast,
                move |label| {
                    btn_label.set_label(label);
                    row.set_subtitle(label);
                },
            );
        });
        ag.add_action(&disable);
    }

    {
        let shared = Arc::clone(&shared);
        let toast = toast.clone();
        let btn_label = btn_label.clone();
        let row = row.clone();
        let factory_action = gio::SimpleAction::new("factory", None);
        factory_action.connect_activate(move |_, _| {
            let btn_label = btn_label.clone();
            let row = row.clone();
            write_block(&shared, table_id, layer, factory, &toast, move |label| {
                btn_label.set_label(label);
                row.set_subtitle(label);
            });
        });
        ag.add_action(&factory_action);
    }

    {
        let shared = Arc::clone(&shared);
        let toast = toast.clone();
        let btn_label = btn_label.clone();
        let row = row.clone();
        let parent = parent.clone();
        let record = gio::SimpleAction::new("record", None);
        record.connect_activate(move |_, _| {
            open_record_dialog(
                &parent,
                Arc::clone(&shared),
                toast.clone(),
                table_id,
                layer,
                btn_label.clone(),
                row.clone(),
            );
        });
        ag.add_action(&record);
    }

    // Silence unused warning for initial current_block (label already set).
    let _ = current_block;

    btn.insert_action_group("app", Some(&ag));
    btn
}

fn open_record_dialog(
    parent: &gtk::Window,
    shared: Arc<Shared>,
    toast: adw::ToastOverlay,
    table_id: u8,
    layer: u8,
    btn: gtk::MenuButton,
    row: adw::ActionRow,
) {
    let dialog = adw::AlertDialog::builder()
        .heading(i18n::remap_record_title())
        .body(i18n::remap_record_body())
        .close_response("cancel")
        .default_response("cancel")
        .build();
    dialog.add_response("cancel", &i18n::cancel());

    let status = gtk::Label::builder()
        .label(i18n::remap_record_waiting())
        .margin_top(8)
        .build();
    status.add_css_class("dim-label");
    dialog.set_extra_child(Some(&status));

    let done = Rc::new(RefCell::new(false));
    let key_ctl = gtk::EventControllerKey::new();
    key_ctl.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let dialog = dialog.clone();
        let done = Rc::clone(&done);
        let status = status.clone();
        key_ctl.connect_key_pressed(move |_, key, _keycode, state| {
            if *done.borrow() {
                return glib::Propagation::Stop;
            }
            if key == gdk::Key::Escape {
                dialog.close();
                return glib::Propagation::Stop;
            }
            let Some((mods, usage)) = gdk_key_to_hid(key, state) else {
                status.set_label(&format!("{} ({:?})", i18n::remap_record_waiting(), key));
                return glib::Propagation::Stop;
            };
            // Ignore pure modifier presses (wait for a real key, or alone if usage==0).
            if usage == 0 && mods == 0 {
                return glib::Propagation::Stop;
            }
            *done.borrow_mut() = true;
            let block = onboard::keyboard_block(mods, usage);
            let label_preview = i18n::button_label(&onboard::describe_block(&block));
            status.set_label(&label_preview);
            let btn = btn.clone();
            let row = row.clone();
            write_block(&shared, table_id, layer, block, &toast, move |label| {
                btn.set_label(label);
                row.set_subtitle(label);
            });
            dialog.close();
            glib::Propagation::Stop
        });
    }
    dialog.add_controller(key_ctl);
    dialog.present(Some(parent));
}

/// Map a GDK key + modifiers to Razer keyboard block fields `(modifiers, hid_usage)`.
fn gdk_key_to_hid(key: gdk::Key, state: gdk::ModifierType) -> Option<(u8, u8)> {
    let mut mods = 0u8;
    if state.contains(gdk::ModifierType::CONTROL_MASK) {
        mods |= 0x01;
    }
    if state.contains(gdk::ModifierType::SHIFT_MASK) {
        mods |= 0x02;
    }
    if state.contains(gdk::ModifierType::ALT_MASK) {
        mods |= 0x04;
    }
    // Meta / Super
    if state.contains(gdk::ModifierType::SUPER_MASK) || state.contains(gdk::ModifierType::META_MASK)
    {
        mods |= 0x08;
    }

    // Bare modifiers as the key itself (Serpentine supports this).
    if matches!(
        key,
        gdk::Key::Control_L
            | gdk::Key::Control_R
            | gdk::Key::Shift_L
            | gdk::Key::Shift_R
            | gdk::Key::Alt_L
            | gdk::Key::Alt_R
            | gdk::Key::Super_L
            | gdk::Key::Super_R
            | gdk::Key::Meta_L
            | gdk::Key::Meta_R
    ) {
        let alone = match key {
            gdk::Key::Control_L | gdk::Key::Control_R => 0x01,
            gdk::Key::Shift_L | gdk::Key::Shift_R => 0x02,
            gdk::Key::Alt_L | gdk::Key::Alt_R => 0x04,
            _ => 0x08,
        };
        return Some((alone, 0));
    }

    let usage = gdk_keyval_to_usage(key)?;
    // Don't count the key's own modifier bit twice when recording Shift+A etc. — GDK
    // includes Shift in state for letter keys; strip shift for A–Z when shifted.
    if (0x04..=0x1d).contains(&usage) {
        mods &= !0x02;
    }
    Some((mods, usage))
}

fn gdk_keyval_to_usage(key: gdk::Key) -> Option<u8> {
    // Numpad first (distinct HID usages from the top row).
    if let Some(u) = match key {
        gdk::Key::KP_0 => Some(0x62),
        gdk::Key::KP_1 => Some(0x59),
        gdk::Key::KP_2 => Some(0x5a),
        gdk::Key::KP_3 => Some(0x5b),
        gdk::Key::KP_4 => Some(0x5c),
        gdk::Key::KP_5 => Some(0x5d),
        gdk::Key::KP_6 => Some(0x5e),
        gdk::Key::KP_7 => Some(0x5f),
        gdk::Key::KP_8 => Some(0x60),
        gdk::Key::KP_9 => Some(0x61),
        gdk::Key::KP_Decimal => Some(0x63),
        gdk::Key::KP_Divide => Some(0x54),
        gdk::Key::KP_Multiply => Some(0x55),
        gdk::Key::KP_Subtract => Some(0x56),
        gdk::Key::KP_Add => Some(0x57),
        gdk::Key::KP_Enter => Some(0x58),
        _ => None,
    } {
        return Some(u);
    }

    // Top-row digits via unicode (reliable across GDK keyval aliases).
    if let Some(ch) = key.to_unicode() {
        if ch == '0' {
            return Some(0x27);
        }
        if ('1'..='9').contains(&ch) {
            return Some(0x1e + (ch as u8 - b'1'));
        }
        if ch.is_ascii_lowercase() {
            return Some(0x04 + (ch as u8 - b'a'));
        }
        if ch.is_ascii_uppercase() {
            return Some(0x04 + (ch as u8 - b'A'));
        }
    }

    // F1–F24 via key name ("F1" … "F24")
    if let Some(name) = key.name() {
        if let Some(rest) = name.strip_prefix('F') {
            if let Ok(n) = rest.parse::<u8>() {
                if (1..=12).contains(&n) {
                    return Some(0x3a + (n - 1));
                }
                if (13..=24).contains(&n) {
                    return Some(0x68 + (n - 13));
                }
            }
        }
    }

    Some(match key {
        gdk::Key::Escape => 0x29,
        gdk::Key::Tab => 0x2b,
        gdk::Key::Return => 0x28,
        gdk::Key::space => 0x2c,
        gdk::Key::BackSpace => 0x2a,
        gdk::Key::Delete => 0x4c,
        gdk::Key::Home => 0x4a,
        gdk::Key::End => 0x4d,
        gdk::Key::Page_Up => 0x4b,
        gdk::Key::Page_Down => 0x4e,
        gdk::Key::Up => 0x52,
        gdk::Key::Down => 0x51,
        gdk::Key::Left => 0x50,
        gdk::Key::Right => 0x4f,
        gdk::Key::Insert => 0x49,
        gdk::Key::Print => 0x46,
        gdk::Key::Scroll_Lock => 0x47,
        gdk::Key::Pause => 0x48,
        gdk::Key::minus => 0x2d,
        gdk::Key::equal => 0x2e,
        gdk::Key::bracketleft => 0x2f,
        gdk::Key::bracketright => 0x30,
        gdk::Key::backslash => 0x31,
        gdk::Key::semicolon => 0x33,
        gdk::Key::apostrophe => 0x34,
        gdk::Key::grave => 0x35,
        gdk::Key::comma => 0x36,
        gdk::Key::period => 0x37,
        gdk::Key::slash => 0x38,
        _ => return None,
    })
}
