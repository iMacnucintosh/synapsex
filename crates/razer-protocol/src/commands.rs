use crate::devices::{self, Connection};
use crate::lighting::VARSTORE;
use crate::report::RazerReport;

pub const DPI_CEILING: u16 = 50_000;
pub const MAX_DPI_STAGES: usize = 5;

pub fn get_battery_level() -> RazerReport {
    RazerReport::new(0x07, 0x80, 0x02)
}

pub fn battery_percent(raw: u8) -> u8 {
    ((f64::from(raw) * 100.0 / 255.0).round() as u8).min(100)
}

pub fn get_charging_status() -> RazerReport {
    RazerReport::new(0x07, 0x84, 0x02)
}

pub fn get_serial() -> RazerReport {
    RazerReport::new(0x00, 0x82, 0x16)
}

pub fn parse_serial(resp: &RazerReport) -> Option<String> {
    let bytes: Vec<u8> = resp.arguments[..22]
        .iter()
        .copied()
        .take_while(|&b| b != 0)
        .collect();
    let raw = String::from_utf8_lossy(&bytes);
    let s: String = raw.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if s.len() >= 4 && s.chars().collect::<std::collections::HashSet<_>>().len() > 1 {
        Some(s)
    } else {
        None
    }
}

pub fn get_firmware_version() -> RazerReport {
    RazerReport::new(0x00, 0x81, 0x02)
}

pub fn parse_firmware(resp: &RazerReport) -> String {
    format!("v{}.{}", resp.arguments[0], resp.arguments[1])
}

pub fn set_dpi(x: u16, y: u16) -> RazerReport {
    let dx = x.clamp(100, DPI_CEILING);
    let dy = y.clamp(100, DPI_CEILING);
    let mut r = RazerReport::new(0x04, 0x05, 0x07);
    r.arguments[0] = VARSTORE;
    r.arguments[1] = (dx >> 8) as u8;
    r.arguments[2] = (dx & 0xff) as u8;
    r.arguments[3] = (dy >> 8) as u8;
    r.arguments[4] = (dy & 0xff) as u8;
    r
}

pub fn get_dpi() -> RazerReport {
    let mut r = RazerReport::new(0x04, 0x85, 0x07);
    r.arguments[0] = VARSTORE;
    r
}

pub fn parse_dpi(resp: &RazerReport) -> (u16, u16) {
    let x = (u16::from(resp.arguments[1]) << 8) | u16::from(resp.arguments[2]);
    let y = (u16::from(resp.arguments[3]) << 8) | u16::from(resp.arguments[4]);
    (x, y)
}

pub fn get_dpi_stages() -> RazerReport {
    let mut r = RazerReport::new(0x04, 0x86, 0x26);
    r.arguments[0] = VARSTORE;
    r
}

pub fn parse_dpi_stages(resp: &RazerReport) -> Vec<u16> {
    let count = resp.arguments[2] as usize;
    if count == 0 || count > 7 {
        return Vec::new();
    }
    let mut stages = Vec::with_capacity(count);
    for i in 0..count {
        let base = 3 + i * 7;
        if base + 2 >= resp.arguments.len() {
            break;
        }
        let x = (u16::from(resp.arguments[base + 1]) << 8) | u16::from(resp.arguments[base + 2]);
        if x > 0 {
            stages.push(x);
        }
    }
    stages
}

pub fn parse_active_dpi_stage(resp: &RazerReport, count_from_one: bool) -> usize {
    let raw = resp.arguments[1] as usize;
    if count_from_one {
        raw.saturating_sub(1)
    } else {
        raw
    }
}

pub fn set_dpi_stages(stages: &[u16], active_stage: usize, count_from_one: bool) -> RazerReport {
    let mut r = RazerReport::new(0x04, 0x06, 0x26);
    let clamped: Vec<u16> = stages
        .iter()
        .take(MAX_DPI_STAGES)
        .map(|d| (*d).clamp(100, DPI_CEILING))
        .collect();
    r.arguments[0] = VARSTORE;
    let base = if count_from_one { 1 } else { 0 };
    let active = active_stage.min(clamped.len().saturating_sub(1));
    r.arguments[1] = (active + base) as u8;
    r.arguments[2] = clamped.len() as u8;
    let mut offset = 3;
    for (i, dpi) in clamped.iter().enumerate() {
        r.arguments[offset] = (i + base) as u8;
        r.arguments[offset + 1] = (dpi >> 8) as u8;
        r.arguments[offset + 2] = (dpi & 0xff) as u8;
        r.arguments[offset + 3] = (dpi >> 8) as u8;
        r.arguments[offset + 4] = (dpi & 0xff) as u8;
        offset += 7;
    }
    r
}

pub fn get_polling_rate() -> RazerReport {
    RazerReport::new(0x00, 0x85, 0x01)
}

pub fn parse_polling_rate(resp: &RazerReport) -> u32 {
    match resp.arguments[0] {
        0x01 => 1000,
        0x02 => 500,
        0x08 => 125,
        _ => 0,
    }
}

pub fn set_polling_rate(hz: u32) -> RazerReport {
    let mut r = RazerReport::new(0x00, 0x05, 0x01);
    r.arguments[0] = match hz {
        1000 => 0x01,
        500 => 0x02,
        125 => 0x08,
        _ => 0x02,
    };
    r
}

pub const POLLING_RATE2: &[(u32, u8)] = &[
    (8000, 0x01),
    (4000, 0x02),
    (2000, 0x04),
    (1000, 0x08),
    (500, 0x10),
    (250, 0x20),
    (125, 0x40),
];

pub fn get_polling_rate2() -> RazerReport {
    RazerReport::new(0x00, 0xc0, 0x01)
}

