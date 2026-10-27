//! Lightweight i18n. Defaults to Spanish; switches to English when LANG/LC_MESSAGES is `en*`.

use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Es,
    En,
}

static LANG: OnceLock<Lang> = OnceLock::new();

pub fn init() {
    let _ = LANG.set(detect());
}

pub fn lang() -> Lang {
    *LANG.get_or_init(detect)
}

fn detect() -> Lang {
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(key) {
            let v = v.to_ascii_lowercase();
            if v.is_empty() || v == "c" || v.starts_with("c.") {
                continue;
            }
            if v.starts_with("en") {
                return Lang::En;
            }
            if v.starts_with("es") {
                return Lang::Es;
            }
        }
    }
    // Product default for this Linux port: Spanish.
    Lang::Es
}

fn pick<'a>(es: &'a str, en: &'a str) -> &'a str {
    match lang() {
        Lang::Es => es,
        Lang::En => en,
    }
}

macro_rules! tr {
    ($es:literal, $en:literal) => {
        pick($es, $en)
    };
}

pub fn app_title() -> &'static str {
    "Synapsex"
}

pub fn keep_background() -> &'static str {
    tr!("Mantener en segundo plano", "Keep in background")
}

pub fn looking_for_mouse() -> &'static str {
    tr!(
        "Buscando Basilisk V4 Pro…",
        "Looking for Basilisk V4 Pro…"
    )
}

pub fn section_dpi() -> &'static str {
    tr!("DPI", "DPI")
}

pub fn apply_dpi() -> &'static str {
    tr!("Aplicar DPI", "Apply DPI")
}

pub fn stages_placeholder() -> &'static str {
    tr!(
        "Etapas: 400,800,1600,3200,6400",
        "Stages: 400,800,1600,3200,6400"
    )
}

pub fn apply_stages() -> &'static str {
    tr!("Aplicar etapas", "Apply stages")
}

pub fn section_poll() -> &'static str {
    tr!("Frecuencia de sondeo", "Polling rate")
}

pub fn apply() -> &'static str {
    tr!("Aplicar", "Apply")
}

pub fn section_lighting() -> &'static str {
    tr!(
        "Iluminación (se guarda en el perfil activo del ratón)",
        "Lighting (writes to active onboard profile)"
    )
}

pub fn apply_lighting() -> &'static str {
    tr!("Aplicar iluminación", "Apply lighting")
}

pub fn color_label() -> &'static str {
    tr!("Color", "Color")
}

pub fn color_dialog_title() -> &'static str {
    tr!("Elegir color de iluminación", "Choose lighting color")
}

pub fn color_presets_label() -> &'static str {
    tr!("Presets", "Presets")
}

pub fn section_wheel() -> &'static str {
    tr!("Rueda de desplazamiento", "Scroll wheel")
}

pub fn smart_reel() -> &'static str {
    "Smart-Reel"
}

pub fn scroll_accel() -> &'static str {
    tr!("Aceleración del scroll", "Scroll acceleration")
}

pub fn apply_wheel() -> &'static str {
    tr!("Aplicar rueda", "Apply wheel")
}

pub fn section_power() -> &'static str {
    tr!("Energía", "Power")
}

pub fn sleep_seconds() -> &'static str {
    tr!("Suspensión (s)", "Sleep (s)")
}

pub fn low_battery_pct() -> &'static str {
    tr!("Batería baja %", "Low battery %")
}

pub fn apply_power() -> &'static str {
    tr!("Aplicar energía", "Apply power")
}

pub fn section_buttons() -> &'static str {
    tr!("Botones (remapeos en el ratón)", "Buttons (onboard remaps)")
}

pub fn section_profiles() -> &'static str {
    tr!(
        "Perfiles (leídos de la memoria del ratón)",
        "Profiles (read from mouse onboard memory)"
    )
}

pub fn activate() -> &'static str {
    tr!("Activar", "Activate")
}

