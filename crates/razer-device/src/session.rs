use std::collections::HashMap;

use razer_protocol::{
    commands, devices, lighting, onboard, LightingEffect, LightingZone, RazerReport, Rgb,
};

use crate::config::AppConfig;
use crate::discover::{self, FoundDevice};
use crate::hidraw::{HidDevice, HidError};

#[derive(Clone, Debug)]
pub struct ZoneState {
    pub zone: LightingZone,
    pub effect: LightingEffect,
    pub color: Rgb,
    pub brightness: u8,
}

#[derive(Clone, Debug)]
pub struct ButtonState {
    pub name: &'static str,
    pub table_id: u8,
    /// `0` = normal, `1` = HyperShift (held).
    pub layer: u8,
    pub block: [u8; 7],
    pub label: String,
}

/// Full onboard configuration for one profile slot, read from the mouse.
#[derive(Clone, Debug)]
pub struct SlotProfile {
    pub slot: u8,
    pub name: String,
    pub is_active: bool,
    pub dpi_x: u16,
    pub dpi_y: u16,
    pub dpi_stages: Vec<u16>,
    pub active_dpi_stage: usize,
    pub smart_reel: bool,
    pub scroll_accel: bool,
    pub smart_reel_level: u8,
    pub scroll_accel_level: u8,
    pub wheel_mode: u8,
    pub zones: Vec<ZoneState>,
    /// Layer 0 (normal press) remaps from hardware.
    pub buttons: Vec<ButtonState>,
    /// Layer 1 (HyperShift held) remaps from hardware.
    pub hypershift_buttons: Vec<ButtonState>,
}

#[derive(Clone, Debug)]
pub struct MouseState {
    pub pid: u16,
    pub name: String,
    pub connection: String,
    pub serial: String,
    pub firmware: String,
    pub battery_percent: u8,
    pub charging: bool,
    pub dpi_x: u16,
    pub dpi_y: u16,
    pub dpi_stages: Vec<u16>,
    pub active_dpi_stage: usize,
    pub poll_hz: u32,
    pub idle_seconds: u16,
    pub low_battery_percent: u8,
    /// Wireless motion dimming (Synapse "Smart Dimming").
    pub smart_dimming: bool,
    pub smart_reel: bool,
    pub scroll_accel: bool,
    pub smart_reel_level: u8,
    pub scroll_accel_level: u8,
    pub wheel_mode: u8,
    pub active_slot: u8,
    pub slots: Vec<u8>,
    pub slot_name: String,
    pub zones: Vec<ZoneState>,
    pub buttons: Vec<ButtonState>,
    pub hypershift_buttons: Vec<ButtonState>,
    /// Onboard profiles known to exist. Only the active slot is guaranteed fully
    /// populated; inactive entries may be stubs (name/slot only) from cache.
    pub profiles: Vec<SlotProfile>,
}

pub struct DeviceSession {
    pub found: FoundDevice,
    pub hid: HidDevice,
    pub active_slot: u8,
    pub serial: String,
    /// Full profiles keyed by slot — filled on demand, invalidated on writes.
    profile_cache: HashMap<u8, SlotProfile>,
}

impl DeviceSession {
    pub fn open() -> Result<Self, HidError> {
        let (found, hid) = discover::open_first()?;
        let mut session = Self {
            found,
            hid,
            active_slot: 1,
            serial: String::new(),
            profile_cache: HashMap::new(),
        };
        session.refresh_slot_and_serial()?;
        Ok(session)
    }

    /// Drop cached profile(s). `None` clears every slot.
    pub fn invalidate_profile_cache(&mut self, slot: Option<u8>) {
        match slot {
            Some(s) => {
                self.profile_cache.remove(&s);
            }
            None => self.profile_cache.clear(),
        }
    }

    fn send_to_slot(&self, slot: u8, report: RazerReport) -> Result<RazerReport, HidError> {
        let report = onboard::address_to_slot(&report, slot);
        self.hid.send(report)
    }

    fn send_slotted(&self, report: RazerReport) -> Result<RazerReport, HidError> {
        self.send_to_slot(self.active_slot, report)
    }

