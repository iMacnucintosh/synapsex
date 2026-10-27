use crate::commands::{set_dpi, set_dpi_stages, set_scroll_smart_reel};
use crate::lighting::{
    set_brightness, set_zone_effect, LightingEffect, LightingZone, Rgb, VARSTORE,
};
use crate::report::RazerReport;

pub const FIRST_SLOT: u8 = 1;
pub const MAX_SLOTS: u8 = 5;

/// Settings commands whose first argument is the onboard slot number.
const SLOTTED: &[(u8, u8)] = &[
    (0x04, 0x05),
    (0x04, 0x85),
    (0x04, 0x06),
    (0x04, 0x86),
    (0x02, 0x0c),
    (0x02, 0x8c),
    (0x02, 0x14),
    (0x02, 0x94),
    (0x02, 0x17),
    (0x02, 0x97),
    (0x02, 0x30),
    (0x02, 0xb0),
    (0x0f, 0x02),
    (0x0f, 0x82),
    (0x0f, 0x04),
    (0x0f, 0x84),
];

pub fn list_slots() -> RazerReport {
    RazerReport::new(0x05, 0x81, 0x0a)
}

pub fn get_active_slot() -> RazerReport {
    RazerReport::new(0x05, 0x84, 0x0a)
}

pub fn create_slot(slot: u8) -> RazerReport {
    let mut r = RazerReport::new(0x05, 0x02, 0x01);
    r.arguments[0] = slot;
    r
}

pub fn remove_slot(slot: u8) -> RazerReport {
    let mut r = RazerReport::new(0x05, 0x03, 0x01);
    r.arguments[0] = slot;
    r
}

pub fn activate_slot(slot: u8) -> RazerReport {
    let mut r = RazerReport::new(0x05, 0x04, 0x01);
    r.arguments[0] = slot;
    r
}

pub fn is_slot(value: u8) -> bool {
    (FIRST_SLOT..=MAX_SLOTS).contains(&value)
}

pub fn parse_slots(resp: &RazerReport) -> Option<Vec<u8>> {
    let size = resp.data_size as usize;
    if !(2..=resp.arguments.len()).contains(&size) {
        return None;
    }
    let mut ids: Vec<u8> = resp.arguments[1..size]
        .iter()
        .copied()
        .filter(|v| is_slot(*v))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    if ids.contains(&FIRST_SLOT) {
        Some(ids)
    } else {
        None
    }
}

pub fn parse_active_slot(resp: &RazerReport) -> Option<u8> {
    let id = resp.arguments[0];
    is_slot(id).then_some(id)
}

/// Point a settings command at `slot` (1–5). The first argument is the onboard
/// profile number, not a generic “VARSTORE” flag — writes/reads go to that slot’s memory.
pub fn address_to_slot(report: &RazerReport, slot: u8) -> RazerReport {
    let key = (report.command_class, report.command_id);
    if !SLOTTED.contains(&key) || !is_slot(slot) {
        return report.clone();
    }
    let mut r = report.clone();
    r.arguments[0] = slot;
    r
}

/// Retarget a settings report from slot 1 to another slot (Serpentine-compatible helper).
pub fn retarget(report: &RazerReport, slot: u8) -> RazerReport {
    address_to_slot(report, slot)
}

#[derive(Clone, Debug)]
pub struct ButtonControl {
    pub name: &'static str,
    pub table_id: u8,
    pub factory: [u8; 7],
}

const fn mouse_block(hid: u8) -> [u8; 7] {
    [0x01, 0x01, hid, 0, 0, 0, 0]
}

const DISABLED: [u8; 7] = [0x00, 0x01, 0x00, 0, 0, 0, 0];