pub fn create_next() -> &'static str {
    tr!("Crear siguiente", "Create next")
}

pub fn rename_placeholder() -> &'static str {
    tr!("Renombrar perfil activo", "Rename active profile")
}

pub fn rename() -> &'static str {
    tr!("Renombrar", "Rename")
}

pub fn use_profile() -> &'static str {
    tr!("Usar", "Use")
}

pub fn profiles_on_device() -> &'static str {
    tr!("Perfil en el ratón", "On-mouse profile")
}

pub fn create_profile() -> &'static str {
    tr!("Crear", "Create")
}

pub fn delete_profile() -> &'static str {
    tr!("Eliminar", "Delete")
}

pub fn toast_deleted(slot: u8) -> String {
    match lang() {
        Lang::Es => format!("Perfil {slot} eliminado del ratón"),
        Lang::En => format!("Deleted profile {slot} from mouse"),
    }
}

pub fn toast_cannot_delete_slot1() -> &'static str {
    tr!(
        "El perfil 1 no se puede eliminar",
        "Profile 1 cannot be deleted"
    )
}

pub fn profile_dropdown_item(slot: u8, name: &str, active: bool) -> String {
    if active {
        match lang() {
            Lang::Es => format!("● {slot} — {name}"),
            Lang::En => format!("● {slot} — {name}"),
        }
    } else {
        format!("{slot} — {name}")
    }
}

pub fn zone_all() -> &'static str {
    tr!("Todas", "All")
}

pub fn zone_logo() -> &'static str {
    "Logo"
}

pub fn zone_wheel() -> &'static str {
    tr!("Rueda", "Wheel")
}

pub fn zone_underglow() -> &'static str {
    tr!("Luz inferior", "Underglow")
}

pub fn zone_label(name: &str) -> String {
    match name {
        "Logo" => zone_logo().into(),
        "Wheel" => zone_wheel().into(),
        "Underglow" => zone_underglow().into(),
        other => other.into(),
    }
}

pub fn effect_static() -> &'static str {
    tr!("Fijo", "Static")
}

pub fn effect_breathing() -> &'static str {
    tr!("Respiración", "Breathing")
}

pub fn effect_spectrum() -> &'static str {
    tr!("Espectro", "Spectrum")
}

pub fn effect_reactive() -> &'static str {
    tr!("Reactivo", "Reactive")
}

pub fn effect_wave() -> &'static str {
    tr!("Onda", "Wave")
}

pub fn effect_off() -> &'static str {
    tr!("Apagado", "Off")
}

pub fn effect_name(effect: &str) -> String {
    match effect {
        "Static" | "Fijo" => effect_static().into(),
        "Breathing" | "Respiración" => effect_breathing().into(),
        "Spectrum" | "Espectro" => effect_spectrum().into(),
        "Reactive" | "Reactivo" => effect_reactive().into(),
        "Wave" | "Onda" => effect_wave().into(),
        "Off" | "Apagado" => effect_off().into(),
        other => other.into(),
    }
}

pub fn current_dpi(x: u16, y: u16) -> String {
    match lang() {
        Lang::Es => format!("Actual {x}×{y}"),
        Lang::En => format!("Current {x}×{y}"),
    }
}

pub fn battery_line(percent: u8, charging: bool, serial: &str) -> String {
    match lang() {
        Lang::Es => format!(
            "Batería {}%{} · serie {}",
            percent,
            if charging { " cargando" } else { "" },
            serial
        ),
        Lang::En => format!(
            "Battery {}%{} · serial {}",
            percent,
            if charging { " charging" } else { "" },
            serial
        ),
    }
}

pub fn active_profiles(slot: u8, name: &str, count: usize) -> String {
    match lang() {
        Lang::Es => format!("Activo: {slot} ({name}) · {count} perfil(es) en el ratón"),
        Lang::En => format!("Active: {slot} ({name}) · {count} profile(s) on mouse"),
    }
}