    fn refresh_slot_and_serial(&mut self) -> Result<(), HidError> {
        if let Ok(resp) = self.hid.send(commands::get_serial()) {
            if let Some(s) = commands::parse_serial(&resp) {
                self.serial = s;
            }
        }
        if self.serial.is_empty() {
            self.serial = self
                .found
                .serial_hint
                .clone()
                .unwrap_or_else(|| format!("pid{:04x}", self.found.pid));
        }
        if let Ok(resp) = self.hid.send(onboard::get_active_slot()) {
            if let Some(slot) = onboard::parse_active_slot(&resp) {
                self.active_slot = slot;
            }
        }
        Ok(())
    }

    /// Poll active slot; returns true if it changed (caller should rehydrate UI).
    pub fn poll_slot_change(&mut self) -> Result<bool, HidError> {
        let resp = self.hid.send(onboard::get_active_slot())?;
        let Some(slot) = onboard::parse_active_slot(&resp) else {
            return Ok(false);
        };
        if slot != self.active_slot {
            self.active_slot = slot;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Lightweight read of live DPI + active stage (for following the DPI button).
    pub fn peek_dpi(&self) -> Result<(u16 /*x*/, usize /*stage*/, usize /*count*/), HidError> {
        let count_from_one = self.found.info.dpi_stages_count_from_one;
        let (mut x, _) = self
            .send_slotted(commands::get_dpi())
            .map(|r| commands::parse_dpi(&r))
            .unwrap_or((0, 0));
        let stages_resp = self.send_slotted(commands::get_dpi_stages())?;
        let stages = commands::parse_dpi_stages(&stages_resp);
        let mut stage = commands::parse_active_dpi_stage(&stages_resp, count_from_one);
        if !stages.is_empty() {
            stage = stage.min(stages.len() - 1);
        } else {
            stage = 0;
        }
        if x == 0 || (!stages.is_empty() && !stages.contains(&x)) {
            if let Some(&d) = stages.get(stage) {
                x = d;
            }
        }
        Ok((x, stage, stages.len()))
    }

    /// Returns `Some((dpi, stage, count))` when the DPI button (or anything) changed live DPI.
    pub fn poll_dpi_change(
        &self,
        last_dpi: u16,
        last_stage: usize,
    ) -> Result<Option<(u16, usize, usize)>, HidError> {
        let (dpi, stage, count) = self.peek_dpi()?;
        if dpi == 0 {
            return Ok(None);
        }
        if dpi != last_dpi || stage != last_stage {
            Ok(Some((dpi, stage, count)))
        } else {
            Ok(None)
        }
    }

    pub fn list_existing_slots(&self) -> Result<Vec<u8>, HidError> {
        let resp = self.hid.send(onboard::list_slots())?;
        Ok(onboard::parse_slots(&resp).unwrap_or_else(|| vec![1]))
    }

    /// Read one onboard profile’s settings from the mouse without activating it.
    pub fn read_slot_profile(&self, slot: u8) -> Result<SlotProfile, HidError> {
        let info = self.found.info;
        let cfg = AppConfig::load();
        let count_from_one = info.dpi_stages_count_from_one;

        let (mut dpi_x, mut dpi_y) = self
            .send_to_slot(slot, commands::get_dpi())
            .map(|r| commands::parse_dpi(&r))
            .unwrap_or((0, 0));
        // Dongle can return a successful empty DPI frame under load — retry once.
        if dpi_x == 0 && dpi_y == 0 {
            if let Ok(r) = self.send_to_slot(slot, commands::get_dpi()) {
                let (x, y) = commands::parse_dpi(&r);
                dpi_x = x;
                dpi_y = y;
            }
        }

        let (dpi_stages, mut active_dpi_stage) =
            match self.send_to_slot(slot, commands::get_dpi_stages()) {
                Ok(stages_resp) => {
                    let stages = commands::parse_dpi_stages(&stages_resp);
                    let active = commands::parse_active_dpi_stage(&stages_resp, count_from_one);
                    if stages.is_empty() {
                        // Same flaky-empty response pattern as get_dpi.
                        if let Ok(again) = self.send_to_slot(slot, commands::get_dpi_stages()) {
                            (
                                commands::parse_dpi_stages(&again),
                                commands::parse_active_dpi_stage(&again, count_from_one),
                            )
                        } else {
                            (stages, active)
                        }
                    } else {
                        (stages, active)
                    }
                }
                Err(_) => (Vec::new(), 0),
            };
        if !dpi_stages.is_empty() {
            active_dpi_stage = active_dpi_stage.min(dpi_stages.len() - 1);
        } else {
            active_dpi_stage = 0;
        }

        // Prefer a DPI that belongs to the stage table; otherwise fall back to the active preset.
        let dpi_in_stages = !dpi_stages.is_empty() && dpi_stages.contains(&dpi_x);
        if dpi_x == 0 || !dpi_stages.is_empty() && !dpi_in_stages {
            if let Ok(r) = self.send_to_slot(slot, commands::get_dpi()) {
                let (x, y) = commands::parse_dpi(&r);
                if x != 0 && (dpi_stages.is_empty() || dpi_stages.contains(&x)) {
                    dpi_x = x;
                    dpi_y = y;
                }
            }
        }
        if dpi_x == 0 || (!dpi_stages.is_empty() && !dpi_stages.contains(&dpi_x)) {
            if let Some(&d) = dpi_stages.get(active_dpi_stage) {
                dpi_x = d;
                dpi_y = d;
            }
        }

        let smart_reel = self
            .send_to_slot(slot, commands::get_scroll_smart_reel())
            .map(|r| commands::parse_flag(&r))
            .unwrap_or(false);
        let scroll_accel = self
            .send_to_slot(slot, commands::get_scroll_acceleration())
            .map(|r| commands::parse_flag(&r))
            .unwrap_or(false);
        let smart_reel_level = self
            .send_to_slot(slot, commands::get_smart_reel_sensitivity())
            .map(|r| commands::parse_level(&r))
            .unwrap_or(0);
        let scroll_accel_level = self
            .send_to_slot(slot, commands::get_scroll_acceleration_level())
            .map(|r| commands::parse_level(&r))
            .unwrap_or(0);
        let wheel_mode = self
            .send_to_slot(slot, commands::get_wheel_mode())
            .map(|r| r.arguments[1])
            .unwrap_or(0);

        let mut zones = Vec::new();
        for &zone in info.lighting_zones {
            let (effect, color) = self
                .send_to_slot(slot, lighting::get_zone_effect(zone))
                .ok()
                .and_then(|r| lighting::parse_zone_effect(&r))
                .unwrap_or((LightingEffect::Off, None));
            let brightness = self
                .send_to_slot(slot, lighting::get_brightness(zone.as_u8()))
                .map(|r| lighting::brightness_percent(r.arguments[2]))
                .unwrap_or(0);
            zones.push(ZoneState {
                zone,
                effect,
                color: color.unwrap_or(Rgb::new(255, 0, 0)),
                brightness,
            });
        }

        let buttons = self.read_buttons_layer(slot, onboard::LAYER_NORMAL)?;
        let hypershift_buttons = self.read_buttons_layer(slot, onboard::LAYER_HYPERSHIFT)?;

        Ok(SlotProfile {
            slot,
            name: cfg.name_for(&self.serial, slot),
            is_active: slot == self.active_slot,
            dpi_x,
            dpi_y,
            dpi_stages,
            active_dpi_stage,
            smart_reel,
            scroll_accel,
            smart_reel_level,
            scroll_accel_level,
            wheel_mode,
            zones,
            buttons,
            hypershift_buttons,
        })
    }

    fn read_buttons_layer(&self, slot: u8, layer: u8) -> Result<Vec<ButtonState>, HidError> {
        let mut buttons = Vec::new();
        for ctrl in onboard::basilisk_v4_pro_buttons() {
            let block = self.read_button_block(slot, ctrl.table_id, layer).unwrap_or(
                if layer == onboard::LAYER_NORMAL {
                    ctrl.factory
                } else {
                    // Prefer Disabled over inventing a HyperShift bind when reads fail.
                    [0x00, 0x01, 0x00, 0, 0, 0, 0]
                },
            );
            let label = onboard::describe_block(&block);
            buttons.push(ButtonState {
                name: ctrl.name,
                table_id: ctrl.table_id,
                layer,
                block,
                label,
            });
        }
        Ok(buttons)
    }

    fn read_button_block(&self, slot: u8, table_id: u8, layer: u8) -> Option<[u8; 7]> {
        for attempt in 0..6 {
            if attempt > 0 {
                std::thread::sleep(std::time::Duration::from_millis(40));
            }
            if let Ok(r) = self.send_to_slot(slot, commands::get_button_function(table_id, layer)) {
                if let Some(block) = commands::parse_button_function(&r, table_id, layer) {
                    return Some(block);
                }
            }
        }
        None
    }

    /// Read every existing onboard profile from the mouse (slow — CLI dumps).
    pub fn read_all_profiles(&mut self) -> Result<Vec<SlotProfile>, HidError> {
        let slots = self.list_existing_slots()?;
        let mut profiles = Vec::with_capacity(slots.len());
        for slot in slots {
            let p = self.read_slot_profile(slot)?;
            self.profile_cache.insert(slot, p.clone());
            profiles.push(p);
        }
        Ok(profiles)
    }

    /// Stub entry for the profile dropdown (name/slot only — no HID button traffic).
    fn profile_stub(&self, slot: u8) -> SlotProfile {
        let cfg = AppConfig::load();
        SlotProfile {
            slot,
            name: cfg.name_for(&self.serial, slot),
            is_active: slot == self.active_slot,
            dpi_x: 0,
            dpi_y: 0,
            dpi_stages: Vec::new(),
            active_dpi_stage: 0,
            smart_reel: false,
            scroll_accel: false,
            smart_reel_level: 0,
            scroll_accel_level: 0,
            wheel_mode: 0,
            zones: Vec::new(),
            buttons: Vec::new(),
            hypershift_buttons: Vec::new(),
        }
    }

    /// Read active slot fully; other slots from cache or lightweight stubs.
    pub fn read_state(&mut self) -> Result<MouseState, HidError> {
        self.refresh_slot_and_serial()?;
        let cfg = AppConfig::load();
        let info = self.found.info;

        let firmware = self
            .hid
            .send(commands::get_firmware_version())
            .map(|r| commands::parse_firmware(&r))
            .unwrap_or_else(|_| "?".into());

        let battery_percent = self
            .hid
            .send(commands::get_battery_level())
            .map(|r| commands::battery_percent(r.arguments[1]))
            .unwrap_or(0);

        let charging = self
            .hid
            .send(commands::get_charging_status())
            .map(|r| r.arguments[1] != 0)
            .unwrap_or(false);

        let poll_hz = self.read_poll_hz();

        let idle_seconds = self
            .hid
            .send(commands::get_idle_time())
            .map(|r| commands::parse_idle_time(&r))
            .unwrap_or(0);

        let low_battery_percent = self
            .hid
            .send(commands::get_low_battery_threshold())
            .map(|r| commands::parse_low_battery_threshold(&r))
            .unwrap_or(0);

        let slots = self.list_existing_slots()?;
        let active_slot = self.active_slot;
        // Only the active profile hits the wire for buttons/DPI/lights.
        let active = self.read_slot_profile(active_slot)?;
        self.profile_cache.insert(active_slot, active.clone());

        let mut profiles = Vec::with_capacity(slots.len());
        for slot in &slots {
            if *slot == active_slot {
                profiles.push(active.clone());
                continue;
            }
            if let Some(cached) = self.profile_cache.get(slot) {
                let mut p = cached.clone();
                p.is_active = false;
                p.name = cfg.name_for(&self.serial, *slot);
                profiles.push(p);
            } else {
                profiles.push(self.profile_stub(*slot));
            }
        }

        // After onboard traffic the dongle often returns stale DPI frames; read last with retries.
        let smart_dimming = self.read_smart_dimming();

        let connection = match info.connection {
            devices::Connection::Wired => "Wired",
            devices::Connection::WirelessDongle => "2.4 GHz",
            devices::Connection::Bluetooth => "Bluetooth",
        }
        .to_string();

        Ok(MouseState {
            pid: self.found.pid,
            name: info.name.to_string(),
            connection,
            serial: self.serial.clone(),
            firmware,
            battery_percent,
            charging,
            dpi_x: active.dpi_x,
            dpi_y: active.dpi_y,
            dpi_stages: active.dpi_stages.clone(),
            active_dpi_stage: active.active_dpi_stage,
            poll_hz,
            idle_seconds,
            low_battery_percent,
            smart_dimming,
            smart_reel: active.smart_reel,
            scroll_accel: active.scroll_accel,
            smart_reel_level: active.smart_reel_level,
            scroll_accel_level: active.scroll_accel_level,
            wheel_mode: active.wheel_mode,
            active_slot,
            slots,
            slot_name: cfg.name_for(&self.serial, active_slot),
            zones: active.zones.clone(),
            buttons: active.buttons.clone(),
            hypershift_buttons: active.hypershift_buttons.clone(),
            profiles,
        })
    }

    fn write_dpi_xy(&self, x: u16, y: u16) -> Result<(), HidError> {
        self.send_slotted(commands::set_dpi(x, y))?;
        Ok(())
    }

    /// Set live sensor DPI. If `x` matches an onboard stage, that stage is marked active.
    pub fn set_dpi(&mut self, x: u16, y: u16) -> Result<(), HidError> {
        self.write_dpi_xy(x, y)?;
        if let Ok(resp) = self.send_slotted(commands::get_dpi_stages()) {
            let count_from_one = self.found.info.dpi_stages_count_from_one;
            let stages = commands::parse_dpi_stages(&resp);
            if let Some(idx) = stages.iter().position(|&s| s == x) {
                self.send_slotted(commands::set_dpi_stages(
                    &stages,
                    idx,
                    count_from_one,
                ))?;
            }
        }
        self.invalidate_profile_cache(Some(self.active_slot));
        Ok(())
    }

    /// Write the onboard DPI stage table and apply the active stage as the live sensor DPI.
    pub fn set_dpi_stages(&mut self, stages: &[u16], active: usize) -> Result<(), HidError> {
        if stages.is_empty() {
            return Ok(());
        }
        let count_from_one = self.found.info.dpi_stages_count_from_one;
        let active = active.min(stages.len() - 1);
        self.send_slotted(commands::set_dpi_stages(stages, active, count_from_one))?;
        // Stages alone are the cycle list; Synapse also pushes XY so the sensor matches now.
        self.write_dpi_xy(stages[active], stages[active])?;
        self.invalidate_profile_cache(Some(self.active_slot));
        Ok(())
    }

    /// Read polling rate with retries — HyperSpeed often returns a stale non-`00 xx` frame.
    pub fn read_poll_hz(&self) -> u32 {
        let info = self.found.info;
        for attempt in 0..5 {
            if attempt > 0 {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            let hz = if info.uses_polling_rate2 {
                self.hid
                    .send(commands::get_polling_rate2())
                    .map(|r| commands::parse_polling_rate2(&r))
                    .unwrap_or(0)
            } else {
                self.hid
                    .send(commands::get_polling_rate())
                    .map(|r| commands::parse_polling_rate(&r))
                    .unwrap_or(0)
            };
            if hz != 0 {
                return hz;
            }
        }
        0
    }

    /// Read Smart Dimming with retries — under profile load the dongle often
    /// returns a leftover DPI frame whose `args[2]` looks like "on".
    pub fn read_smart_dimming(&self) -> bool {
        for attempt in 0..5 {
            if attempt > 0 {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            if let Ok(r) = self.hid.send(commands::get_smart_dimming()) {
                if let Some(on) = commands::parse_smart_dimming(&r) {
                    return on;
                }
            }
        }
        false
    }

    pub fn set_poll(&self, hz: u32) -> Result<(), HidError> {
        let reports = commands::set_polling_rate_reports(hz, self.found.pid);
        for (i, report) in reports.into_iter().enumerate() {
            if i > 0 {
                // Dongle needs a beat between mouse (0x00) and receiver (0x01) writes.
                std::thread::sleep(std::time::Duration::from_millis(80));
            }
            self.hid.send(report)?;
        }
        // Firmware applies asynchronously over HyperSpeed.
        std::thread::sleep(std::time::Duration::from_millis(200));
        Ok(())
    }

    pub fn set_zone_lighting(
        &mut self,
        zone: LightingZone,
        effect: LightingEffect,
        color: Rgb,
        brightness_pct: u8,
    ) -> Result<(), HidError> {
        self.send_slotted(lighting::set_zone_effect(zone, effect, color))?;
        self.send_slotted(lighting::set_brightness(
            zone.as_u8(),
            lighting::brightness_raw(brightness_pct),
        ))?;
        self.invalidate_profile_cache(Some(self.active_slot));
        Ok(())
    }

    pub fn set_all_zones(
        &mut self,
        effect: LightingEffect,
        color: Rgb,
        brightness_pct: u8,
    ) -> Result<(), HidError> {
        for &zone in self.found.info.lighting_zones {
            self.set_zone_lighting(zone, effect, color, brightness_pct)?;
        }
        Ok(())
    }

    pub fn set_idle(&self, seconds: u16) -> Result<(), HidError> {
        self.hid.send(commands::set_idle_time(seconds))?;
        Ok(())
    }

    pub fn set_low_battery(&self, percent: u8) -> Result<(), HidError> {
        self.hid
            .send(commands::set_low_battery_threshold(percent))?;
        Ok(())
    }

    /// Write Smart Dimming into **every** onboard profile. The firmware stores this
    /// flag per slot (`args[0]=slot`); a volatile-only write is restored on profile switch.
    pub fn set_smart_dimming(&self, on: bool) -> Result<(), HidError> {
        let slots = self.list_existing_slots()?;
        for (i, slot) in slots.iter().copied().enumerate() {
            if i > 0 {
                std::thread::sleep(std::time::Duration::from_millis(40));
            }
            self.hid.send(commands::set_smart_dimming(slot, on))?;
        }
        Ok(())
    }

    pub fn set_smart_reel(&mut self, on: bool) -> Result<(), HidError> {
        self.send_slotted(commands::set_scroll_smart_reel(on))?;
        self.invalidate_profile_cache(Some(self.active_slot));
        Ok(())
    }

    pub fn set_scroll_accel(&mut self, on: bool) -> Result<(), HidError> {
        self.send_slotted(commands::set_scroll_acceleration(on))?;
        self.invalidate_profile_cache(Some(self.active_slot));
        Ok(())
    }

    pub fn set_smart_reel_level(&mut self, level: u8) -> Result<(), HidError> {
        self.send_slotted(commands::set_smart_reel_sensitivity(level))?;
        self.invalidate_profile_cache(Some(self.active_slot));
        Ok(())
    }

    pub fn set_scroll_accel_level(&mut self, level: u8) -> Result<(), HidError> {
        self.send_slotted(commands::set_scroll_acceleration_level(level))?;
        self.invalidate_profile_cache(Some(self.active_slot));
        Ok(())
    }

    pub fn set_wheel_mode(&mut self, mode: u8) -> Result<(), HidError> {
        self.send_slotted(commands::set_wheel_mode(mode))?;
        self.invalidate_profile_cache(Some(self.active_slot));
        Ok(())
    }

    pub fn activate_slot(&mut self, slot: u8) -> Result<(), HidError> {
        self.hid.send(onboard::activate_slot(slot))?;
        self.active_slot = slot;
        Ok(())
    }

    pub fn create_slot(&mut self, slot: u8) -> Result<(), HidError> {
        self.hid.send(onboard::create_slot(slot))?;
        self.invalidate_profile_cache(None);
        Ok(())
    }

    pub fn remove_slot(&mut self, slot: u8) -> Result<(), HidError> {
        if slot == 1 {
            return Ok(());
        }
        self.hid.send(onboard::remove_slot(slot))?;
        self.invalidate_profile_cache(Some(slot));
        if self.active_slot == slot {
            self.activate_slot(1)?;
        }
        Ok(())
    }

    pub fn reset_slot_factory(&mut self, slot: u8) -> Result<(), HidError> {
        let prev = self.active_slot;
        if slot != prev {
            self.activate_slot(slot)?;
        }
        for report in onboard::factory_settings_reports() {
            self.send_slotted(report)?;
        }
        self.invalidate_profile_cache(Some(slot));
        if slot != prev {
            self.activate_slot(prev)?;
        }
        Ok(())
    }

    /// Write a button function block and confirm it sticks (read-back with retries).
    /// Same residency model as Serpentine: `02/0c` into the active onboard slot.
    pub fn set_button(&mut self, table_id: u8, block: [u8; 7], layer: u8) -> Result<(), HidError> {
        for attempt in 0..6 {
            if attempt > 0 {
                std::thread::sleep(std::time::Duration::from_millis(40));
            }
            self.send_slotted(commands::set_button_function(table_id, &block, layer))?;
            std::thread::sleep(std::time::Duration::from_millis(30));
            if let Ok(resp) = self.send_slotted(commands::get_button_function(table_id, layer)) {
                if let Some(got) = commands::parse_button_function(&resp, table_id, layer) {
                    // Prefer an exact match. For HyperShift, also require the device echoed
                    // layer 1 so we don't treat an unset L1 (which mirrors L0) as success.
                    let layer_ok = resp.arguments[2] == layer
                        || layer == onboard::LAYER_NORMAL;
                    if layer_ok && (got == block || onboard::blocks_equivalent(&got, &block)) {
                        self.invalidate_profile_cache(Some(self.active_slot));
                        return Ok(());
                    }
                }
            }
        }
        Err(HidError::StillBusy)
    }

    pub fn rename_slot(&self, slot: u8, name: String) -> Result<(), crate::config::ConfigError> {
        let mut cfg = AppConfig::load();
        cfg.set_name(&self.serial, slot, name);
        cfg.save()
    }
}
