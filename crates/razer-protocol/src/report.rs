use thiserror::Error;

/// 90-byte Razer HID feature report (OpenRazer `struct razer_report`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RazerReport {
    pub status: u8,
    pub transaction_id: u8,
    pub remaining_packets: u16,
    pub protocol_type: u8,
    pub data_size: u8,
    pub command_class: u8,
    pub command_id: u8,
    pub arguments: [u8; 80],
}

pub const WIRE_SIZE: usize = 90;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RazerStatus {
    New = 0x00,
    Busy = 0x01,
    Successful = 0x02,
    Failure = 0x03,
    Timeout = 0x04,
    NotSupported = 0x05,
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("report shorter than {WIRE_SIZE} bytes")]
    Truncated,
    #[error("device returned status 0x{0:02x}")]
    BadStatus(u8),
    #[error("device busy")]
    Busy,
    #[error("command failed")]
    Failure,
    #[error("command timed out")]
    Timeout,
    #[error("command not supported")]
    NotSupported,
}

impl RazerReport {
    pub fn new(command_class: u8, command_id: u8, data_size: u8) -> Self {
        Self {
            status: 0,
            transaction_id: 0,
            remaining_packets: 0,
            protocol_type: 0,
            data_size,
            command_class,
            command_id,
            arguments: [0; 80],
        }
    }

    pub fn with_transaction(mut self, tid: u8) -> Self {
        self.transaction_id = tid;
        self
    }

    pub fn crc(buffer: &[u8]) -> u8 {
        let mut crc = 0u8;
        for i in 2..88 {
            if i < buffer.len() {
                crc ^= buffer[i];
            }
        }
        crc
    }

    pub fn serialize(&self) -> [u8; WIRE_SIZE] {
        let mut b = [0u8; WIRE_SIZE];
        b[0] = self.status;
        b[1] = self.transaction_id;
        b[2] = (self.remaining_packets >> 8) as u8;
        b[3] = (self.remaining_packets & 0xff) as u8;
        b[4] = self.protocol_type;
        b[5] = self.data_size;
        b[6] = self.command_class;
        b[7] = self.command_id;
        b[8..88].copy_from_slice(&self.arguments);
        b[88] = Self::crc(&b);
        b[89] = 0;
        b
    }

    pub fn parse(b: &[u8]) -> Result<Self, ProtocolError> {
        if b.len() < WIRE_SIZE {
            return Err(ProtocolError::Truncated);
        }
        let mut arguments = [0u8; 80];
        arguments.copy_from_slice(&b[8..88]);
        Ok(Self {
            status: b[0],
            transaction_id: b[1],
            remaining_packets: (u16::from(b[2]) << 8) | u16::from(b[3]),
            protocol_type: b[4],
            data_size: b[5],
            command_class: b[6],
            command_id: b[7],
            arguments,
        })
    }

    pub fn ensure_ok(&self) -> Result<(), ProtocolError> {
        match self.status {
            0x02 => Ok(()),
            0x01 => Err(ProtocolError::Busy),
            0x03 => Err(ProtocolError::Failure),
            0x04 => Err(ProtocolError::Timeout),
            0x05 => Err(ProtocolError::NotSupported),
            other => Err(ProtocolError::BadStatus(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_report() {
        let mut r = RazerReport::new(0x07, 0x80, 0x02);
        r.transaction_id = 0x1f;
        let wire = r.serialize();
        assert_eq!(wire.len(), 90);
        assert_eq!(wire[88], RazerReport::crc(&wire));
        let parsed = RazerReport::parse(&wire).unwrap();
        assert_eq!(parsed.command_class, 0x07);
        assert_eq!(parsed.command_id, 0x80);
        assert_eq!(parsed.transaction_id, 0x1f);
    }
}