/// Basilisk V4 Pro onboard button table (from Serpentine hardware probe).
pub fn basilisk_v4_pro_buttons() -> &'static [ButtonControl] {
    const BUTTONS: &[ButtonControl] = &[
        ButtonControl {
            name: "Right Click",
            table_id: 0x02,
            factory: mouse_block(2),
        },
        ButtonControl {
            name: "Middle Click",
            table_id: 0x03,
            factory: mouse_block(3),
        },
        ButtonControl {
            name: "Scroll Up",
            table_id: 0x09,
            factory: mouse_block(9),
        },
        ButtonControl {
            name: "Scroll Down",
            table_id: 0x0A,
            factory: mouse_block(10),
        },
        ButtonControl {
            name: "Back",
            table_id: 0x04,
            factory: mouse_block(4),
        },
        ButtonControl {
            name: "Forward",
            table_id: 0x05,
            factory: mouse_block(5),
        },
        ButtonControl {
            name: "DPI Cycle",
            table_id: 0x60,
            factory: [0x06, 0x01, 0x06, 0, 0, 0, 0],
        },
        ButtonControl {
            name: "Side Front",
            table_id: 0x80,
            factory: DISABLED,
        },
        ButtonControl {
            name: "Left Side (Profile)",
            table_id: 0x0E,
            factory: [0x07, 0x01, 0x04, 0, 0, 0, 0],
        },
        ButtonControl {
            name: "Tilt Left",
            table_id: 0x34,
            factory: [0x0E, 0x01, 0x68, 0x00, 0x14, 0, 0],
        },
        ButtonControl {
            name: "Tilt Right",
            table_id: 0x35,
            factory: [0x0E, 0x01, 0x69, 0x00, 0x14, 0, 0],
        },
        ButtonControl {
            name: "Side Trigger",
            table_id: 0x39,
            factory: [0x0C, 0x01, 0x39, 0, 0, 0, 0],
        },
        ButtonControl {
            name: "Wheel Mode",
            table_id: 0x6A,
            factory: [0x12, 0x01, 0x01, 0, 0, 0, 0],
        },
    ];
    BUTTONS
}

/// Keyboard function block: type 0x02, `[02, 02, modifiers, hid_usage, 0, 0, 0]`.
pub const fn keyboard_block(modifiers: u8, hid_usage: u8) -> [u8; 7] {
    [0x02, 0x02, modifiers, hid_usage, 0, 0, 0]
}

/// USB HID keyboard usage → label (usage page 0x07).
pub fn hid_key_name(usage: u8) -> String {
    match usage {
        0x04..=0x1d => {
            let letters = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
            ((letters[(usage - 0x04) as usize]) as char).to_string()
        }
        0x1e..=0x27 => {
            let nums = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];
            nums[(usage - 0x1e) as usize].to_string()
        }
        0x28 => "Enter".into(),
        0x29 => "Esc".into(),
        0x2a => "Backspace".into(),
        0x2b => "Tab".into(),
        0x2c => "Space".into(),
        0x2d => "-".into(),
        0x2e => "=".into(),
        0x2f => "[".into(),
        0x30 => "]".into(),
        0x31 => "\\".into(),
        0x33 => ";".into(),
        0x34 => "'".into(),
        0x35 => "`".into(),
        0x36 => ",".into(),
        0x37 => ".".into(),
        0x38 => "/".into(),
        0x3a..=0x45 => format!("F{}", usage - 0x39), // F1=0x3A … F12=0x45
        0x46 => "PrintScreen".into(),
        0x47 => "ScrollLock".into(),
        0x48 => "Pause".into(),
        0x49 => "Insert".into(),
        0x4a => "Home".into(),
        0x4b => "PageUp".into(),
        0x4c => "Delete".into(),
        0x4d => "End".into(),
        0x4e => "PageDown".into(),
        0x4f => "Right".into(),
        0x50 => "Left".into(),
        0x51 => "Down".into(),
        0x52 => "Up".into(),
        0x6f => "F20".into(),
        other => format!("Key 0x{other:02X}"),
    }
}

pub fn modifier_prefix(modifiers: u8) -> String {
    let mut s = String::new();
    if modifiers & 0x01 != 0 {
        s.push_str("Ctrl+");
    }
    if modifiers & 0x02 != 0 {
        s.push_str("Shift+");
    }
    if modifiers & 0x04 != 0 {
        s.push_str("Alt+");
    }
    if modifiers & 0x08 != 0 {
        s.push_str("Super+");
    }
    s
}

