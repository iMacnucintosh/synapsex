//! Scroll wheel and power settings (hidden from the shell for now).

#![allow(dead_code)]

use std::rc::Rc;

use adw::prelude::*;
use razer_device::MouseState;
use synapsex_i18n as i18n;

use super::apply_button;
use crate::ctx::Ctx;

pub struct MorePage {
    pub widget: adw::PreferencesPage,
    smart_reel: adw::SwitchRow,
    scroll_accel: adw::SwitchRow,
    idle: adw::SpinRow,
    low_battery: adw::SpinRow,
}

impl MorePage {
    pub fn new(ctx: &Rc<Ctx>) -> Rc<Self> {
        let widget = adw::PreferencesPage::new();

        let wheel_apply = apply_button(i18n::apply());
        let wheel_group = adw::PreferencesGroup::builder()
            .title(i18n::section_wheel())
            .description(i18n::wheel_group_desc())
            .header_suffix(&wheel_apply)
            .build();
        let smart_reel = adw::SwitchRow::builder().title(i18n::smart_reel()).build();
        let scroll_accel = adw::SwitchRow::builder()
            .title(i18n::scroll_accel())
            .build();
        wheel_group.add(&smart_reel);
        wheel_group.add(&scroll_accel);
        widget.add(&wheel_group);

        let power_apply = apply_button(i18n::apply());
        let power_group = adw::PreferencesGroup::builder()
            .title(i18n::section_power())
            .description(i18n::power_group_desc())
            .header_suffix(&power_apply)
            .build();
        let idle = adw::SpinRow::with_range(60.0, 900.0, 30.0);
        idle.set_title(i18n::sleep_seconds());
        let low_battery = adw::SpinRow::with_range(5.0, 50.0, 1.0);
        low_battery.set_title(i18n::low_battery_pct());
        power_group.add(&idle);
        power_group.add(&low_battery);
        widget.add(&power_group);

        let page = Rc::new(Self {
            widget,
            smart_reel,
            scroll_accel,
            idle,
            low_battery,
        });

        wheel_apply.connect_clicked({
            let ctx = Rc::clone(ctx);
            let page = Rc::clone(&page);
            move |_| {
                let sr = page.smart_reel.is_active();
                let sa = page.scroll_accel.is_active();
                ctx.write_and_refresh(i18n::toast_wheel_saved(), move |s| {
                    s.set_smart_reel(sr)?;
                    Ok(s.set_scroll_accel(sa)?)
                });
            }
        });

        power_apply.connect_clicked({
            let ctx = Rc::clone(ctx);
            let page = Rc::clone(&page);
            move |_| {
                let idle = page.idle.value() as u16;
                let low = page.low_battery.value() as u8;
                ctx.write_and_refresh(i18n::toast_power_saved(), move |s| {
                    s.set_idle(idle)?;
                    Ok(s.set_low_battery(low)?)
                });
            }
        });

        page
    }

    pub fn apply(&self, st: &MouseState) {
        self.smart_reel.set_active(st.smart_reel);
        self.scroll_accel.set_active(st.scroll_accel);
        self.idle.set_value(f64::from(st.idle_seconds));
        self.low_battery.set_value(f64::from(st.low_battery_percent));
    }
}
