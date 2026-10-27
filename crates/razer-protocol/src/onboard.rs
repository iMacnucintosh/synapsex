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

/// Basilisk V4 Pro onboard buttons — IDs from firmware `02/84` (14 controls).
/// Names match Serpentine / Synapse layout labels (`HardwareButton` + side cluster).
pub fn basilisk_v4_pro_buttons() -> &'static [ButtonControl] {
    const BUTTONS: &[ButtonControl] = &[
        // Serpentine row order (plus Left Click, which Serpentine omits on purpose)
        ButtonControl {
            name: "Left Index",
            table_id: 0x0E,
            factory: [0x07, 0x01, 0x04, 0, 0, 0, 0],
        },
        ButtonControl {
            name: "Left Click",
            table_id: 0x01,
            factory: mouse_block(1),
        },
        ButtonControl {
            name: "Right Click",
            table_id: 0x02,
            factory: mouse_block(2),
        },
        ButtonControl {
            name: "Scroll Click",
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
            name: "Scroll Left",
            table_id: 0x34,
            factory: [0x0E, 0x01, 0x68, 0x00, 0x14, 0, 0],
        },
        ButtonControl {
            name: "Scroll Right",
            table_id: 0x35,
            factory: [0x0E, 0x01, 0x69, 0x00, 0x14, 0, 0],
        },
        ButtonControl {
            name: "Scroll Mode",
            table_id: 0x6A,
            factory: [0x12, 0x01, 0x01, 0, 0, 0, 0],
        },
        ButtonControl {
            name: "DPI",
            table_id: 0x60,
            factory: [0x06, 0x01, 0x06, 0, 0, 0, 0],
        },
        // Thumb: Side Trigger (factory HyperShift) + Side Front / Middle / Back
        ButtonControl {
            name: "Side Trigger",
            table_id: 0x39,
            factory: [0x0C, 0x01, 0x39, 0, 0, 0, 0],
        },
        ButtonControl {
            name: "Side Front",
            table_id: 0x80,
            factory: DISABLED,
        },
        ButtonControl {
            name: "Side Middle",
            table_id: 0x05,
            factory: mouse_block(5),
        },
        ButtonControl {
            name: "Side Back",
            table_id: 0x04,
            factory: mouse_block(4),
        },
    ];
    BUTTONS
}

/// Keyboard function block: type 0x02, `[02, 02, modifiers, hid_usage, 0, 0, 0]`.
pub const fn keyboard_block(modifiers: u8, hid_usage: u8) -> [u8; 7] {
    [0x02, 0x02, modifiers, hid_usage, 0, 0, 0]
}

/// Linux/Hyprland keysym for HID Fn (evdev `inet` map). Synapse still calls these
/// F13–F24, but compositors must bind the XF86* name — `F13` will never match.
pub fn linux_fn_keysym(f_number: u8) -> Option<&'static str> {
    match f_number {
        13 => Some("XF86Tools"),
        14 => Some("XF86Launch5"),
        15 => Some("XF86Launch6"),
        16 => Some("XF86Launch7"),
        17 => Some("XF86Launch8"),
        18 => Some("XF86Launch9"),
        19 => Some("F19"),
        20 => Some("XF86AudioMicMute"),
        21 => Some("XF86TouchpadToggle"),
        22 => Some("XF86TouchpadOn"),
        23 => Some("XF86TouchpadOff"),
        24 => Some("F24"),
        _ => None,
    }
}

/// Label for an F-key, including the Linux keysym when it differs (F13+).
pub fn f_key_label(f_number: u8) -> String {
    match linux_fn_keysym(f_number) {
        Some(sym) if sym != format!("F{f_number}") => format!("F{f_number} · {sym}"),
        _ => format!("F{f_number}"),
    }
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
        0x3a..=0x45 => f_key_label(usage - 0x39), // F1=0x3A … F12=0x45
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
        // F13–F24 (HID 0x68–0x73) — Synapse name + Linux/Hypr keysym
        0x68..=0x73 => f_key_label((usage - 0x68) + 13),
        // Keypad (numpad)
        0x54 => "KP_/".into(),
        0x55 => "KP_*".into(),
        0x56 => "KP_-".into(),
        0x57 => "KP_+".into(),
        0x58 => "KP_Enter".into(),
        0x59..=0x61 => format!("KP_{}", usage - 0x58), // KP_1..KP_9
        0x62 => "KP_0".into(),
        0x63 => "KP_.".into(),
        other => format!("Key 0x{other:02X}"),
    }
}

/// Normal (0) and HyperShift (1) remap layers stored per button on the mouse.
pub const LAYER_NORMAL: u8 = 0;
pub const LAYER_HYPERSHIFT: u8 = 1;

/// True when the onboard block is the HyperShift clutch (type `0x0C`).
pub fn is_hypershift_block(block: &[u8; 7]) -> bool {
    block[0] == 0x0C
}