/// Human-readable description of an onboard function block.
pub fn describe_block(block: &[u8; 7]) -> String {
    if block[0] == 0x00 {
        return "Disabled".into();
    }
    // Keyboard shortcut stored in the mouse (Synapse / Hyprland-style binds).
    if block[0] == 0x02 {
        let modifiers = block[2];
        let usage = block[3];
        if usage == 0 {
            return match modifiers {
                0x01 => "Ctrl".into(),
                0x02 => "Shift".into(),
                0x04 => "Alt".into(),
                0x08 => "Super".into(),
                _ => format!("Modifiers 0x{modifiers:02X}"),
            };
        }
        return format!("{}{}", modifier_prefix(modifiers), hid_key_name(usage));
    }
    // Turbo / repeating scroll (tilt factory and similar)
    if block[0] == 0x0e {
        return match block[2] {
            0x68 => "Scroll Left".into(),
            0x69 => "Scroll Right".into(),
            0x09 => "Scroll Up (repeat)".into(),
            0x0a => "Scroll Down (repeat)".into(),
            1..=5 => format!("Turbo click {}", block[2]),
            _ => "Custom".into(),
        };
    }
    match (block[0], block[2]) {
        (0x01, 1) => "Left Click".into(),
        (0x01, 2) => "Right Click".into(),
        (0x01, 3) => "Middle Click".into(),
        (0x01, 4) => "Back".into(),
        (0x01, 5) => "Forward".into(),
        (0x01, 9) => "Scroll Up".into(),
        (0x01, 10) => "Scroll Down".into(),
        (0x06, 0x06) => "DPI Cycle Up".into(),
        (0x06, 0x01) => "DPI Stage Up".into(),
        (0x06, 0x02) => "DPI Stage Down".into(),
        (0x07, 0x04) => "Cycle Up Profile".into(),
        (0x07, 0x05) => "Cycle Down Profile".into(),
        (0x0b, _) => "Double Click".into(),
        (0x0c, _) => "HyperShift".into(),
        (0x12, _) => "Cycle Scroll Mode".into(),
        (0x0a, _) => {
            let usage = u16::from(block[2]) << 8 | u16::from(block[3]);
            match usage {
                0x00e9 => "Volume Up".into(),
                0x00ea => "Volume Down".into(),
                0x00e2 => "Mute".into(),
                0x00cd => "Play/Pause".into(),
                0x00b5 => "Next Track".into(),
                0x00b6 => "Previous Track".into(),
                _ => format!("Media 0x{usage:04X}"),
            }
        }
        _ => format!(
            "Custom [{:02x} {:02x} {:02x} {:02x}]",
            block[0], block[1], block[2], block[3]
        ),
    }
}

/// Legacy alias used by older call sites.
pub fn factory_label(block: &[u8; 7]) -> String {
    describe_block(block)
}

/// Whether two blocks are the same mapping (tolerate Disabled padding variants).
pub fn blocks_equivalent(a: &[u8; 7], b: &[u8; 7]) -> bool {
    if a == b {
        return true;
    }
    if a[0] == 0x00 && b[0] == 0x00 {
        return true;
    }
    false
}

