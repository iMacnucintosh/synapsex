use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use razer_device::{list_devices, open_control, DeviceSession};
use razer_protocol::{commands, onboard, LightingEffect, LightingZone, Rgb};
use synapsex_i18n as i18n;

#[derive(Parser)]
#[command(name = "synapsex-cli", about = "Synapsex — probe and configure the Razer Basilisk V4 Pro")]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List detected supported mice
    List,
    /// Device identity, firmware, connection
    Info,
    /// Battery level and charging
    Battery,
    /// Read or set DPI / stages
    Dpi {
        #[arg(long)]
        set: Option<u16>,
        #[arg(long, value_delimiter = ',')]
        stages: Option<Vec<u16>>,
        #[arg(long)]
        active: Option<usize>,
    },
    /// Read or set polling rate
    Poll {
        #[arg(long)]
        set: Option<u32>,
    },
    /// Lighting per zone or all zones
    Rgb {
        #[arg(long, default_value = "all")]
        zone: String,
        #[arg(long)]
        effect: Option<String>,
        #[arg(long)]
        color: Option<String>,
        #[arg(long)]
        brightness: Option<u8>,
    },
    /// Power / sleep / low-battery
    Power {
        #[arg(long)]
        idle: Option<u16>,
        #[arg(long)]
        low_battery: Option<u8>,
    },
    /// Scroll wheel settings
    Wheel {
        #[arg(long)]
        smart_reel: Option<bool>,
        #[arg(long)]
        accel: Option<bool>,
        #[arg(long)]
        mode: Option<u8>,
        #[arg(long)]
        smart_reel_level: Option<u8>,
        #[arg(long)]
        accel_level: Option<u8>,
    },
    /// Onboard profile slots
    Slots {
        #[arg(long)]
        activate: Option<u8>,
        #[arg(long)]
        create: Option<u8>,
        #[arg(long)]
        remove: Option<u8>,
        #[arg(long)]
        reset: Option<u8>,
        #[arg(long)]
        rename: Option<String>,
        #[arg(long)]
        rename_slot: Option<u8>,
        /// Poll until the active slot changes (profile button)
        #[arg(long)]
        watch: bool,
    },
    /// Button remaps
    Buttons {
        #[arg(long)]
        set: Option<String>,
        #[arg(long)]
        to: Option<String>,
    },
    /// Dump full state as text
    State,
}

fn parse_color(s: &str) -> Result<Rgb> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        bail!("color must be RRGGBB");
    }
    let r = u8::from_str_radix(&s[0..2], 16)?;
    let g = u8::from_str_radix(&s[2..4], 16)?;
    let b = u8::from_str_radix(&s[4..6], 16)?;
    Ok(Rgb::new(r, g, b))
}