/// HyperShift clutch block for `table_id`.
///
/// Observed on Basilisk V4 Pro (Mac Serpentine / Synapse writes):
/// - factory Side Trigger (`0x39`): `0C 01 39`
/// - any other button used as clutch (e.g. Side Middle): `0C 01 01`
pub const fn hypershift_block(table_id: u8) -> [u8; 7] {
    let id = if table_id == 0x39 { 0x39 } else { 0x01 };
    [0x0C, 0x01, id, 0, 0, 0, 0]
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
                0x006f => "Brightness Up".into(),
                0x0070 => "Brightness Down".into(),
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

const fn consumer_block(usage: u16) -> [u8; 7] {
    [0x0A, 0x02, (usage >> 8) as u8, (usage & 0xff) as u8, 0, 0, 0]
}

/// One item in a remap submenu (Serpentine-style categories).
#[derive(Clone, Copy, Debug)]
pub struct RemapItem {
    pub label: &'static str,
    pub block: [u8; 7],
}

/// Named category shown as a submenu in the remap UI.
#[derive(Clone, Copy, Debug)]
pub struct RemapCategory {
    pub id: &'static str,
    pub items: &'static [RemapItem],
}

/// Top-row digit keys 1–9, 0 — same blocks Synapse/Serpentine write (`02 02 00 1e…`).
pub fn remap_digits() -> &'static [RemapItem] {
    const ITEMS: &[RemapItem] = &[
        RemapItem { label: "1", block: keyboard_block(0, 0x1e) },
        RemapItem { label: "2", block: keyboard_block(0, 0x1f) },
        RemapItem { label: "3", block: keyboard_block(0, 0x20) },
        RemapItem { label: "4", block: keyboard_block(0, 0x21) },
        RemapItem { label: "5", block: keyboard_block(0, 0x22) },
        RemapItem { label: "6", block: keyboard_block(0, 0x23) },
        RemapItem { label: "7", block: keyboard_block(0, 0x24) },
        RemapItem { label: "8", block: keyboard_block(0, 0x25) },
        RemapItem { label: "9", block: keyboard_block(0, 0x26) },
        RemapItem { label: "0", block: keyboard_block(0, 0x27) },
    ];
    ITEMS
}

/// Numpad keys KP_1…KP_9, KP_0 (HID 0x59–0x62).
pub fn remap_keypad() -> &'static [RemapItem] {
    const ITEMS: &[RemapItem] = &[
        RemapItem { label: "KP_1", block: keyboard_block(0, 0x59) },
        RemapItem { label: "KP_2", block: keyboard_block(0, 0x5a) },
        RemapItem { label: "KP_3", block: keyboard_block(0, 0x5b) },
        RemapItem { label: "KP_4", block: keyboard_block(0, 0x5c) },
        RemapItem { label: "KP_5", block: keyboard_block(0, 0x5d) },
        RemapItem { label: "KP_6", block: keyboard_block(0, 0x5e) },
        RemapItem { label: "KP_7", block: keyboard_block(0, 0x5f) },
        RemapItem { label: "KP_8", block: keyboard_block(0, 0x60) },
        RemapItem { label: "KP_9", block: keyboard_block(0, 0x61) },
        RemapItem { label: "KP_0", block: keyboard_block(0, 0x62) },
    ];
    ITEMS
}

/// Function-key presets F1–F24 (HID 0x3A–0x45, 0x68–0x73).
/// F13+ labels include the Linux/Hypr keysym (`XF86…`) so compositor binds match.
pub fn remap_f_keys() -> &'static [RemapItem] {
    const ITEMS: &[RemapItem] = &[
        RemapItem { label: "F1", block: keyboard_block(0, 0x3a) },
        RemapItem { label: "F2", block: keyboard_block(0, 0x3b) },
        RemapItem { label: "F3", block: keyboard_block(0, 0x3c) },
        RemapItem { label: "F4", block: keyboard_block(0, 0x3d) },
        RemapItem { label: "F5", block: keyboard_block(0, 0x3e) },
        RemapItem { label: "F6", block: keyboard_block(0, 0x3f) },
        RemapItem { label: "F7", block: keyboard_block(0, 0x40) },
        RemapItem { label: "F8", block: keyboard_block(0, 0x41) },
        RemapItem { label: "F9", block: keyboard_block(0, 0x42) },
        RemapItem { label: "F10", block: keyboard_block(0, 0x43) },
        RemapItem { label: "F11", block: keyboard_block(0, 0x44) },
        RemapItem { label: "F12", block: keyboard_block(0, 0x45) },
        RemapItem {
            label: "F13 · XF86Tools",
            block: keyboard_block(0, 0x68),
        },
        RemapItem {
            label: "F14 · XF86Launch5",
            block: keyboard_block(0, 0x69),
        },
        RemapItem {
            label: "F15 · XF86Launch6",
            block: keyboard_block(0, 0x6a),
        },
        RemapItem {
            label: "F16 · XF86Launch7",
            block: keyboard_block(0, 0x6b),
        },
        RemapItem {
            label: "F17 · XF86Launch8",
            block: keyboard_block(0, 0x6c),
        },
        RemapItem {
            label: "F18 · XF86Launch9",
            block: keyboard_block(0, 0x6d),
        },
        RemapItem { label: "F19", block: keyboard_block(0, 0x6e) },
        RemapItem {
            label: "F20 · XF86AudioMicMute",
            block: keyboard_block(0, 0x6f),
        },
        RemapItem {
            label: "F21 · XF86TouchpadToggle",
            block: keyboard_block(0, 0x70),
        },
        RemapItem {
            label: "F22 · XF86TouchpadOn",
            block: keyboard_block(0, 0x71),
        },
        RemapItem {
            label: "F23 · XF86TouchpadOff",
            block: keyboard_block(0, 0x72),
        },
        RemapItem { label: "F24", block: keyboard_block(0, 0x73) },
    ];
    ITEMS
}

