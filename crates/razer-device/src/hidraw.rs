use std::fs::OpenOptions;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use nix::libc;
use razer_protocol::{devices, ProtocolError, RazerReport, WIRE_SIZE};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HidError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("protocol: {0}")]
    Protocol(#[from] ProtocolError),
    #[error("no usable control interface for product 0x{0:04x}")]
    NoControlInterface(u16),
    #[error("device busy after retries")]
    StillBusy,
}

// linux/hidraw.h
// #define HIDIOCSFEATURE(len) _IOC(_IOC_WRITE|_IOC_READ, 'H', 0x06, len)
// #define HIDIOCGFEATURE(len) _IOC(_IOC_WRITE|_IOC_READ, 'H', 0x07, len)
fn hid_ioc(dir_write_read: bool, nr: u8, size: usize) -> libc::c_ulong {
    const IOC_WRITE: u64 = 1;
    const IOC_READ: u64 = 2;
    let dir = if dir_write_read {
        IOC_WRITE | IOC_READ
    } else {
        IOC_READ
    };
    (dir << 30) | ((b'H' as u64) << 8) | (nr as u64) | ((size as u64) << 16)
}

fn hid_set_feature(len: usize) -> libc::c_ulong {
    hid_ioc(true, 0x06, len)
}

fn hid_get_feature(len: usize) -> libc::c_ulong {
    hid_ioc(true, 0x07, len)
}

pub struct HidDevice {
    file: std::fs::File,
    pub path: PathBuf,
    pub pid: u16,
    pub interface: u8,
    /// Some kernels want a leading report-id byte of 0.
    prepend_report_id: bool,
}

impl HidDevice {
    pub fn open(path: &Path, pid: u16, interface: u8) -> Result<Self, HidError> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        let mut dev = Self {
            file,
            path: path.to_path_buf(),
            pid,
            interface,
            prepend_report_id: false,
        };
        // Probe which framing works
        if Self::probe(&mut dev, false).is_ok() {
            return Ok(dev);
        }
        dev.prepend_report_id = true;
        Self::probe(&mut dev, true)?;
        Ok(dev)
    }

    fn probe(dev: &mut Self, with_rid: bool) -> Result<(), HidError> {
        let saved = dev.prepend_report_id;
        dev.prepend_report_id = with_rid;
        let mut req = RazerReport::new(0x00, 0x81, 0x02);
        req.transaction_id = devices::transaction_id(dev.pid, 0x00);
        let result = dev.transact_raw(&req);
        dev.prepend_report_id = saved;
        match result {
            Ok(resp) if matches!(resp.status, 0x00 | 0x01 | 0x02) => Ok(()),
            Ok(_) => Err(HidError::Protocol(ProtocolError::Failure)),
            Err(e) => Err(e),
        }
    }

    fn ioctl_buf(&self, report: &[u8; WIRE_SIZE]) -> Vec<u8> {
        if self.prepend_report_id {
            let mut v = Vec::with_capacity(WIRE_SIZE + 1);
            v.push(0);
            v.extend_from_slice(report);
            v
        } else {
            report.to_vec()
        }
    }

    fn strip_buf(&self, buf: &[u8]) -> Result<RazerReport, HidError> {
        let data = if self.prepend_report_id {
            if buf.len() < WIRE_SIZE + 1 {
                return Err(ProtocolError::Truncated.into());
            }
            &buf[1..]
        } else {
            buf
        };
        Ok(RazerReport::parse(data)?)
    }

    fn transact_raw(&self, report: &RazerReport) -> Result<RazerReport, HidError> {
        let wire = report.serialize();
        let mut set_buf = self.ioctl_buf(&wire);
        let fd = self.file.as_raw_fd();
        let set_rc = unsafe {
            libc::ioctl(
                fd,
                hid_set_feature(set_buf.len()) as _,
                set_buf.as_mut_ptr(),
            )
        };
        if set_rc < 0 {
            return Err(HidError::Io(std::io::Error::last_os_error()));
        }
        thread::sleep(Duration::from_millis(35));

        let mut get_buf = if self.prepend_report_id {
            let mut v = vec![0u8; WIRE_SIZE + 1];
            v[0] = 0;
            v
        } else {
            vec![0u8; WIRE_SIZE]
        };
        let get_rc = unsafe {
            libc::ioctl(
                fd,
                hid_get_feature(get_buf.len()) as _,
                get_buf.as_mut_ptr(),
            )
        };
        if get_rc < 0 {
            return Err(HidError::Io(std::io::Error::last_os_error()));
        }
        self.strip_buf(&get_buf)
    }

    pub fn send(&self, mut report: RazerReport) -> Result<RazerReport, HidError> {
        if report.transaction_id == 0 {
            report.transaction_id = devices::transaction_id(self.pid, report.command_class);
        }
        let mut last = None;
        for _ in 0..8 {
            let resp = self.transact_raw(&report)?;
            match resp.status {
                0x02 => return Ok(resp),
                0x01 => {
                    thread::sleep(Duration::from_millis(40));
                    last = Some(resp);
                    continue;
                }
                _ => {
                    resp.ensure_ok()?;
                    return Ok(resp);
                }
            }
        }
        if let Some(resp) = last {
            Err(HidError::Protocol(ProtocolError::Busy))
                .or_else(|_| {
                    let _ = resp;
                    Err(HidError::StillBusy)
                })
        } else {
            Err(HidError::StillBusy)
        }
    }
}
