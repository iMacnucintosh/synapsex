//! Onboard profile snapshot (JSON) for export / restore.

use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use razer_device::DeviceSession;
use razer_protocol::{onboard, LightingEffect, LightingZone, Rgb};
use serde::{Deserialize, Serialize};

const FORMAT: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
pub struct ProfileBackup {
    pub format: u32,
    pub device: String,
    pub serial: String,
    pub firmware: String,
    pub captured_at_unix: u64,
    pub note: String,
    pub profile: ProfileBody,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProfileBody {
    pub slot: u8,
    pub name: String,
    pub dpi_x: u16,
    pub dpi_y: u16,
    pub dpi_stages: Vec<u16>,
    pub active_dpi_stage: usize,
    pub smart_reel: bool,
    pub scroll_accel: bool,
    pub smart_reel_level: u8,
    pub scroll_accel_level: u8,
    pub wheel_mode: u8,
    pub poll_hz: u32,
    pub idle_seconds: u16,
    pub low_battery_percent: u8,
    pub smart_dimming: bool,
    pub zones: Vec<ZoneBody>,
    pub buttons_l0: Vec<ButtonBody>,
    pub buttons_l1: Vec<ButtonBody>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ZoneBody {
    pub zone: u8,
    pub zone_name: String,
    pub effect: String,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub brightness: u8,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ButtonBody {
    pub table_id: u8,
    pub name: String,
    pub label: String,
    /// Seven-byte onboard block as lowercase hex (14 chars).
    pub block_hex: String,
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn block_to_hex(block: &[u8; 7]) -> String {
    block.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_to_block(hex: &str) -> Result<[u8; 7]> {
    let hex = hex.trim();
    if hex.len() != 14 {
        bail!("block_hex must be 14 hex chars, got len {}", hex.len());
    }
    let mut out = [0u8; 7];
    for i in 0..7 {
        out[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
            .with_context(|| format!("bad hex at byte {i}"))?;
    }
    Ok(out)
}

fn effect_from_name(s: &str) -> Result<LightingEffect> {
    Ok(match s.to_ascii_lowercase().as_str() {
        "off" => LightingEffect::Off,
        "static" => LightingEffect::Static,
        "breathing" | "breathe" => LightingEffect::Breathing,
        "spectrum" => LightingEffect::Spectrum,
        "wave" => LightingEffect::Wave,
        "reactive" => LightingEffect::Reactive,
        other => bail!("unknown effect {other}"),
    })
}

fn zone_from_u8(z: u8) -> Result<LightingZone> {
    Ok(match z {
        z if z == LightingZone::Logo.as_u8() => LightingZone::Logo,
        z if z == LightingZone::ScrollWheel.as_u8() => LightingZone::ScrollWheel,
        z if z == LightingZone::Underglow.as_u8() => LightingZone::Underglow,
        other => bail!("unknown lighting zone 0x{other:02x}"),
    })
}

fn map_buttons(btns: &[razer_device::ButtonState]) -> Vec<ButtonBody> {
    btns.iter()
        .map(|b| ButtonBody {
            table_id: b.table_id,
            name: b.name.to_string(),
            label: b.label.clone(),
            block_hex: block_to_hex(&b.block),
        })
        .collect()
}

/// Read slot `slot` from the mouse and write a JSON snapshot to `path`.
pub fn export_slot(session: &mut DeviceSession, slot: u8, path: &Path) -> Result<()> {
    let st = session.read_state().context("read_state")?;
    let p = st
        .profiles
        .iter()
        .find(|p| p.slot == slot)
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| session.read_slot_profile(slot))
        .context("read slot profile")?;

    let zones = p
        .zones
        .iter()
        .map(|z| ZoneBody {
            zone: z.zone.as_u8(),
            zone_name: z.zone.name().to_string(),
            effect: z.effect.name().to_string(),
            r: z.color.r,
            g: z.color.g,
            b: z.color.b,
            brightness: z.brightness,
        })
        .collect();

    let backup = ProfileBackup {
        format: FORMAT,
        device: st.name.clone(),
        serial: st.serial.clone(),
        firmware: st.firmware.clone(),
        captured_at_unix: now_unix(),
        note: format!(
            "Onboard slot {slot} «{}» — restore with: synapsex-cli profile import <file>",
            p.name
        ),
        profile: ProfileBody {
            slot: p.slot,
            name: p.name.clone(),
            dpi_x: p.dpi_x,
            dpi_y: p.dpi_y,
            dpi_stages: p.dpi_stages.clone(),
            active_dpi_stage: p.active_dpi_stage,
            smart_reel: p.smart_reel,
            scroll_accel: p.scroll_accel,
            smart_reel_level: p.smart_reel_level,
            scroll_accel_level: p.scroll_accel_level,
            wheel_mode: p.wheel_mode,
            poll_hz: st.poll_hz,
            idle_seconds: st.idle_seconds,
            low_battery_percent: st.low_battery_percent,
            smart_dimming: st.smart_dimming,
            zones,
            buttons_l0: map_buttons(&p.buttons),
            buttons_l1: map_buttons(&p.hypershift_buttons),
        },
    };

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).ok();
        }
    }
    let json = serde_json::to_string_pretty(&backup).context("serialize")?;
    fs::write(path, json.as_bytes()).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// Restore a JSON snapshot into `target_slot` (defaults to the slot stored in the file).
pub fn import_slot(
    session: &mut DeviceSession,
    path: &Path,
    target_slot: Option<u8>,
) -> Result<()> {
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let backup: ProfileBackup = serde_json::from_str(&raw).context("parse profile JSON")?;
    if backup.format != FORMAT {
        bail!(
            "unsupported profile format {} (want {FORMAT})",
            backup.format
        );
    }
    let p = &backup.profile;
    let slot = target_slot.unwrap_or(p.slot);

    session
        .activate_slot(slot)
        .with_context(|| format!("activate slot {slot}"))?;

    if !p.dpi_stages.is_empty() {
        session
            .set_dpi_stages(&p.dpi_stages, p.active_dpi_stage)
            .context("set_dpi_stages")?;
    }
    session
        .set_dpi(p.dpi_x, p.dpi_y)
        .context("set_dpi")?;

    session.set_smart_reel(p.smart_reel).context("smart_reel")?;
    session
        .set_scroll_accel(p.scroll_accel)
        .context("scroll_accel")?;
    session
        .set_smart_reel_level(p.smart_reel_level)
        .context("smart_reel_level")?;
    session
        .set_scroll_accel_level(p.scroll_accel_level)
        .context("scroll_accel_level")?;
    session.set_wheel_mode(p.wheel_mode).context("wheel_mode")?;

    if p.poll_hz != 0 {
        session.set_poll(p.poll_hz).context("poll")?;
    }
    session.set_idle(p.idle_seconds).context("idle")?;
    session
        .set_low_battery(p.low_battery_percent)
        .context("low_battery")?;
    session
        .set_smart_dimming(p.smart_dimming)
        .context("smart_dimming")?;

    for z in &p.zones {
        let zone = zone_from_u8(z.zone)?;
        let effect = effect_from_name(&z.effect)?;
        let color = Rgb::new(z.r, z.g, z.b);
        session
            .set_zone_lighting(zone, effect, color, z.brightness)
            .with_context(|| format!("zone {}", z.zone_name))?;
    }

    for b in &p.buttons_l0 {
        let mut block = hex_to_block(&b.block_hex)?;
        if onboard::is_hypershift_block(&block) {
            block = onboard::hypershift_block(b.table_id);
        }
        session
            .set_button(b.table_id, block, onboard::LAYER_NORMAL)
            .with_context(|| format!("L0 button {} (0x{:02x})", b.name, b.table_id))?;
    }
    for b in &p.buttons_l1 {
        let mut block = hex_to_block(&b.block_hex)?;
        if onboard::is_hypershift_block(&block) {
            block = onboard::hypershift_block(b.table_id);
        }
        session
            .set_button(b.table_id, block, onboard::LAYER_HYPERSHIFT)
            .with_context(|| format!("L1 button {} (0x{:02x})", b.name, b.table_id))?;
    }

    let _ = session.rename_slot(slot, p.name.clone());
    Ok(())
}