fn parse_effect(s: &str) -> Result<LightingEffect> {
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

fn parse_zone(s: &str) -> Result<Option<LightingZone>> {
    Ok(match s.to_ascii_lowercase().as_str() {
        "all" => None,
        "logo" => Some(LightingZone::Logo),
        "scroll" | "wheel" => Some(LightingZone::ScrollWheel),
        "underglow" | "glow" => Some(LightingZone::Underglow),
        other => bail!("unknown zone {other}"),
    })
}

fn main() -> Result<()> {
    i18n::init();
    let cli = Cli::parse();
    match cli.cmd {
        Command::List => {
            let devices = list_devices();
            if devices.is_empty() {
                println!("{}", i18n::cli_no_mouse());
                println!("{}", i18n::cli_udev_hint());
                return Ok(());
            }
            for d in devices {
                println!(
                    "{:04x}:{} — {} ({} interfaces)",
                    d.pid,
                    d.name,
                    d.serial_hint.as_deref().unwrap_or("?"),
                    d.interfaces.len()
                );
                for i in &d.interfaces {
                    println!("  iface {} → {}", i.interface, i.path.display());
                }
            }
        }
        Command::Info | Command::Battery | Command::State => {
            let mut session = DeviceSession::open().context("open device")?;
            let state = session.read_state().context("read state")?;
            match cli.cmd {
                Command::Info => {
                    println!("{} ({})", state.name, state.connection);
                    println!("PID 0x{:04x}  serial {}  firmware {}", state.pid, state.serial, state.firmware);
                    println!(
                        "Profile slot {} ({}) — slots {:?}",
                        state.active_slot, state.slot_name, state.slots
                    );
                }
                Command::Battery => {
                    println!(
                        "Battery {}%{}",
                        state.battery_percent,
                        if state.charging { " (charging)" } else { "" }
                    );
                }
                Command::State => {
                    println!("{:#?}", state);
                }
                _ => unreachable!(),
            }
        }
        Command::Dpi { set, stages, active } => {
            let mut session = DeviceSession::open()?;
            if let Some(dpi) = set {
                session.set_dpi(dpi, dpi)?;
                println!("DPI set to {dpi}");
            }
            if let Some(st) = stages {
                let act = active.unwrap_or(0);
                session.set_dpi_stages(&st, act)?;
                println!("Stages set to {st:?} active={act}");
            }
            let state = session.read_state()?;
            println!(
                "DPI {}x{}  stages {:?}  active_stage={}",
                state.dpi_x, state.dpi_y, state.dpi_stages, state.active_dpi_stage
            );
        }
        Command::Poll { set } => {
            let mut session = DeviceSession::open()?;
            if let Some(hz) = set {
                session.set_poll(hz)?;
                println!("Poll set to {hz} Hz");
            }
            let state = session.read_state()?;
            println!("Poll {} Hz (supported: {:?})", state.poll_hz, commands::supported_polling_rates(state.pid));
        }
        Command::Rgb {
            zone,
            effect,
            color,
            brightness,
        } => {
            let mut session = DeviceSession::open()?;
            let z = parse_zone(&zone)?;
            if effect.is_some() || color.is_some() || brightness.is_some() {
                let eff = effect
                    .as_deref()
                    .map(parse_effect)
                    .transpose()?
                    .unwrap_or(LightingEffect::Static);
                let col = color
                    .as_deref()
                    .map(parse_color)
                    .transpose()?
                    .unwrap_or(Rgb::new(0, 255, 0));
                let bright = brightness.unwrap_or(100);
                match z {
                    None => session.set_all_zones(eff, col, bright)?,
                    Some(zone) => session.set_zone_lighting(zone, eff, col, bright)?,
                }
                println!("{}", i18n::cli_lighting_applied());
            }
            let state = session.read_state()?;
            for zs in state.zones {
                println!(
                    "{}: {:?} color=#{:02x}{:02x}{:02x} brightness={}%",
                    zs.zone.name(),
                    zs.effect,
                    zs.color.r,
                    zs.color.g,
                    zs.color.b,
                    zs.brightness
                );
            }
        }
        Command::Power { idle, low_battery } => {
            let mut session = DeviceSession::open()?;
            if let Some(s) = idle {
                session.set_idle(s)?;
            }
            if let Some(p) = low_battery {
                session.set_low_battery(p)?;
            }
            let state = session.read_state()?;
            println!(
                "Idle {}s  low-battery threshold {}%",
                state.idle_seconds, state.low_battery_percent
            );
        }
        Command::Wheel {
            smart_reel,
            accel,
            mode,
            smart_reel_level,
            accel_level,
        } => {
            let mut session = DeviceSession::open()?;
            if let Some(v) = smart_reel {
                session.set_smart_reel(v)?;
            }
            if let Some(v) = accel {
                session.set_scroll_accel(v)?;
            }
            if let Some(v) = mode {
                session.set_wheel_mode(v)?;
            }
            if let Some(v) = smart_reel_level {
                session.set_smart_reel_level(v)?;
            }
            if let Some(v) = accel_level {
                session.set_scroll_accel_level(v)?;
            }
            let state = session.read_state()?;
            println!(
                "smart_reel={} (lvl {})  accel={} (lvl {})  wheel_mode={}",
                state.smart_reel,
                state.smart_reel_level,
                state.scroll_accel,
                state.scroll_accel_level,
                state.wheel_mode
            );
        }
        Command::Slots {
            activate,
            create,
            remove,
            reset,
            rename,
            rename_slot,
            watch,
        } => {
            let mut session = DeviceSession::open()?;
            if let Some(s) = create {
                session.create_slot(s)?;
                println!("Created slot {s}");
            }
            if let Some(s) = remove {
                session.remove_slot(s)?;
                println!("Removed slot {s}");
            }
            if let Some(s) = reset {
                session.reset_slot_factory(s)?;
                println!("Reset slot {s} to factory");
            }
            if let Some(s) = activate {
                session.activate_slot(s)?;
                println!("Activated slot {s}");
            }
            if let (Some(name), Some(slot)) = (rename, rename_slot.or(activate)) {
                session.rename_slot(slot, name.clone())?;
                println!("Renamed slot {slot} → {name}");
            }
            if watch {
                println!("{}", i18n::cli_watching());
                loop {
                    if session.poll_slot_change()? {
                        let state = session.read_state()?;
                        println!(
                            "→ slot {} ({})  DPI {}  zones: {}",
                            state.active_slot,
                            state.slot_name,
                            state.dpi_x,
                            state
                                .zones
                                .iter()
                                .map(|z| format!("{}={:?}", z.zone.name(), z.effect))
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                    }
                    std::thread::sleep(std::time::Duration::from_millis(400));
                }
            }
            let state = session.read_state()?;
            println!(
                "Active slot {} ({})  — {} profile(s) on mouse",
                state.active_slot,
                state.slot_name,
                state.profiles.len()
            );
            for p in &state.profiles {
                let mark = if p.is_active { "●" } else { " " };
                let lights = p
                    .zones
                    .iter()
                    .map(|z| {
                        format!(
                            "{}={:?} #{:02x}{:02x}{:02x} {}%",
                            z.zone.name(),
                            z.effect,
                            z.color.r,
                            z.color.g,
                            z.color.b,
                            z.brightness
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                println!(
                    "{mark} slot {} «{}»  DPI {}×{} stages {:?}  smart_reel={} accel={} wheel={}",
                    p.slot,
                    p.name,
                    p.dpi_x,
                    p.dpi_y,
                    p.dpi_stages,
                    p.smart_reel,
                    p.scroll_accel,
                    p.wheel_mode
                );
                println!("    lights: {lights}");
                for b in &p.buttons {
                    println!("    button 0x{:02x} {:<22} {}", b.table_id, b.name, b.label);
                }
            }
        }
        Command::Buttons { set, to } => {
            let mut session = DeviceSession::open()?;
            if let (Some(name), Some(target)) = (set, to) {
                let ctrl = onboard::basilisk_v4_pro_buttons()
                    .iter()
                    .find(|c| c.name.eq_ignore_ascii_case(&name) || c.table_id.to_string() == name)
                    .context("unknown button")?;
                let block = onboard::remap_presets()
                    .iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case(&target))
                    .map(|(_, b)| *b)
                    .context("unknown remap target")?;
                session.set_button(ctrl.table_id, block)?;
                println!("{} → {}", ctrl.name, target);
            }
            let state = session.read_state()?;
            for b in state.buttons {
                println!("0x{:02x} {:<22} {}", b.table_id, b.name, b.label);
            }
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn _probe_interfaces() -> Result<()> {
    for d in list_devices() {
        for i in &d.interfaces {
            match open_control(&d) {
                Ok(h) => {
                    println!("opened {} iface {}", h.path.display(), h.interface);
                    break;
                }
                Err(e) => println!("{}: {e}", i.path.display()),
            }
        }
    }
    Ok(())
}
