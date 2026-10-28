//! DPI, DPI stages and polling rate.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use razer_device::MouseState;
use razer_protocol::MAX_DPI_STAGES;
use synapsex_i18n as i18n;

use super::{apply_button, string_combo};
use crate::ctx::Ctx;

const POLL_RATES: [u32; 7] = [125, 250, 500, 1000, 2000, 4000, 8000];
const DPI_MIN: f64 = 100.0;
const DPI_MAX: f64 = 50_000.0;
const DPI_STEP: f64 = 50.0;

struct StageRow {
    spin: adw::SpinRow,
    active: gtk::CheckButton,
}

pub struct PerformancePage {
    pub widget: adw::PreferencesPage,
    dpi: adw::SpinRow,
    stages_group: adw::PreferencesGroup,
    stages: RefCell<Vec<StageRow>>,
    add_stage: gtk::Button,
    remove_stage: gtk::Button,
    poll: adw::ComboRow,
}

impl PerformancePage {
    pub fn new(ctx: &Rc<Ctx>) -> Rc<Self> {
        let widget = adw::PreferencesPage::new();

        let dpi_apply = apply_button(i18n::apply());
        let dpi_group = adw::PreferencesGroup::builder()
            .title(i18n::section_dpi())
            .header_suffix(&dpi_apply)
            .build();
        let dpi = adw::SpinRow::with_range(DPI_MIN, DPI_MAX, DPI_STEP);
        dpi.set_title(i18n::dpi_current_title());
        dpi.set_numeric(true);
        dpi_group.add(&dpi);
        widget.add(&dpi_group);

        let add_stage = gtk::Button::from_icon_name("list-add-symbolic");
        add_stage.set_tooltip_text(Some(i18n::add_dpi_stage()));
        add_stage.add_css_class("flat");
        add_stage.set_valign(gtk::Align::Center);
        let remove_stage = gtk::Button::from_icon_name("list-remove-symbolic");
        remove_stage.set_tooltip_text(Some(i18n::remove_dpi_stage()));
        remove_stage.add_css_class("flat");
        remove_stage.set_valign(gtk::Align::Center);
        let stages_apply = apply_button(i18n::apply());
        let stages_tools = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        stages_tools.append(&remove_stage);
        stages_tools.append(&add_stage);
        stages_tools.append(&stages_apply);

        let stages_group = adw::PreferencesGroup::builder()
            .title(i18n::dpi_stages_title())
            .description(i18n::dpi_stages_desc())
            .header_suffix(&stages_tools)
            .build();
        widget.add(&stages_group);

        let poll_apply = apply_button(i18n::apply());
        let poll_group = adw::PreferencesGroup::builder()
            .title(i18n::section_poll())
            .header_suffix(&poll_apply)
            .build();
        let labels: Vec<String> = POLL_RATES.iter().map(|hz| format!("{hz} Hz")).collect();
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        let poll = string_combo(i18n::section_poll(), &labels);
        poll_group.add(&poll);
        widget.add(&poll_group);

        let page = Rc::new(Self {
            widget,
            dpi,
            stages_group,
            stages: RefCell::new(Vec::new()),
            add_stage,
            remove_stage,
            poll,
        });

        dpi_apply.connect_clicked({
            let ctx = Rc::clone(ctx);
            let page = Rc::clone(&page);
            move |_| {
                let dpi = page.dpi.value() as u16;
                ctx.write_and_refresh(i18n::toast_dpi_written(), move |s| {
                    Ok(s.set_dpi(dpi, dpi)?)
                });
            }
        });

        stages_apply.connect_clicked({
            let ctx = Rc::clone(ctx);
            let page = Rc::clone(&page);
            move |_| {
                let (stages, active) = page.collect_stages();
                if stages.is_empty() {
                    return;
                }
                ctx.write_and_refresh(i18n::toast_stages_written(), move |s| {
                    Ok(s.set_dpi_stages(&stages, active)?)
                });
            }
        });

        page.add_stage.connect_clicked({
            let page = Rc::clone(&page);
            move |_| page.push_stage(None)
        });
        page.remove_stage.connect_clicked({
            let page = Rc::clone(&page);
            move |_| page.pop_stage()
        });

        poll_apply.connect_clicked({
            let ctx = Rc::clone(ctx);
            let page = Rc::clone(&page);
            move |_| {
                let hz = POLL_RATES
                    .get(page.poll.selected() as usize)
                    .copied()
                    .unwrap_or(1000);
                ctx.hid(
                    move |s| {
                        s.set_poll(hz)?;
                        Ok(s.read_poll_hz())
                    },
                    |ctx, readback| {
                        if readback == 0 {
                            ctx.toast(i18n::toast_poll_written());
                        } else {
                            ctx.toast(&i18n::toast_poll_confirmed(readback));
                        }
                        ctx.refresh();
                    },
                );
            }
        });

        page
    }