pub fn profile_header(name: &str) -> String {
    match lang() {
        Lang::Es => format!("Perfil: {name}"),
        Lang::En => format!("Profile: {name}"),
    }
}

pub fn slot_title(active: bool, slot: u8, name: &str) -> String {
    if active {
        format!("● Slot {slot} — {name}")
    } else {
        format!("Slot {slot} — {name}")
    }
}

pub fn slot_subtitle(dpi_x: u16, dpi_y: u16, stages: &str, wheel: u8, lights: &str) -> String {
    match lang() {
        Lang::Es => format!(
            "DPI {dpi_x}×{dpi_y} (etapas {stages}) · modo rueda {wheel} · {lights}"
        ),
        Lang::En => format!(
            "DPI {dpi_x}×{dpi_y} (stages {stages}) · wheel mode {wheel} · {lights}"
        ),
    }
}

pub fn no_mouse(err: &str) -> String {
    match lang() {
        Lang::Es => format!("Sin ratón: {err}"),
        Lang::En => format!("No mouse: {err}"),
    }
}

pub fn toast_dpi_written() -> &'static str {
    tr!("DPI guardado en el ratón", "DPI written")
}

pub fn toast_stages_written() -> &'static str {
    tr!("Etapas guardadas", "Stages written")
}

pub fn toast_poll_written() -> &'static str {
    tr!("Sondeo guardado", "Polling written")
}

pub fn toast_lighting_saved() -> &'static str {
    tr!(
        "Iluminación guardada en el perfil activo",
        "Lighting saved in active profile"
    )
}

pub fn toast_wheel_saved() -> &'static str {
    tr!("Ajustes de rueda guardados", "Wheel settings saved")
}

pub fn toast_power_saved() -> &'static str {
    tr!("Ajustes de energía guardados", "Power settings saved")
}

pub fn toast_activated(slot: u8) -> String {
    match lang() {
        Lang::Es => format!("Perfil {slot} activado"),
        Lang::En => format!("Activated profile {slot}"),
    }
}

pub fn toast_activated_reload(slot: u8) -> String {
    match lang() {
        Lang::Es => format!("Slot {slot} activado — recargando desde el ratón"),
        Lang::En => format!("Activated slot {slot} — reloading from mouse"),
    }
}

pub fn toast_slots_full() -> &'static str {
    tr!("Los 5 slots están ocupados", "All 5 slots in use")
}

pub fn toast_created(slot: u8) -> String {
    match lang() {
        Lang::Es => format!("Slot {slot} creado"),
        Lang::En => format!("Created slot {slot}"),
    }
}

pub fn toast_renamed() -> &'static str {
    tr!("Renombrado", "Renamed")
}

pub fn toast_button_saved() -> &'static str {
    tr!("Botón guardado en el ratón", "Button saved on mouse")
}

pub fn toast_error(err: &str) -> String {
    match lang() {
        Lang::Es => format!("Error: {err}"),
        Lang::En => format!("Error: {err}"),
    }
}

pub fn toast_profile_changed() -> &'static str {
    tr!(
        "Perfil cambiado en el ratón — recargando luces y ajustes",
        "Profile changed on mouse — reloading lights & settings"
    )
}

pub fn connection_label(raw: &str) -> String {
    match raw {
        "Wired" => tr!("Cable", "Wired").into(),
        "2.4 GHz" => "2.4 GHz".into(),
        "Bluetooth" => "Bluetooth".into(),
        other => other.into(),
    }
}

pub fn default_profile_name(slot: u8) -> String {
    match lang() {
        Lang::Es => format!("Perfil {slot}"),
        Lang::En => format!("Profile {slot}"),
    }
}

// --- CLI ---

pub fn cli_about() -> &'static str {
    tr!(
        "Synapsex — sonda y configura el Razer Basilisk V4 Pro",
        "Synapsex — probe and configure the Razer Basilisk V4 Pro"
    )
}