/// Remap menu categories (mirrors Serpentine: Keyboard / Mouse / Sensitivity / Multimedia / …).
pub fn remap_categories() -> &'static [RemapCategory] {
    const MOUSE: &[RemapItem] = &[
        RemapItem { label: "Left Click", block: mouse_block(1) },
        RemapItem { label: "Right Click", block: mouse_block(2) },
        RemapItem { label: "Middle Click", block: mouse_block(3) },
        RemapItem { label: "Back", block: mouse_block(4) },
        RemapItem { label: "Forward", block: mouse_block(5) },
        RemapItem { label: "Double Click", block: [0x0B, 0x01, 0x01, 0, 0, 0, 0] },
        RemapItem { label: "Scroll Up", block: mouse_block(9) },
        RemapItem { label: "Scroll Down", block: mouse_block(10) },
        RemapItem {
            label: "Scroll Left",
            block: [0x0E, 0x01, 0x68, 0x00, 0x14, 0, 0],
        },
        RemapItem {
            label: "Scroll Right",
            block: [0x0E, 0x01, 0x69, 0x00, 0x14, 0, 0],
        },
    ];
    const SENSITIVITY: &[RemapItem] = &[
        RemapItem {
            label: "DPI Cycle Up",
            block: [0x06, 0x01, 0x06, 0, 0, 0, 0],
        },
        RemapItem {
            label: "DPI Stage Up",
            block: [0x06, 0x01, 0x01, 0, 0, 0, 0],
        },
        RemapItem {
            label: "DPI Stage Down",
            block: [0x06, 0x01, 0x02, 0, 0, 0, 0],
        },
    ];
    const MULTIMEDIA: &[RemapItem] = &[
        RemapItem {
            label: "Volume Up",
            block: consumer_block(0x00e9),
        },
        RemapItem {
            label: "Volume Down",
            block: consumer_block(0x00ea),
        },
        RemapItem {
            label: "Mute",
            block: consumer_block(0x00e2),
        },
        RemapItem {
            label: "Play/Pause",
            block: consumer_block(0x00cd),
        },
        RemapItem {
            label: "Next Track",
            block: consumer_block(0x00b5),
        },
        RemapItem {
            label: "Previous Track",
            block: consumer_block(0x00b6),
        },
        RemapItem {
            label: "Brightness Up",
            block: consumer_block(0x006f),
        },
        RemapItem {
            label: "Brightness Down",
            block: consumer_block(0x0070),
        },
    ];
    const NAVIGATION: &[RemapItem] = &[
        RemapItem { label: "Esc", block: keyboard_block(0, 0x29) },
        RemapItem { label: "Tab", block: keyboard_block(0, 0x2b) },
        RemapItem { label: "Enter", block: keyboard_block(0, 0x28) },
        RemapItem { label: "Space", block: keyboard_block(0, 0x2c) },
        RemapItem { label: "Backspace", block: keyboard_block(0, 0x2a) },
        RemapItem { label: "Delete", block: keyboard_block(0, 0x4c) },
        RemapItem { label: "Home", block: keyboard_block(0, 0x4a) },
        RemapItem { label: "End", block: keyboard_block(0, 0x4d) },
        RemapItem { label: "PageUp", block: keyboard_block(0, 0x4b) },
        RemapItem { label: "PageDown", block: keyboard_block(0, 0x4e) },
        RemapItem { label: "Up", block: keyboard_block(0, 0x52) },
        RemapItem { label: "Down", block: keyboard_block(0, 0x51) },
        RemapItem { label: "Left", block: keyboard_block(0, 0x50) },
        RemapItem { label: "Right", block: keyboard_block(0, 0x4f) },
        RemapItem { label: "Alt+Tab", block: keyboard_block(0x04, 0x2b) },
        RemapItem { label: "Ctrl+Tab", block: keyboard_block(0x01, 0x2b) },
    ];
    const PROFILE: &[RemapItem] = &[
        RemapItem {
            label: "Cycle Up Profile",
            block: [0x07, 0x01, 0x04, 0, 0, 0, 0],
        },
        RemapItem {
            label: "Cycle Down Profile",
            block: [0x07, 0x01, 0x05, 0, 0, 0, 0],
        },
        RemapItem {
            label: "Cycle Scroll Mode",
            block: [0x12, 0x01, 0x01, 0, 0, 0, 0],
        },
        RemapItem {
            // Placeholder; writers must call `hypershift_block(table_id)` before send.
            label: "HyperShift",
            block: [0x0C, 0x01, 0x00, 0, 0, 0, 0],
        },
    ];
    const SHORTCUTS: &[RemapItem] = &[
        RemapItem { label: "Super+F1", block: keyboard_block(0x08, 0x3a) },
        RemapItem { label: "Super+F2", block: keyboard_block(0x08, 0x3b) },
        RemapItem { label: "Super+F3", block: keyboard_block(0x08, 0x3c) },
        RemapItem { label: "Super+F4", block: keyboard_block(0x08, 0x3d) },
        RemapItem { label: "Super+F5", block: keyboard_block(0x08, 0x3e) },
        RemapItem { label: "Super+F6", block: keyboard_block(0x08, 0x3f) },
        RemapItem { label: "Super+F7", block: keyboard_block(0x08, 0x40) },
        RemapItem { label: "Super+F8", block: keyboard_block(0x08, 0x41) },
        RemapItem { label: "Super+F9", block: keyboard_block(0x08, 0x42) },
        RemapItem { label: "Super+F10", block: keyboard_block(0x08, 0x43) },
        RemapItem { label: "Super+F11", block: keyboard_block(0x08, 0x44) },
        RemapItem { label: "Super+F12", block: keyboard_block(0x08, 0x45) },
        RemapItem { label: "Ctrl+C", block: keyboard_block(0x01, 0x06) },
        RemapItem { label: "Ctrl+V", block: keyboard_block(0x01, 0x19) },
        RemapItem { label: "Ctrl+X", block: keyboard_block(0x01, 0x1b) },
        RemapItem { label: "Ctrl+Z", block: keyboard_block(0x01, 0x1d) },
        RemapItem { label: "Ctrl+A", block: keyboard_block(0x01, 0x04) },
        RemapItem { label: "Ctrl+S", block: keyboard_block(0x01, 0x16) },
    ];
    const CATS: &[RemapCategory] = &[
        RemapCategory {
            id: "mouse",
            items: MOUSE,
        },
        RemapCategory {
            id: "sensitivity",
            items: SENSITIVITY,
        },
        RemapCategory {
            id: "multimedia",
            items: MULTIMEDIA,
        },
        RemapCategory {
            id: "navigation",
            items: NAVIGATION,
        },
        RemapCategory {
            id: "shortcuts",
            items: SHORTCUTS,
        },
        RemapCategory {
            id: "profile",
            items: PROFILE,
        },
    ];
    CATS
}