    fn collect_stages(&self) -> (Vec<u16>, usize) {
        let rows = self.stages.borrow();
        let stages: Vec<u16> = rows.iter().map(|r| r.spin.value() as u16).collect();
        let active = rows
            .iter()
            .position(|r| r.active.is_active())
            .unwrap_or(0)
            .min(stages.len().saturating_sub(1));
        (stages, active)
    }

    fn retitle_stages(&self) {
        let rows = self.stages.borrow();
        for (i, row) in rows.iter().enumerate() {
            row.spin.set_title(&i18n::dpi_stage_label(i + 1));
        }
        let n = rows.len();
        self.add_stage.set_sensitive(n < MAX_DPI_STAGES);
        self.remove_stage.set_sensitive(n > 1);
    }

    fn push_stage(&self, value: Option<u16>) {
        if self.stages.borrow().len() >= MAX_DPI_STAGES {
            return;
        }
        let fallback = self
            .stages
            .borrow()
            .last()
            .map(|r| (r.spin.value() as u16).saturating_add(800).min(30_000))
            .unwrap_or(800);
        let dpi = value.unwrap_or(fallback);
        let spin = adw::SpinRow::with_range(DPI_MIN, DPI_MAX, DPI_STEP);
        spin.set_numeric(true);
        spin.set_value(f64::from(dpi));

        let active = gtk::CheckButton::builder()
            .valign(gtk::Align::Center)
            .tooltip_text(i18n::dpi_stage_in_use())
            .build();
        if let Some(first) = self.stages.borrow().first() {
            active.set_group(Some(&first.active));
        }
        if self.stages.borrow().is_empty() {
            active.set_active(true);
        }
        spin.add_suffix(&active);
        self.stages_group.add(&spin);
        self.stages.borrow_mut().push(StageRow { spin, active });
        self.retitle_stages();
        self.update_dpi_subtitle();
    }

    fn pop_stage(&self) {
        let mut rows = self.stages.borrow_mut();
        if rows.len() <= 1 {
            return;
        }
        let row = rows.pop().unwrap();
        if row.active.is_active() {
            if let Some(prev) = rows.last() {
                prev.active.set_active(true);
            }
        }
        self.stages_group.remove(&row.spin);
        drop(rows);
        self.retitle_stages();
        self.update_dpi_subtitle();
    }

    fn update_dpi_subtitle(&self) {
        let rows = self.stages.borrow();
        let count = rows.len();
        let idx = rows.iter().position(|r| r.active.is_active()).unwrap_or(0);
        if count == 0 {
            self.dpi.set_subtitle("");
        } else {
            self.dpi.set_subtitle(&i18n::dpi_stage_of(idx + 1, count));
        }
    }

    pub fn apply(&self, st: &MouseState) {
        self.show_live_dpi(st.dpi_x, st.active_dpi_stage, st.dpi_stages.len());
        {
            let mut rows = self.stages.borrow_mut();
            for row in rows.drain(..) {
                self.stages_group.remove(&row.spin);
            }
        }
        let stages = if st.dpi_stages.is_empty() {
            vec![st.dpi_x.max(100)]
        } else {
            st.dpi_stages.clone()
        };
        for (i, &dpi) in stages.iter().take(MAX_DPI_STAGES).enumerate() {
            self.push_stage(Some(dpi));
            if i == st.active_dpi_stage {
                if let Some(row) = self.stages.borrow().get(i) {
                    row.active.set_active(true);
                }
            }
        }
        if let Some(idx) = POLL_RATES.iter().position(|&r| r == st.poll_hz) {
            self.poll.set_subtitle("");
            self.poll.set_selected(idx as u32);
        } else if st.poll_hz == 0 {
            self.poll.set_subtitle(i18n::poll_unknown());
        }
        self.update_dpi_subtitle();
    }

    /// Update the DPI row from the live DPI-button poll.
    pub fn show_live_dpi(&self, dpi: u16, stage: usize, count: usize) {
        self.dpi.set_value(f64::from(dpi));
        if count == 0 {
            self.dpi.set_subtitle("");
        } else {
            self.dpi.set_subtitle(&i18n::dpi_stage_of(stage + 1, count));
        }
        let rows = self.stages.borrow();
        if let Some(row) = rows.get(stage) {
            row.active.set_active(true);
        }
    }
}