pub fn cli_no_mouse() -> &'static str {
    tr!(
        "No se encontró un ratón Razer compatible (hace falta 1532:00E2/00E3).",
        "No supported Razer mouse found (need 1532:00E2/00E3)."
    )
}

pub fn cli_udev_hint() -> &'static str {
    tr!(
        "Instala packaging/99-synapsex.rules si hay denegación de permisos.",
        "Install packaging/99-synapsex.rules if permission denied."
    )
}

pub fn cli_lighting_applied() -> &'static str {
    tr!("Iluminación aplicada", "Lighting applied")
}

pub fn cli_watching() -> &'static str {
    tr!(
        "Observando cambios de perfil con el botón del ratón (Ctrl+C para salir)…",
        "Watching for profile-button slot changes (Ctrl+C to stop)…"
    )
}

pub fn lighting_effect_label(effect: &str) -> String {
    let key = effect.to_ascii_lowercase();
    match key.as_str() {
        "static" | "fijo" => effect_static().into(),
        "breathing" | "respiración" | "respiracion" => effect_breathing().into(),
        "spectrum" | "espectro" => effect_spectrum().into(),
        "reactive" | "reactivo" => effect_reactive().into(),
        "wave" | "onda" => effect_wave().into(),
        "off" | "apagado" => effect_off().into(),
        other => other.to_string(),
    }
}

pub fn button_label(english: &str) -> String {
    match lang() {
        Lang::En => english.into(),
        Lang::Es => match english {
            "Disabled" => "Desactivado".into(),
            "Left Click" => "Clic izquierdo".into(),
            "Right Click" => "Clic derecho".into(),
            "Middle Click" => "Clic central".into(),
            "Back" => "Atrás".into(),
            "Forward" => "Adelante".into(),
            "Scroll Up" => "Scroll arriba".into(),
            "Scroll Down" => "Scroll abajo".into(),
            "DPI Cycle Up" => "Ciclar DPI ↑".into(),
            "Cycle Up Profile" => "Ciclar perfil ↑".into(),
            "Cycle Down Profile" => "Ciclar perfil ↓".into(),
            "Scroll Left" => "Scroll izquierda".into(),
            "Scroll Right" => "Scroll derecha".into(),
            "HyperShift" => "HyperShift".into(),
            "Cycle Scroll Mode" => "Ciclar modo de rueda".into(),
            "Double Click" => "Doble clic".into(),
            "Volume Up" => "Subir volumen".into(),
            "Volume Down" => "Bajar volumen".into(),
            "Mute" => "Silencio".into(),
            "Play/Pause" => "Reproducir/Pausa".into(),
            "DPI Stage Up" => "Etapa DPI ↑".into(),
            "DPI Stage Down" => "Etapa DPI ↓".into(),
            "Custom" => "Personalizado".into(),
            "Default" => "Predeterminado".into(),
            // Keyboard / compositor binds keep English HID names (F6, Super+F1, …)
            other => other.into(),
        },
    }
}

pub fn button_control_name(english: &str) -> String {
    match lang() {
        Lang::En => english.into(),
        Lang::Es => match english {
            "Right Click" => "Clic derecho".into(),
            "Middle Click" => "Clic central".into(),
            "Scroll Up" => "Scroll arriba".into(),
            "Scroll Down" => "Scroll abajo".into(),
            "Back" => "Atrás".into(),
            "Forward" => "Adelante".into(),
            "DPI Cycle" => "Ciclo DPI".into(),
            "Side Front" => "Lateral frontal".into(),
            "Left Side (Profile)" => "Lateral izquierdo (perfil)".into(),
            "Tilt Left" => "Inclinación izq.".into(),
            "Tilt Right" => "Inclinación der.".into(),
            "Side Trigger" => "Gatillo lateral".into(),
            "Wheel Mode" => "Modo de rueda".into(),
            other => other.into(),
        },
    }
}

/// Translate a remap preset / block label. Keyboard names (F6, Super+F1, …) stay as-is.
pub fn remap_label(english: &str) -> String {
    button_label(english)
}