/// Flat list of every known preset (CLI / search). Includes F-keys + category items + Disabled.
pub fn remap_presets() -> Vec<(&'static str, [u8; 7])> {
    let mut out = Vec::new();
    out.push(("Disabled", DISABLED));
    for item in remap_digits() {
        out.push((item.label, item.block));
    }
    for item in remap_keypad() {
        out.push((item.label, item.block));
    }
    for item in remap_f_keys() {
        out.push((item.label, item.block));
    }
    for cat in remap_categories() {
        for item in cat.items {
            out.push((item.label, item.block));
        }
    }
    out
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_fn_keysyms_match_xkb_inet() {
        assert_eq!(linux_fn_keysym(13), Some("XF86Tools"));
        assert_eq!(linux_fn_keysym(14), Some("XF86Launch5"));
        assert_eq!(linux_fn_keysym(15), Some("XF86Launch6"));
        assert_eq!(linux_fn_keysym(16), Some("XF86Launch7"));
        assert_eq!(linux_fn_keysym(17), Some("XF86Launch8"));
        assert_eq!(linux_fn_keysym(18), Some("XF86Launch9"));
        assert_eq!(linux_fn_keysym(20), Some("XF86AudioMicMute"));
        assert_eq!(f_key_label(6), "F6");
        assert_eq!(f_key_label(13), "F13 · XF86Tools");
        assert_eq!(hid_key_name(0x68), "F13 · XF86Tools");
        assert_eq!(hid_key_name(0x69), "F14 · XF86Launch5");
    }
}
