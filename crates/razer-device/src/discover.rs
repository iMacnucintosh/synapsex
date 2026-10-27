use std::fs;
use std::path::{Path, PathBuf};

use razer_protocol::devices::{self, DeviceInfo};

use crate::hidraw::{HidDevice, HidError};

#[derive(Clone, Debug)]
pub struct FoundDevice {
    pub pid: u16,
    pub name: String,
    pub serial_hint: Option<String>,
    pub interfaces: Vec<InterfacePath>,
    pub info: &'static DeviceInfo,
}

#[derive(Clone, Debug)]
pub struct InterfacePath {
    pub path: PathBuf,
    pub interface: u8,
}

fn read_uevent(hidraw: &Path) -> Option<String> {
    let uevent = hidraw.join("device/uevent");
    fs::read_to_string(uevent).ok()
}

fn parse_hid_id(uevent: &str) -> Option<(u16, u16)> {
    // HID_ID=0003:00001532:000000E3
    for line in uevent.lines() {
        if let Some(rest) = line.strip_prefix("HID_ID=") {
            let parts: Vec<&str> = rest.split(':').collect();
            if parts.len() == 3 {
                let vid = u16::from_str_radix(parts[1], 16).ok()?;
                let pid = u16::from_str_radix(parts[2], 16).ok()?;
                return Some((vid, pid));
            }
        }
    }
    None
}

fn parse_uniq(uevent: &str) -> Option<String> {
    for line in uevent.lines() {
        if let Some(rest) = line.strip_prefix("HID_UNIQ=") {
            let s = rest.trim();
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

fn interface_number(hidraw_sys: &Path) -> Option<u8> {
    // .../hidrawN/device/../bInterfaceNumber
    let path = hidraw_sys.join("device/../bInterfaceNumber");
    fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

/// Enumerate Razer Basilisk V4 Pro hidraw nodes on the system.
pub fn list_devices() -> Vec<FoundDevice> {
    let mut by_pid: std::collections::BTreeMap<u16, FoundDevice> = std::collections::BTreeMap::new();
    let Ok(entries) = fs::read_dir("/sys/class/hidraw") else {
        return Vec::new();
    };
    for entry in entries.flatten() {
        let sys = entry.path();
        let name = entry.file_name();
        let Some(uevent) = read_uevent(&sys) else {
            continue;
        };
        let Some((vid, pid)) = parse_hid_id(&uevent) else {
            continue;
        };
        if vid != devices::VENDOR_ID || !devices::is_supported(pid) {
            continue;
        }
        let Some(info) = devices::info(pid) else {
            continue;
        };
        let iface = interface_number(&sys).unwrap_or(0);
        let path = PathBuf::from("/dev").join(&name);
        let serial_hint = parse_uniq(&uevent);
        let slot = by_pid.entry(pid).or_insert_with(|| FoundDevice {
            pid,
            name: info.name.to_string(),
            serial_hint: serial_hint.clone(),
            interfaces: Vec::new(),
            info,
        });
        if slot.serial_hint.is_none() {
            slot.serial_hint = serial_hint;
        }
        slot.interfaces.push(InterfacePath {
            path,
            interface: iface,
        });
    }
    for dev in by_pid.values_mut() {
        dev.interfaces.sort_by_key(|i| i.interface);
    }
    by_pid.into_values().collect()
}

/// Open the first working control interface for a found device.
/// Prefer interface 0, then try the rest.
pub fn open_control(found: &FoundDevice) -> Result<HidDevice, HidError> {
    let mut last_err = HidError::NoControlInterface(found.pid);
    let mut ordered = found.interfaces.clone();
    ordered.sort_by_key(|i| if i.interface == 0 { 0 } else { 1 + i.interface as u16 });

    for iface in ordered {
        match HidDevice::open(&iface.path, found.pid, iface.interface) {
            Ok(dev) => return Ok(dev),
            Err(e) => last_err = e,
        }
    }
    Err(last_err)
}

/// Discover and open the first supported Basilisk V4 Pro.
pub fn open_first() -> Result<(FoundDevice, HidDevice), HidError> {
    let devices = list_devices();
    let Some(found) = devices.into_iter().next() else {
        return Err(HidError::NoControlInterface(0));
    };
    let hid = open_control(&found)?;
    Ok((found, hid))
}
