//! Button remaps for the Standard and HyperShift layers.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use razer_device::ButtonState;
use razer_protocol::onboard;
use synapsex_i18n as i18n;

use crate::ctx::Ctx;
use crate::remap_picker::{self, Target};

pub struct ButtonsPage {
    pub widget: adw::PreferencesPage,
    group: adw::PreferencesGroup,
    rows: RefCell<Vec<adw::ActionRow>>,
    hypershift: Cell<bool>,
}

impl ButtonsPage {
    pub fn new(ctx: &Rc<Ctx>) -> Rc<Self> {
        let widget = adw::PreferencesPage::new();

        let layers = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        layers.add_css_class("linked");
        layers.set_valign(gtk::Align::Center);
        let standard = gtk::ToggleButton::with_label(i18n::layer_standard());
        let hypershift = gtk::ToggleButton::with_label(i18n::layer_hypershift());
        hypershift.set_group(Some(&standard));
        standard.set_active(true);
        layers.append(&standard);
        layers.append(&hypershift);

        let group = adw::PreferencesGroup::builder()
            .title(i18n::section_buttons())
            .description(i18n::buttons_page_desc())
            .header_suffix(&layers)
            .build();
        widget.add(&group);

        let page = Rc::new(Self {
            widget,
            group,
            rows: RefCell::new(Vec::new()),
            hypershift: Cell::new(false),
        });

        for (btn, is_hs) in [(standard, false), (hypershift, true)] {
            let ctx = Rc::clone(ctx);
            let page_ref = Rc::downgrade(&page);
            btn.connect_toggled(move |b| {
                if !b.is_active() {
                    return;
                }
                if let Some(page) = page_ref.upgrade() {
                    page.hypershift.set(is_hs);
                    page.render(&ctx);
                }
            });
        }

        page
    }

    /// Rebuild the rows for the visible layer from `ctx.state`.
    pub fn render(self: &Rc<Self>, ctx: &Rc<Ctx>) {
        for row in self.rows.borrow_mut().drain(..) {
            self.group.remove(&row);
        }
        let hypershift = self.hypershift.get();
        let (buttons, clutch): (Vec<ButtonState>, Vec<String>) = {
            let state = ctx.state.borrow();
            let Some(st) = state.as_ref() else {
                return;
            };
            let clutch = st
                .buttons
                .iter()
                .filter(|b| onboard::is_hypershift_block(&b.block))
                .map(|b| i18n::button_control_name(b.name))
                .collect();
            let list = if hypershift {
                st.hypershift_buttons.clone()
            } else {
                st.buttons.clone()
            };
            (list, clutch)
        };

        if hypershift {
            let names = (!clutch.is_empty()).then(|| clutch.join(", "));
            self.group
                .set_description(Some(&i18n::hypershift_hint(names.as_deref())));
        } else {
            self.group.set_description(Some(i18n::buttons_page_desc()));
        }

        let mut rows = self.rows.borrow_mut();
        for b in buttons {
            let row = self.button_row(ctx, &b);
            self.group.add(&row);
            rows.push(row);
        }
    }

    fn button_row(self: &Rc<Self>, ctx: &Rc<Ctx>, b: &ButtonState) -> adw::ActionRow {
        let title = i18n::button_control_name(b.name);
        let row = adw::ActionRow::builder()
            .title(&title)
            .activatable(true)
            .build();
        let current = gtk::Label::builder()
            .label(i18n::button_label(&b.label))
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .max_width_chars(28)
            .css_classes(["dim-label"])
            .build();
        row.add_suffix(&current);
        row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));

        let factory = onboard::basilisk_v4_pro_buttons()
            .iter()
            .find(|c| c.table_id == b.table_id)
            .map(|c| c.factory)
            .unwrap_or([0x00, 0x01, 0x00, 0, 0, 0, 0]);
        let target = Target {
            title,
            table_id: b.table_id,
            layer: b.layer,
            factory,
            current: b.block,
        };

        row.connect_activated({
            let ctx = Rc::clone(ctx);
            move |_| {
                let current = current.clone();
                let ctx_cb = Rc::clone(&ctx);
                let (table_id, layer) = (target.table_id, target.layer);
                remap_picker::open(&ctx, target.clone(), move |block| {
                    let label = onboard::describe_block(&block);
                    current.set_label(&i18n::button_label(&label));
                    let mut state = ctx_cb.state.borrow_mut();
                    if let Some(st) = state.as_mut() {
                        let list = if layer == 0 {
                            &mut st.buttons
                        } else {
                            &mut st.hypershift_buttons
                        };
                        if let Some(entry) = list.iter_mut().find(|e| e.table_id == table_id) {
                            entry.block = block;
                            entry.label = label;
                        }
                    }
                });
            }
        });
        row
    }
}
