use crate::lighting::LightingZone;

pub const VENDOR_ID: u16 = 0x1532;

pub const PID_V4_PRO_WIRED: u16 = 0x00E2;
pub const PID_V4_PRO_DONGLE: u16 = 0x00E3;
pub const PID_V4_PRO_BT: u16 = 0x00E4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Connection {
    Wired,
    WirelessDongle,
    Bluetooth,
}

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub pid: u16,
    pub name: &'static str,
    pub connection: Connection,
    pub max_dpi: u32,
    pub transaction_id: u8,
    pub matrix_transaction_id: u8,
    pub brightness_led: u8,
    pub uses_polling_rate2: bool,
    pub has_wheel_controls: bool,
    pub has_smart_reel_sensitivity: bool,
    pub has_scroll_acceleration: bool,
    pub supports_low_battery_threshold: bool,
    pub supports_sleep_timeout: bool,
    pub has_onboard_slots: bool,
    pub supports_dpi_modes: bool,
    pub dpi_stages_count_from_one: bool,
    pub lighting_zones: &'static [LightingZone],
    pub has_button_table: bool,
}

const V4_ZONES: &[LightingZone] = &[
    LightingZone::Logo,
    LightingZone::ScrollWheel,
    LightingZone::Underglow,
];

fn v4_pro(pid: u16, name: &'static str, connection: Connection) -> DeviceInfo {
    DeviceInfo {
        pid,
        name,
        connection,
        max_dpi: 50_000,
        transaction_id: 0x1f,
        matrix_transaction_id: 0x1f,
        brightness_led: 0x04, // logo
        uses_polling_rate2: connection != Connection::Bluetooth,
        has_wheel_controls: connection != Connection::Bluetooth,
        has_smart_reel_sensitivity: connection != Connection::Bluetooth,
        has_scroll_acceleration: connection != Connection::Bluetooth,
        supports_low_battery_threshold: true,
        supports_sleep_timeout: true,
        has_onboard_slots: true,
        supports_dpi_modes: true,
        dpi_stages_count_from_one: true,
        lighting_zones: V4_ZONES,
        has_button_table: true,
    }
}

pub fn known_devices() -> &'static [DeviceInfo] {
    use std::sync::OnceLock;
    static DEVICES: OnceLock<Vec<DeviceInfo>> = OnceLock::new();
    DEVICES.get_or_init(|| {
        vec![
            v4_pro(
                PID_V4_PRO_WIRED,
                "Razer Basilisk V4 Pro (Wired)",
                Connection::Wired,
            ),
            v4_pro(
                PID_V4_PRO_DONGLE,
                "Razer Basilisk V4 Pro (Wireless)",
                Connection::WirelessDongle,
            ),
            v4_pro(
                PID_V4_PRO_BT,
                "Razer Basilisk V4 Pro (Bluetooth)",
                Connection::Bluetooth,
            ),
        ]
    })
}

pub fn info(pid: u16) -> Option<&'static DeviceInfo> {
    known_devices().iter().find(|d| d.pid == pid)
}

pub fn is_supported(pid: u16) -> bool {
    info(pid).is_some()
}

pub fn transaction_id(pid: u16, command_class: u8) -> u8 {
    match info(pid) {
        Some(d) if command_class == 0x0f => d.matrix_transaction_id,
        Some(d) => d.transaction_id,
        None => 0x1f,
    }
}

pub fn supported_pids() -> &'static [u16] {
    &[PID_V4_PRO_WIRED, PID_V4_PRO_DONGLE]
}
