use serde::{Deserialize, Serialize};

use crate::report::RazerReport;

pub const VARSTORE: u8 = 0x01;
pub const LOGO_LED: u8 = 0x04;
pub const SCROLL_LED: u8 = 0x01;
pub const UNDERGLOW_LED: u8 = 0x0A;
pub const ZERO_LED: u8 = 0x00;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum LightingZone {
    Logo = LOGO_LED,
    ScrollWheel = SCROLL_LED,
    Underglow = UNDERGLOW_LED,
}

impl LightingZone {
    pub fn name(self) -> &'static str {
        match self {
            Self::Logo => "Logo",
            Self::ScrollWheel => "Wheel",
            Self::Underglow => "Underglow",
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn all() -> &'static [LightingZone] {
        &[Self::Logo, Self::ScrollWheel, Self::Underglow]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LightingEffect {
    Off,
    Static,
    Breathing,
    Spectrum,
    Wave,
    Reactive,
}

impl LightingEffect {
    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Static => "Static",
            Self::Breathing => "Breathing",
            Self::Spectrum => "Spectrum",
            Self::Wave => "Wave",
            Self::Reactive => "Reactive",
        }
    }

    pub fn all() -> &'static [LightingEffect] {
        &[
            Self::Static,
            Self::Breathing,
            Self::Spectrum,
            Self::Reactive,
            Self::Wave,
            Self::Off,
        ]
    }
}

const BODY_SIZE: usize = 10;

pub fn effect_body(effect: LightingEffect, color: Rgb) -> [u8; BODY_SIZE] {
    let rgb = [color.r, color.g, color.b];
    let mut body = [0u8; BODY_SIZE];
    let prefix: &[u8] = match effect {
        LightingEffect::Off => &[0x00, 0, 0, 0],
        LightingEffect::Static => &[0x01, 0x00, 0x00, 0x01],
        LightingEffect::Breathing => &[0x02, 0x01, 0x28, 0x01],
        LightingEffect::Spectrum => &[0x03, 0x00, 0x28, 0x00],
        LightingEffect::Wave => &[0x04, 0x01, 0x28, 0x00],
        LightingEffect::Reactive => &[0x05, 0x00, 0x28, 0x01],
    };
    body[..prefix.len()].copy_from_slice(prefix);
    if matches!(
        effect,
        LightingEffect::Static | LightingEffect::Breathing | LightingEffect::Reactive
    ) {
        body[4..7].copy_from_slice(&rgb);
    }
    body
}

pub fn parse_effect_body(body: &[u8]) -> Option<(LightingEffect, Option<Rgb>)> {
    if body.len() < 4 {
        return None;
    }
    let effect = match body[0] {
        0x00 => LightingEffect::Off,
        0x01 => LightingEffect::Static,
        0x02 => LightingEffect::Breathing,
        0x03 => LightingEffect::Spectrum,
        0x04 => LightingEffect::Wave,
        0x05 => LightingEffect::Reactive,
        _ => return None,
    };
    let color = if body[3] >= 1 && body.len() >= 7 {
        Some(Rgb::new(body[4], body[5], body[6]))
    } else {
        None
    };
    Some((effect, color))
}

pub fn set_zone_effect(zone: LightingZone, effect: LightingEffect, color: Rgb) -> RazerReport {
    let mut r = RazerReport::new(0x0f, 0x02, 0x0c);
    r.arguments[0] = VARSTORE;
    r.arguments[1] = zone.as_u8();
    let body = effect_body(effect, color);
    r.arguments[2..12].copy_from_slice(&body);
    r
}

pub fn get_zone_effect(zone: LightingZone) -> RazerReport {
    let mut r = RazerReport::new(0x0f, 0x82, 0x0c);
    r.arguments[0] = VARSTORE;
    r.arguments[1] = zone.as_u8();
    r
}

pub fn parse_zone_effect(resp: &RazerReport) -> Option<(LightingEffect, Option<Rgb>)> {
    parse_effect_body(&resp.arguments[2..12])
}

pub fn set_brightness(led: u8, value: u8) -> RazerReport {
    let mut r = RazerReport::new(0x0f, 0x04, 0x03);
    r.arguments[0] = VARSTORE;
    r.arguments[1] = led;
    r.arguments[2] = value;
    r
}

pub fn get_brightness(led: u8) -> RazerReport {
    let mut r = RazerReport::new(0x0f, 0x84, 0x03);
    r.arguments[0] = VARSTORE;
    r.arguments[1] = led;
    r
}

pub fn brightness_percent(raw: u8) -> u8 {
    ((f64::from(raw) * 100.0 / 255.0).round() as u8).min(100)
}

pub fn brightness_raw(percent: u8) -> u8 {
    (f64::from(percent.min(100)) * 255.0 / 100.0).round() as u8
}
