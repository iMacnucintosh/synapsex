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

pub fn tab_buttons() -> &'static str {
    tr!("Botones", "Buttons")
}

pub fn tab_performance() -> &'static str {
    tr!("Rendimiento", "Performance")
}

pub fn tab_lighting() -> &'static str {
    tr!("Iluminación", "Lighting")
}

pub fn tab_more() -> &'static str {
    tr!("Más", "More")
}

pub fn menu_app() -> &'static str {
    tr!("Menú", "Menu")
}

pub fn profile_actions() -> &'static str {
    tr!("Acciones de perfil", "Profile actions")
}

pub fn rename_dialog_title() -> &'static str {
    tr!("Renombrar perfil", "Rename profile")
}

pub fn rename_dialog_body() -> &'static str {
    tr!(
        "Nombre local para el perfil activo (no se escribe en el firmware).",
        "Local name for the active profile (not written to firmware)."
    )
}

pub fn delete_dialog_title() -> &'static str {
    tr!("Eliminar perfil", "Delete profile")
}

pub fn delete_dialog_body(slot: u8) -> String {
    match lang() {
        Lang::Es => format!(
            "¿Eliminar el perfil {slot} de la memoria del ratón? Esta acción no se puede deshacer."
        ),
        Lang::En => format!(
            "Delete profile {slot} from the mouse memory? This cannot be undone."
        ),
    }
}

pub fn dpi_current_title() -> &'static str {
    tr!("DPI actual", "Current DPI")
}

pub fn dpi_stages_title() -> &'static str {
    tr!("Etapas DPI", "DPI stages")
}

pub fn zone_title() -> &'static str {
    tr!("Zona", "Zone")
}

pub fn effect_title() -> &'static str {
    tr!("Efecto", "Effect")
}

pub fn lighting_group_title() -> &'static str {
    tr!("Iluminación", "Lighting")
}

pub fn lighting_group_desc() -> &'static str {
    tr!(
        "Se guarda en el perfil activo del ratón",
        "Saved to the active onboard profile"
    )
}

pub fn wheel_group_desc() -> &'static str {
    tr!(
        "Ajustes de la rueda; pulsa Aplicar para escribirlos",
        "Wheel settings; press Apply to write them"
    )
}

pub fn power_group_desc() -> &'static str {
    tr!(
        "Suspensión y umbral de batería baja",
        "Sleep timeout and low-battery threshold"
    )
}

pub fn buttons_page_desc() -> &'static str {
    tr!(
        "Remapeos guardados en la memoria del ratón",
        "Remaps stored in the mouse onboard memory"
    )
}

pub fn device_subtitle_loading() -> &'static str {
    tr!("Buscando dispositivo…", "Looking for device…")
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

pub fn poll_unknown() -> &'static str {
    tr!("Sondeo: (no leído)", "Poll: (unread)")
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

pub fn cancel() -> &'static str {
    tr!("Cancelar", "Cancel")
}

pub fn select() -> &'static str {
    tr!("Seleccionar", "Select")
}

pub fn color_presets_label() -> &'static str {
    tr!("Presets", "Presets")
}

pub fn brightness_label() -> &'static str {
    tr!("Brillo", "Brightness")
}

pub fn smart_dimming() -> &'static str {
    "Smart Dimming"
}

pub fn smart_dimming_hint() -> &'static str {
    tr!(
        "En 2,4 GHz atenúa las luces al mover el ratón y las restaura tras ~3 s en reposo. Se aplica a todos los perfiles del ratón.",
        "On 2.4 GHz, dims lighting while you move and restores it after ~3 s idle. Applies to every onboard profile."
    )
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

pub fn layer_standard() -> &'static str {
    tr!("Estándar", "Standard")
}

pub fn layer_hypershift() -> &'static str {
    "HyperShift"
}

/// Hint under the HyperShift layer list. `clutch` is the Standard-layer button name(s)
/// currently mapped to HyperShift (from hardware); `None` if none found.
pub fn hypershift_hint(clutch: Option<&str>) -> String {
    match clutch {
        Some(name) => match lang() {
            Lang::Es => format!(
                "HyperShift: señal que emite cada botón al mantener «{name}». Valores leídos del hardware."
            ),
            Lang::En => format!(
                "HyperShift: signal each button sends while holding «{name}». Values read from hardware."
            ),
        },
        None => pick(
            "HyperShift: ningún botón tiene HyperShift en la capa Estándar. Asígnaselo a uno para usar esta capa.",
            "HyperShift: no Standard-layer button is set to HyperShift. Assign it to one to use this layer.",
        )
        .to_string(),
    }
}

pub fn remap_cat_keyboard() -> &'static str {
    tr!("Teclado", "Keyboard")
}

pub fn remap_record() -> &'static str {
    tr!("Grabar combinación…", "Record keybind…")
}

pub fn remap_f_keys() -> &'static str {
    tr!("Teclas F", "Function keys")
}

/// Shown under the F-key remap submenu (F13+ ≠ compositor name on Linux).
pub fn remap_f_keys_linux_note() -> &'static str {
    tr!(
        "En Linux, F13–F24 no llegan como F13…: enlaza en Hypr/Caelestia el nombre XF86 de la etiqueta.",
        "On Linux, F13–F24 do not arrive as F13…: bind the XF86 name from the label in Hypr/Caelestia."
    )
}

pub fn remap_digits() -> &'static str {
    tr!("Números (fila superior)", "Number row")
}

