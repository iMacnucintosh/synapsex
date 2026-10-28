//! Remap picker: a fixed-size dialog with drill-down categories and search.
//!
//! Replaces the nested popover menus, whose submenus were re-placed by the
//! compositor at a different height each time the popover resized.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::gdk;
use gtk::glib;
use razer_protocol::onboard::{self, RemapItem};
use synapsex_i18n as i18n;

use crate::ctx::Ctx;

const DISABLED: [u8; 7] = [0x00, 0x01, 0x00, 0, 0, 0, 0];

#[derive(Clone)]
pub struct Target {
    pub title: String,
    pub table_id: u8,
    pub layer: u8,
    pub factory: [u8; 7],
    pub current: [u8; 7],
}

/// One leaf category of remap actions.
struct Category {
    title: String,
    note: Option<&'static str>,
    items: &'static [RemapItem],
}

fn keyboard_categories() -> Vec<Category> {
    vec![
        Category {
            title: i18n::remap_digits().into(),
            note: None,
            items: onboard::remap_digits(),
        },
        Category {
            title: i18n::remap_keypad().into(),
            note: None,
            items: onboard::remap_keypad(),
        },
        Category {
            title: i18n::remap_f_keys().into(),
            note: Some(i18n::remap_f_keys_linux_note()),
            items: onboard::remap_f_keys(),
        },
    ]
}

fn other_categories() -> Vec<Category> {
    onboard::remap_categories()
        .iter()
        .map(|c| Category {
            title: i18n::remap_category_label(c.id),
            note: None,
            items: c.items,
        })
        .collect()
}

fn same_action(a: &[u8; 7], b: &[u8; 7]) -> bool {
    a == b || (onboard::is_hypershift_block(a) && onboard::is_hypershift_block(b))
}

type Pick = Rc<dyn Fn([u8; 7])>;

pub fn open(ctx: &Rc<Ctx>, target: Target, on_applied: impl Fn([u8; 7]) + 'static) {
    let dialog = adw::Dialog::builder()
        .title(&target.title)
        .content_width(440)
        .content_height(620)
        .build();
    let nav = adw::NavigationView::new();
    dialog.set_child(Some(&nav));

    let on_applied = Rc::new(on_applied);
    let pick: Pick = {
        let ctx = Rc::clone(ctx);
        let dialog = dialog.clone();
        let target = target.clone();
        Rc::new(move |block: [u8; 7]| {
            // The clutch block carries the button's own table id (factory: `0C 01 39`).
            let block = if onboard::is_hypershift_block(&block) {
                onboard::hypershift_block(target.table_id)
            } else {
                block
            };
            dialog.set_sensitive(false);
            let (table_id, layer) = (target.table_id, target.layer);
            let dialog = dialog.clone();
            let on_applied = Rc::clone(&on_applied);
            ctx.hid_then(
                move |s| Ok(s.set_button(table_id, block, layer)?),
                move |ctx, res| match res {
                    Ok(()) => {
                        on_applied(block);
                        ctx.toast(i18n::toast_button_saved());
                        dialog.close();
                    }
                    Err(e) => {
                        dialog.set_sensitive(true);
                        ctx.toast(&i18n::toast_error(&e));
                    }
                },
            );
        })
    };

    nav.add(&root_page(ctx, &nav, &target, &pick));
    dialog.present(Some(&ctx.window));
}

fn toolbar_page(title: &str, tag: &str, content: &impl IsA<gtk::Widget>) -> adw::NavigationPage {
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(content));
    adw::NavigationPage::builder()
        .title(title)
        .tag(tag)
        .child(&toolbar)
        .build()
}

fn nav_row(title: &str, subtitle: Option<&str>) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(title)
        .activatable(true)
        .build();
    if let Some(s) = subtitle {
        row.set_subtitle(s);
    }
    row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
    row
}

fn action_row(title: &str, subtitle: Option<&str>, block: [u8; 7], current: &[u8; 7], pick: &Pick) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(title)
        .activatable(true)
        .build();
    if let Some(s) = subtitle {
        row.set_subtitle(s);
    }
    if same_action(&block, current) {
        row.add_suffix(&gtk::Image::from_icon_name("object-select-symbolic"));
    }
    let pick = Rc::clone(pick);
    row.connect_activated(move |_| pick(block));
    row
}

fn items_page(cat: &Category, current: &[u8; 7], pick: &Pick) -> adw::NavigationPage {
    let page = adw::PreferencesPage::new();
    let group = adw::PreferencesGroup::new();
    if let Some(note) = cat.note {
        group.set_description(Some(note));
    }
    for item in cat.items {
        group.add(&action_row(&i18n::media_label(item.label), None, item.block, current, pick));
    }
    page.add(&group);
    toolbar_page(&cat.title, &cat.title, &page)
}

fn push_on_activate(row: &adw::ActionRow, nav: &adw::NavigationView, make: impl Fn() -> adw::NavigationPage + 'static) {
    let nav = nav.clone();
    row.connect_activated(move |_| nav.push(&make()));
}

