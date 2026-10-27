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
    pub buttons: Vec<ButtonState>,
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
    /// Every onboard profile that exists on the mouse, with settings read from firmware.
    pub profiles: Vec<SlotProfile>,
}

pub struct DeviceSession {
    pub found: FoundDevice,
    pub hid: HidDevice,
    pub active_slot: u8,
    pub serial: String,
}

impl DeviceSession {
    pub fn open() -> Result<Self, HidError> {
        let (found, hid) = discover::open_first()?;
        let mut session = Self {
            found,
            hid,
            active_slot: 1,
            serial: String::new(),
        };
        session.refresh_slot_and_serial()?;
        Ok(session)
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

    pub fn list_existing_slots(&self) -> Result<Vec<u8>, HidError> {
        let resp = self.hid.send(onboard::list_slots())?;
        Ok(onboard::parse_slots(&resp).unwrap_or_else(|| vec![1]))
    }

    /// Read one onboard profile’s settings from the mouse without activating it.
    pub fn read_slot_profile(&self, slot: u8) -> Result<SlotProfile, HidError> {
        let info = self.found.info;
        let cfg = AppConfig::load();
        let count_from_one = info.dpi_stages_count_from_one;

        let (dpi_x, dpi_y) = self
            .send_to_slot(slot, commands::get_dpi())
            .map(|r| commands::parse_dpi(&r))
            .unwrap_or((0, 0));

        let (dpi_stages, active_dpi_stage) =
            match self.send_to_slot(slot, commands::get_dpi_stages()) {
                Ok(stages_resp) => (
                    commands::parse_dpi_stages(&stages_resp),
                    commands::parse_active_dpi_stage(&stages_resp, count_from_one),
                ),
                Err(_) => (Vec::new(), 0),
            };

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

        let mut buttons = Vec::new();
        for ctrl in onboard::basilisk_v4_pro_buttons() {
            let block = self
                .send_to_slot(slot, commands::get_button_function(ctrl.table_id, 0))
                .ok()
                .and_then(|r| commands::parse_button_function(&r, ctrl.table_id))
                .unwrap_or(ctrl.factory);
            let label = onboard::describe_block(&block);
            buttons.push(ButtonState {
                name: ctrl.name,
                table_id: ctrl.table_id,
                block,
                label,
            });
        }

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
        })
    }

    /// Read every existing onboard profile from the mouse.
    pub fn read_all_profiles(&self) -> Result<Vec<SlotProfile>, HidError> {
        let slots = self.list_existing_slots()?;
        let mut profiles = Vec::with_capacity(slots.len());
        for slot in slots {
            profiles.push(self.read_slot_profile(slot)?);
        }
        Ok(profiles)
    }

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

        let poll_hz = if info.uses_polling_rate2 {
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

        // Device-wide + every onboard profile (source of truth = mouse memory).
        let profiles = self.read_all_profiles()?;
        let slots: Vec<u8> = profiles.iter().map(|p| p.slot).collect();
        let active = profiles
            .iter()
            .find(|p| p.is_active)
            .cloned()
            .or_else(|| profiles.first().cloned())
            .ok_or(HidError::NoControlInterface(self.found.pid))?;

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
            smart_reel: active.smart_reel,
            scroll_accel: active.scroll_accel,
            smart_reel_level: active.smart_reel_level,
            scroll_accel_level: active.scroll_accel_level,
            wheel_mode: active.wheel_mode,
            active_slot: self.active_slot,
            slots,
            slot_name: cfg.name_for(&self.serial, self.active_slot),
            zones: active.zones.clone(),
            buttons: active.buttons.clone(),
            profiles,
        })
    }

    pub fn set_dpi(&self, x: u16, y: u16) -> Result<(), HidError> {
        self.send_slotted(commands::set_dpi(x, y))?;
        Ok(())
    }

    pub fn set_dpi_stages(&self, stages: &[u16], active: usize) -> Result<(), HidError> {
        let count_from_one = self.found.info.dpi_stages_count_from_one;
        self.send_slotted(commands::set_dpi_stages(stages, active, count_from_one))?;
        Ok(())
    }

    pub fn set_poll(&self, hz: u32) -> Result<(), HidError> {
        for report in commands::set_polling_rate_reports(hz, self.found.pid) {
            self.hid.send(report)?;
        }
        Ok(())
    }

    pub fn set_zone_lighting(
        &self,
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
        Ok(())
    }

    pub fn set_all_zones(
        &self,
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

    pub fn set_smart_reel(&self, on: bool) -> Result<(), HidError> {
        self.send_slotted(commands::set_scroll_smart_reel(on))?;
        Ok(())
    }

    pub fn set_scroll_accel(&self, on: bool) -> Result<(), HidError> {
        self.send_slotted(commands::set_scroll_acceleration(on))?;
        Ok(())
    }

    pub fn set_smart_reel_level(&self, level: u8) -> Result<(), HidError> {
        self.send_slotted(commands::set_smart_reel_sensitivity(level))?;
        Ok(())
    }

    pub fn set_scroll_accel_level(&self, level: u8) -> Result<(), HidError> {
        self.send_slotted(commands::set_scroll_acceleration_level(level))?;
        Ok(())
    }

    pub fn set_wheel_mode(&self, mode: u8) -> Result<(), HidError> {
        self.send_slotted(commands::set_wheel_mode(mode))?;
        Ok(())
    }

    pub fn activate_slot(&mut self, slot: u8) -> Result<(), HidError> {
        self.hid.send(onboard::activate_slot(slot))?;
        self.active_slot = slot;
        Ok(())
    }

    pub fn create_slot(&mut self, slot: u8) -> Result<(), HidError> {
        self.hid.send(onboard::create_slot(slot))?;
        Ok(())
    }

    pub fn remove_slot(&mut self, slot: u8) -> Result<(), HidError> {
        if slot == 1 {
            return Ok(());
        }
        self.hid.send(onboard::remove_slot(slot))?;
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
        if slot != prev {
            self.activate_slot(prev)?;
        }
        Ok(())
    }

    pub fn set_button(&self, table_id: u8, block: [u8; 7]) -> Result<(), HidError> {
        self.send_slotted(commands::set_button_function(table_id, &block, 0))?;
        Ok(())
    }

    pub fn rename_slot(&self, slot: u8, name: String) -> Result<(), crate::config::ConfigError> {
        let mut cfg = AppConfig::load();
        cfg.set_name(&self.serial, slot, name);
        cfg.save()
    }
}