pub fn remap_keypad() -> &'static str {
    tr!("Teclado numérico", "Keypad")
}

pub fn remap_cat_mouse() -> &'static str {
    tr!("Ratón", "Mouse")
}

pub fn remap_cat_sensitivity() -> &'static str {
    tr!("Sensibilidad", "Sensitivity")
}

pub fn remap_cat_multimedia() -> &'static str {
    tr!("Multimedia", "Multimedia")
}

pub fn remap_cat_navigation() -> &'static str {
    tr!("Navegación", "Navigation")
}

pub fn remap_cat_shortcuts() -> &'static str {
    tr!("Atajos", "Shortcuts")
}

pub fn remap_cat_profile() -> &'static str {
    tr!("Perfil / ratón", "Profile / mouse")
}

pub fn remap_factory() -> &'static str {
    tr!("Predeterminado", "Default")
}

pub fn remap_disabled() -> &'static str {
    tr!("Desactivado", "Disable")
}

pub fn remap_record_title() -> &'static str {
    tr!("Grabar tecla", "Record key")
}

pub fn remap_record_body() -> &'static str {
    tr!(
        "Pulsa la combinación de teclas que quieras asignar (con Ctrl/Alt/Shift/Super si hace falta). Esc cancela.",
        "Press the key combination to assign (with Ctrl/Alt/Shift/Super if needed). Esc cancels."
    )
}

pub fn remap_record_waiting() -> &'static str {
    tr!("Esperando tecla…", "Waiting for a key…")
}

pub fn remap_category_label(id: &str) -> String {
    match id {
        "mouse" => remap_cat_mouse().into(),
        "sensitivity" => remap_cat_sensitivity().into(),
        "multimedia" => remap_cat_multimedia().into(),
        "navigation" => remap_cat_navigation().into(),
        "shortcuts" => remap_cat_shortcuts().into(),
        "profile" => remap_cat_profile().into(),
        other => other.into(),
    }
}

pub fn media_label(english: &str) -> String {
    match lang() {
        Lang::En => english.into(),
        Lang::Es => match english {
            "Next Track" => "Pista siguiente".into(),
            "Previous Track" => "Pista anterior".into(),
            "Brightness Up" => "Brillo ↑".into(),
            "Brightness Down" => "Brillo ↓".into(),
            other => button_label(other),
        },
    }
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

pub fn current_dpi_with_stage(x: u16, y: u16, stage_idx: usize, stage_count: usize) -> String {
    if stage_count == 0 {
        return current_dpi(x, y);
    }
    let n = stage_idx + 1;
    match lang() {
        Lang::Es => format!("Actual {x}×{y} · etapa {n}/{stage_count}"),
        Lang::En => format!("Current {x}×{y} · stage {n}/{stage_count}"),
    }
}

pub fn battery_short(percent: u8, charging: bool) -> String {
    match lang() {
        Lang::Es if charging => format!("{percent}% cargando"),
        Lang::En if charging => format!("{percent}% charging"),
        _ => format!("{percent}%"),
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

pub fn loading_device() -> &'static str {
    tr!("Leyendo ratón…", "Reading mouse…")
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

pub fn toast_poll_confirmed(hz: u32) -> String {
    match lang() {
        Lang::Es => format!("Sondeo en el ratón: {hz} Hz"),
        Lang::En => format!("Mouse polling: {hz} Hz"),
    }
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

pub fn toast_smart_dimming_on() -> &'static str {
    tr!("Smart Dimming activado", "Smart Dimming enabled")
}

pub fn toast_smart_dimming_off() -> &'static str {
    tr!("Smart Dimming desactivado", "Smart Dimming disabled")
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
    // Canonical English names match Serpentine (`HardwareButton` / Remap UI).
    match lang() {
        Lang::En => match english {
            "Profile (M1)" => "Left Index".into(),
            "DPI Cycle" => "DPI".into(),
            "HyperShift" => "Side Trigger".into(),
            "Side AI" => "Side Front".into(),
            "Forward" => "Side Middle".into(),
            "Back" => "Side Back".into(),
            "Tilt Left" => "Scroll Left".into(),
            "Tilt Right" => "Scroll Right".into(),
            other => other.into(),
        },
        Lang::Es => match english {
            "Left Index" | "Profile (M1)" => "Pestaña izquierda".into(),
            "Left Click" => "Clic izquierdo".into(),
            "Right Click" => "Clic derecho".into(),
            "Scroll Click" | "Middle Click" => "Clic de la rueda".into(),
            "Scroll Up" => "Scroll arriba".into(),
            "Scroll Down" => "Scroll abajo".into(),
            "Scroll Left" | "Tilt Left" => "Scroll izquierda".into(),
            "Scroll Right" | "Tilt Right" => "Scroll derecha".into(),
            "Scroll Mode" | "Wheel Mode" => "Modo de rueda".into(),
            "DPI" | "DPI Cycle" => "DPI".into(),
            "Side Trigger" | "HyperShift" => "Gatillo lateral".into(),
            "Side Front" | "Side AI" => "Lateral delantero".into(),
            "Side Middle" | "Forward" => "Lateral medio".into(),
            "Side Back" | "Back" => "Lateral trasero".into(),
            other => other.into(),
        },
    }
}

/// Translate a remap preset / block label. Keyboard names (F6, Super+F1, …) stay as-is.
pub fn remap_label(english: &str) -> String {
    button_label(english)
}