fn root_page(ctx: &Rc<Ctx>, nav: &adw::NavigationView, target: &Target, pick: &Pick) -> adw::NavigationPage {
    let current = target.current;

    // Browse view
    let browse = adw::PreferencesPage::new();

    let record_group = adw::PreferencesGroup::new();
    let record = adw::ActionRow::builder()
        .title(i18n::remap_record())
        .subtitle(i18n::remap_record_body())
        .activatable(true)
        .build();
    record.add_prefix(&gtk::Image::from_icon_name("input-keyboard-symbolic"));
    {
        let ctx = Rc::clone(ctx);
        let pick = Rc::clone(pick);
        record.connect_activated(move |_| {
            let pick = Rc::clone(&pick);
            open_record_dialog(&ctx.window, move |block| pick(block));
        });
    }
    record_group.add(&record);
    browse.add(&record_group);

    let cats_group = adw::PreferencesGroup::builder()
        .title(i18n::remap_categories_title())
        .build();
    let keyboard = nav_row(i18n::remap_cat_keyboard(), None);
    {
        let nav_inner = nav.clone();
        let pick = Rc::clone(pick);
        push_on_activate(&keyboard, nav, move || {
            let page = adw::PreferencesPage::new();
            let group = adw::PreferencesGroup::new();
            for cat in keyboard_categories() {
                let row = nav_row(&cat.title, None);
                let pick = Rc::clone(&pick);
                push_on_activate(&row, &nav_inner, move || items_page(&cat, &current, &pick));
                group.add(&row);
            }
            page.add(&group);
            toolbar_page(i18n::remap_cat_keyboard(), "keyboard", &page)
        });
    }
    cats_group.add(&keyboard);
    for cat in other_categories() {
        let row = nav_row(&cat.title, None);
        let pick = Rc::clone(pick);
        push_on_activate(&row, nav, move || items_page(&cat, &current, &pick));
        cats_group.add(&row);
    }
    browse.add(&cats_group);

    let reset_group = adw::PreferencesGroup::new();
    let factory_label = i18n::button_label(&onboard::describe_block(&target.factory));
    reset_group.add(&action_row(
        i18n::remap_factory(),
        Some(&factory_label),
        target.factory,
        &current,
        pick,
    ));
    reset_group.add(&action_row(i18n::remap_disabled(), None, DISABLED, &current, pick));
    browse.add(&reset_group);

    // Search results view
    let results = adw::PreferencesPage::new();
    let results_group = adw::PreferencesGroup::new();
    results.add(&results_group);
    let empty = adw::StatusPage::builder()
        .icon_name("edit-find-symbolic")
        .title(i18n::remap_no_results())
        .build();

    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.add_named(&browse, Some("browse"));
    stack.add_named(&results, Some("results"));
    stack.add_named(&empty, Some("empty"));

    let search = gtk::SearchEntry::builder()
        .placeholder_text(i18n::remap_search_placeholder())
        .hexpand(true)
        .build();
    let search_bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    search_bar.set_margin_start(12);
    search_bar.set_margin_end(12);
    search_bar.set_margin_bottom(6);
    search_bar.append(&search);

    let index: Vec<(String, String, [u8; 7])> = keyboard_categories()
        .into_iter()
        .chain(other_categories())
        .flat_map(|cat| {
            cat.items
                .iter()
                .map(move |it| (i18n::media_label(it.label), cat.title.clone(), it.block))
                .collect::<Vec<_>>()
        })
        .collect();
    let shown: Rc<RefCell<Vec<adw::ActionRow>>> = Rc::default();
    {
        let pick = Rc::clone(pick);
        let stack = stack.clone();
        search.connect_search_changed(move |entry| {
            for row in shown.borrow_mut().drain(..) {
                results_group.remove(&row);
            }
            let query = entry.text().trim().to_lowercase();
            if query.is_empty() {
                stack.set_visible_child_name("browse");
                return;
            }
            let mut rows = shown.borrow_mut();
            for (label, cat, block) in index
                .iter()
                .filter(|(label, cat, _)| {
                    label.to_lowercase().contains(&query) || cat.to_lowercase().contains(&query)
                })
                .take(60)
            {
                let row = action_row(label, Some(cat), *block, &current, &pick);
                results_group.add(&row);
                rows.push(row);
            }
            stack.set_visible_child_name(if rows.is_empty() { "empty" } else { "results" });
        });
    }

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.add_top_bar(&search_bar);
    toolbar.set_content(Some(&stack));
    adw::NavigationPage::builder()
        .title(i18n::remap_picker_title())
        .tag("root")
        .child(&toolbar)
        .build()
}

fn open_record_dialog(parent: &impl IsA<gtk::Widget>, on_block: impl Fn([u8; 7]) + 'static) {
    let dialog = adw::AlertDialog::builder()
        .heading(i18n::remap_record_title())
        .body(i18n::remap_record_body())
        .close_response("cancel")
        .default_response("cancel")
        .build();
    dialog.add_response("cancel", i18n::cancel());

    let status = gtk::Label::builder()
        .label(i18n::remap_record_waiting())
        .margin_top(8)
        .css_classes(["dim-label"])
        .build();
    dialog.set_extra_child(Some(&status));

    let done = Rc::new(std::cell::Cell::new(false));
    let key_ctl = gtk::EventControllerKey::new();
    key_ctl.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let dialog = dialog.clone();
        key_ctl.connect_key_pressed(move |_, key, _keycode, state| {
            if done.get() {
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
            if usage == 0 && mods == 0 {
                return glib::Propagation::Stop;
            }
            done.set(true);
            dialog.close();
            on_block(onboard::keyboard_block(mods, usage));
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