/// Common remap targets for the UI (mouse actions + F-keys for compositor binds).
pub fn remap_presets() -> &'static [(&'static str, [u8; 7])] {
    const PRESETS: &[(&str, [u8; 7])] = &[
        ("Disabled", DISABLED),
        ("Left Click", mouse_block(1)),
        ("Right Click", mouse_block(2)),
        ("Middle Click", mouse_block(3)),
        ("Back", mouse_block(4)),
        ("Forward", mouse_block(5)),
        ("DPI Cycle Up", [0x06, 0x01, 0x06, 0, 0, 0, 0]),
        ("DPI Stage Up", [0x06, 0x01, 0x01, 0, 0, 0, 0]),
        ("DPI Stage Down", [0x06, 0x01, 0x02, 0, 0, 0, 0]),
        ("Cycle Up Profile", [0x07, 0x01, 0x04, 0, 0, 0, 0]),
        ("Cycle Down Profile", [0x07, 0x01, 0x05, 0, 0, 0, 0]),
        ("HyperShift", [0x0C, 0x01, 0x01, 0, 0, 0, 0]),
        ("Cycle Scroll Mode", [0x12, 0x01, 0x01, 0, 0, 0, 0]),
        ("Double Click", [0x0B, 0x01, 0x01, 0, 0, 0, 0]),
        ("Volume Up", [0x0A, 0x02, 0x00, 0xE9, 0, 0, 0]),
        ("Volume Down", [0x0A, 0x02, 0x00, 0xEA, 0, 0, 0]),
        ("Mute", [0x0A, 0x02, 0x00, 0xE2, 0, 0, 0]),
        ("Play/Pause", [0x0A, 0x02, 0x00, 0xCD, 0, 0, 0]),
        // Keyboard — common Hyprland / Synapse binds (HID usage page 0x07)
        ("F1", keyboard_block(0, 0x3a)),
        ("F2", keyboard_block(0, 0x3b)),
        ("F3", keyboard_block(0, 0x3c)),
        ("F4", keyboard_block(0, 0x3d)),
        ("F5", keyboard_block(0, 0x3e)),
        ("F6", keyboard_block(0, 0x3f)),
        ("F7", keyboard_block(0, 0x40)),
        ("F8", keyboard_block(0, 0x41)),
        ("F9", keyboard_block(0, 0x42)),
        ("F10", keyboard_block(0, 0x43)),
        ("F11", keyboard_block(0, 0x44)),
        ("F12", keyboard_block(0, 0x45)),
        ("Esc", keyboard_block(0, 0x29)),
        ("Tab", keyboard_block(0, 0x2b)),
        ("Enter", keyboard_block(0, 0x28)),
        ("Space", keyboard_block(0, 0x2c)),
        ("Backspace", keyboard_block(0, 0x2a)),
        ("Delete", keyboard_block(0, 0x4c)),
        ("Home", keyboard_block(0, 0x4a)),
        ("End", keyboard_block(0, 0x4d)),
        ("PageUp", keyboard_block(0, 0x4b)),
        ("PageDown", keyboard_block(0, 0x4e)),
        ("Up", keyboard_block(0, 0x52)),
        ("Down", keyboard_block(0, 0x51)),
        ("Left", keyboard_block(0, 0x50)),
        ("Right", keyboard_block(0, 0x4f)),
        ("Super+F1", keyboard_block(0x08, 0x3a)),
        ("Super+F2", keyboard_block(0x08, 0x3b)),
        ("Super+F3", keyboard_block(0x08, 0x3c)),
        ("Super+F4", keyboard_block(0x08, 0x3d)),
        ("Super+F5", keyboard_block(0x08, 0x3e)),
        ("Super+F6", keyboard_block(0x08, 0x3f)),
        ("Super+F7", keyboard_block(0x08, 0x40)),
        ("Super+F8", keyboard_block(0x08, 0x41)),
        ("Super+F9", keyboard_block(0x08, 0x42)),
        ("Super+F10", keyboard_block(0x08, 0x43)),
        ("Super+F11", keyboard_block(0x08, 0x44)),
        ("Super+F12", keyboard_block(0x08, 0x45)),
        ("Alt+F1", keyboard_block(0x04, 0x3a)),
        ("Alt+F2", keyboard_block(0x04, 0x3b)),
        ("Alt+F3", keyboard_block(0x04, 0x3c)),
        ("Alt+F4", keyboard_block(0x04, 0x3d)),
        ("Alt+Tab", keyboard_block(0x04, 0x2b)),
        ("Ctrl+C", keyboard_block(0x01, 0x06)),
        ("Ctrl+V", keyboard_block(0x01, 0x19)),
        ("Ctrl+X", keyboard_block(0x01, 0x1b)),
        ("Ctrl+Z", keyboard_block(0x01, 0x1d)),
        ("Ctrl+A", keyboard_block(0x01, 0x04)),
        ("Ctrl+S", keyboard_block(0x01, 0x16)),
        ("Ctrl+Tab", keyboard_block(0x01, 0x2b)),
    ];
    PRESETS
}

/// Factory settings for a freshly created / reset slot (V4 Pro USB).
pub fn factory_settings_reports() -> Vec<RazerReport> {
    let stages = [400u16, 800, 1600, 3200, 6400];
    let mut reports = vec![
        set_dpi_stages(&stages, 2, true), // stage 3 = 1600
        set_dpi(1600, 1600),
    ];
    let spectrum = Rgb::new(0xff, 0x00, 0x00);
    for zone in [
        LightingZone::ScrollWheel,
        LightingZone::Logo,
        LightingZone::Underglow,
    ] {
        reports.push(set_zone_effect(zone, LightingEffect::Spectrum, spectrum));
        reports.push(set_brightness(zone.as_u8(), 0x55));
    }
    reports.push(set_scroll_smart_reel(false));
    // Restore factory button map
    for ctrl in basilisk_v4_pro_buttons() {
        let mut r = RazerReport::new(0x02, 0x0c, 0x0a);
        r.arguments[0] = VARSTORE;
        r.arguments[1] = ctrl.table_id;
        r.arguments[2] = 0;
        r.arguments[3..10].copy_from_slice(&ctrl.factory);
        reports.push(r);
    }
    reports
}