pub fn parse_polling_rate2(resp: &RazerReport) -> u32 {
    POLLING_RATE2
        .iter()
        .find(|(_, code)| *code == resp.arguments[1])
        .map(|(hz, _)| *hz)
        .unwrap_or(0)
}

pub fn set_polling_rate2(hz: u32, argument: u8) -> RazerReport {
    let mut r = RazerReport::new(0x00, 0x40, 0x02);
    r.arguments[0] = argument;
    r.arguments[1] = POLLING_RATE2
        .iter()
        .find(|(h, _)| *h == hz)
        .map(|(_, c)| *c)
        .unwrap_or(0x08);
    r
}

pub fn set_polling_rate_reports(hz: u32, pid: u16) -> Vec<RazerReport> {
    let Some(info) = devices::info(pid) else {
        return vec![set_polling_rate(hz)];
    };
    if !info.uses_polling_rate2 {
        return vec![set_polling_rate(hz)];
    }
    if info.connection == Connection::WirelessDongle {
        vec![
            set_polling_rate2(hz, 0x00),
            set_polling_rate2(hz, 0x01),
        ]
    } else {
        vec![set_polling_rate2(hz, 0x00)]
    }
}

pub fn supported_polling_rates(pid: u16) -> Vec<u32> {
    match devices::info(pid) {
        Some(i) if i.uses_polling_rate2 => POLLING_RATE2.iter().map(|(hz, _)| *hz).collect(),
        _ => vec![125, 500, 1000],
    }
}

pub fn get_low_battery_threshold() -> RazerReport {
    RazerReport::new(0x07, 0x82, 0x01)
}

pub fn set_low_battery_threshold(percent: u8) -> RazerReport {
    let mut r = RazerReport::new(0x07, 0x02, 0x01);
    r.arguments[0] = (f64::from(percent.min(100)) * 255.0 / 100.0).round() as u8;
    r
}

pub fn parse_low_battery_threshold(resp: &RazerReport) -> u8 {
    ((f64::from(resp.arguments[0]) * 100.0 / 255.0).round() as u8).min(100)
}

pub fn get_idle_time() -> RazerReport {
    RazerReport::new(0x07, 0x83, 0x02)
}

pub fn set_idle_time(seconds: u16) -> RazerReport {
    let mut r = RazerReport::new(0x07, 0x03, 0x02);
    r.arguments[0] = (seconds >> 8) as u8;
    r.arguments[1] = (seconds & 0xff) as u8;
    r
}

pub fn parse_idle_time(resp: &RazerReport) -> u16 {
    (u16::from(resp.arguments[0]) << 8) | u16::from(resp.arguments[1])
}

fn scroll_read(command_id: u8) -> RazerReport {
    let mut r = RazerReport::new(0x02, command_id, 0x02);
    r.arguments[0] = VARSTORE;
    r
}

pub fn get_scroll_smart_reel() -> RazerReport {
    scroll_read(0x97)
}

pub fn set_scroll_smart_reel(on: bool) -> RazerReport {
    let mut r = RazerReport::new(0x02, 0x17, 0x02);
    r.arguments[0] = VARSTORE;
    r.arguments[1] = if on { 1 } else { 0 };
    r
}

pub fn get_wheel_mode() -> RazerReport {
    scroll_read(0xb0)
}

pub fn set_wheel_mode(mode: u8) -> RazerReport {
    let mut r = RazerReport::new(0x02, 0x30, 0x02);
    r.arguments[0] = VARSTORE;
    r.arguments[1] = mode;
    r
}

pub fn get_scroll_acceleration() -> RazerReport {
    scroll_read(0x96)
}

pub fn set_scroll_acceleration(on: bool) -> RazerReport {
    let mut r = RazerReport::new(0x02, 0x16, 0x02);
    r.arguments[0] = VARSTORE;
    r.arguments[1] = if on { 1 } else { 0 };
    r
}

pub fn get_smart_reel_sensitivity() -> RazerReport {
    scroll_read(0xa3)
}

pub fn set_smart_reel_sensitivity(level: u8) -> RazerReport {
    let mut r = RazerReport::new(0x02, 0x23, 0x02);
    r.arguments[0] = VARSTORE;
    r.arguments[1] = level.min(4);
    r
}

pub fn get_scroll_acceleration_level() -> RazerReport {
    scroll_read(0xa2)
}

pub fn set_scroll_acceleration_level(level: u8) -> RazerReport {
    let mut r = RazerReport::new(0x02, 0x22, 0x02);
    r.arguments[0] = VARSTORE;
    r.arguments[1] = level.min(4);
    r
}

pub fn parse_flag(resp: &RazerReport) -> bool {
    resp.arguments[1] != 0
}

pub fn parse_level(resp: &RazerReport) -> u8 {
    resp.arguments[1].min(4)
}

pub fn get_button_function(id: u8, layer: u8) -> RazerReport {
    let mut r = RazerReport::new(0x02, 0x8c, 0x0a);
    r.arguments[0] = VARSTORE;
    r.arguments[1] = id;
    r.arguments[2] = layer;
    r
}

pub fn set_button_function(id: u8, block: &[u8; 7], layer: u8) -> RazerReport {
    let mut r = RazerReport::new(0x02, 0x0c, 0x0a);
    r.arguments[0] = VARSTORE;
    r.arguments[1] = id;
    r.arguments[2] = layer;
    r.arguments[3..10].copy_from_slice(block);
    r
}

pub fn parse_button_function(resp: &RazerReport, id: u8) -> Option<[u8; 7]> {
    if resp.arguments[1] != id {
        return None;
    }
    let mut block = [0u8; 7];
    block.copy_from_slice(&resp.arguments[3..10]);
    Some(block)
}
