//! Caelestia theming: paint the window with the colour scheme of the
//! Hyprland activity it currently lives in, and follow it live.
//!
//! Caelestia writes `~/.config/gtk-4.0/gtk.css` with the colours of the
//! *active* activity in its shell mode, which clashes with `lightApps`. We
//! resolve the palette ourselves and load it above the user stylesheet.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Above `GTK_STYLE_PROVIDER_PRIORITY_USER` (800), where `gtk.css` lives.
const PRIORITY: u32 = gtk::STYLE_PROVIDER_PRIORITY_USER + 10;

const HYPR_EVENTS: &[&str] = &[
    "workspace",
    "workspacev2",
    "movewindow",
    "movewindowv2",
    "openwindow",
    "moveworkspace",
    "moveworkspacev2",
    "focusedmon",
];

#[derive(Clone, PartialEq)]
struct Palette {
    dark: bool,
    colours: HashMap<String, String>,
}

thread_local! {
    static PROVIDER: gtk::CssProvider = gtk::CssProvider::new();
}

pub fn install() {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    PROVIDER.with(|p| gtk::style_context_add_provider_for_display(&display, p, PRIORITY));
}

/// Resolve now and re-resolve whenever the window may have changed activity
/// or Caelestia's configuration changes.
pub fn follow_window(window: &adw::ApplicationWindow) {
    let dirty = Arc::new(AtomicBool::new(true));
    spawn_hyprland_listener(Arc::clone(&dirty));

    let mut monitors = Vec::new();
    for path in watched_files() {
        let file = gio::File::for_path(&path);
        if let Ok(m) = file.monitor_file(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) {
            let dirty = Arc::clone(&dirty);
            m.connect_changed(move |_, _, _, _| dirty.store(true, Ordering::Relaxed));
            monitors.push(m);
        }
    }

    window.connect_map({
        let dirty = Arc::clone(&dirty);
        move |_| dirty.store(true, Ordering::Relaxed)
    });

    let inflight = std::rc::Rc::new(std::cell::Cell::new(false));
    let last: std::rc::Rc<std::cell::RefCell<Option<Palette>>> = Default::default();
    // First paint as soon as the event loop is idle, then poll for Hyprland /
    // Caelestia changes. 200 ms is enough to coalesce bursts of socket events.
    glib::timeout_add_local(Duration::from_millis(200), move || {
        let _keep_alive = &monitors;
        if inflight.get() || !dirty.swap(false, Ordering::Relaxed) {
            return glib::ControlFlow::Continue;
        }
        inflight.set(true);
        let inflight = inflight.clone();
        let last = last.clone();
        crate::hid_async::spawn_hid(resolve, move |palette| {
            inflight.set(false);
            let Some(palette) = palette else {
                return;
            };
            if last.borrow().as_ref() == Some(&palette) {
                return;
            }
            apply(&palette);
            *last.borrow_mut() = Some(palette);
        });
        glib::ControlFlow::Continue
    });
}

fn apply(p: &Palette) {
    adw::StyleManager::default().set_color_scheme(if p.dark {
        adw::ColorScheme::ForceDark
    } else {
        adw::ColorScheme::ForceLight
    });
    let css = palette_css(p);
    PROVIDER.with(|provider| provider.load_from_string(&css));
}

fn palette_css(p: &Palette) -> String {
    let c = |key: &str, fallback: &str| -> String {
        let hex = p
            .colours
            .get(key)
            .map(String::as_str)
            .unwrap_or(fallback)
            .trim_start_matches('#');
        format!("#{hex}")
    };
    let surface = c("surface", "fafafa");
    let fg = c("onSurface", "1f1f1f");
    let (window, view, card, popover, dialog) = if p.dark {
        (
            surface.clone(),
            c("surfaceContainerLow", &surface),
            c("surfaceContainer", &surface),
            c("surfaceContainerHigh", &surface),
            c("surfaceContainerLow", &surface),
        )
    } else {
        // Light Adwaita: grey window, white cards.
        (
            c("surfaceContainer", &surface),
            c("surfaceContainerLowest", &surface),
            c("surfaceContainerLowest", &surface),
            c("surfaceContainerLowest", &surface),
            c("surfaceContainer", &surface),
        )
    };
    let primary = c("primary", "3584e4");
    let on_primary = c("onPrimary", "ffffff");
    let error = c("error", "e01b24");
    let on_error = c("onError", "ffffff");
    let outline = c("outlineVariant", "cccccc");

    let vars: [(&str, &str); 22] = [
        ("accent-color", &primary),
        ("accent-bg-color", &primary),
        ("accent-fg-color", &on_primary),
        ("destructive-color", &error),
        ("destructive-bg-color", &error),
        ("destructive-fg-color", &on_error),
        ("window-bg-color", &window),
        ("window-fg-color", &fg),
        ("view-bg-color", &view),
        ("view-fg-color", &fg),
        ("headerbar-bg-color", &window),
        ("headerbar-fg-color", &fg),
        ("headerbar-backdrop-color", &window),
        ("sidebar-bg-color", &window),
        ("sidebar-fg-color", &fg),
        ("card-bg-color", &card),
        ("card-fg-color", &fg),
        ("popover-bg-color", &popover),
        ("popover-fg-color", &fg),
        ("dialog-bg-color", &dialog),
        ("dialog-fg-color", &fg),
        ("thumbnail-bg-color", &card),
    ];

    let mut css = String::new();
    for (name, value) in vars {
        css.push_str(&format!("@define-color {} {value};\n", name.replace('-', "_")));
    }
    css.push_str(&format!("@define-color borders {outline};\n:root {{\n"));
    for (name, value) in vars {
        css.push_str(&format!("  --{name}: {value};\n"));
    }
    css.push_str(&format!("  --border-color: {outline};\n}}\n"));
    css
}

// ---------------------------------------------------------------------------
// Resolution (runs on a worker thread)

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_default()
}

fn state_dir() -> PathBuf {
    dirs::state_dir()
        .unwrap_or_else(|| home().join(".local/state"))
        .join("caelestia")
}

fn shell_config() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| home().join(".config"))
        .join("caelestia/shell.json")
}

fn watched_files() -> Vec<PathBuf> {
    vec![
        state_dir().join("scheme.json"),
        state_dir().join("light-apps.json"),
        state_dir().join("wallpaper/path.txt"),
        shell_config(),
    ]
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn hyprctl(args: &[&str]) -> Option<Value> {
    let out = Command::new("hyprctl").args(args).output().ok()?;
    serde_json::from_slice(&out.stdout).ok()
}

/// Workspace holding our window, or the focused one while we are hidden.
fn window_workspace() -> Option<i64> {
    let pid = std::process::id() as i64;
    let ours = hyprctl(&["clients", "-j"]).and_then(|clients| {
        let list = clients.as_array()?;
        let ws = |c: &Value| c["workspace"]["id"].as_i64();
        list.iter()
            .find(|c| c["pid"].as_i64() == Some(pid))
            .and_then(ws)
            .or_else(|| {
                list.iter()
                    .find(|c| {
                        matches!(c["class"].as_str(), Some("com.synapsex.Synapsex" | "synapsex"))
                    })
                    .and_then(ws)
            })
    });
    ours.or_else(|| hyprctl(&["activeworkspace", "-j"])?["id"].as_i64())
}

fn str_field<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

fn tri_field(v: &Value, key: &str) -> i64 {
    v.get(key).and_then(Value::as_i64).unwrap_or(-1)
}

fn pick<'a>(a: &'a str, b: &'a str, c: &'a str) -> &'a str {
    [a, b].into_iter().find(|s| !s.is_empty()).unwrap_or(c)
}

fn resolve() -> Option<Palette> {
    let current = read_json(&state_dir().join("scheme.json"))?;
    let light_apps_default = read_json(&state_dir().join("light-apps.json"))
        .and_then(|v| v["lightApps"].as_bool())
        .unwrap_or(false);

    let Some(shell) = read_json(&shell_config()) else {
        return palette_for_mode(&current, if light_apps_default { "light" } else { str_field(&current, "mode") });
    };
    let activities = &shell["activities"];
    if activities["enabled"].as_bool() == Some(false) {
        return palette_for_mode(&current, if light_apps_default { "light" } else { str_field(&current, "mode") });
    }
    let list = activities["activities"].as_array().cloned().unwrap_or_default();
    let ws = window_workspace()?;
    if ws <= 0 {
        // Special or unknown workspace: keep whatever is shown.
        return None;
    }

    let mut next = 1;
    let mut activity = list.first();
    for a in &list {
        let count = a["workspaceCount"].as_i64().unwrap_or(1).max(1);
        if ws >= next && ws < next + count {
            activity = Some(a);
            break;
        }
        next += count;
    }
    let Some(a) = activity else {
        return palette_for_mode(&current, if light_apps_default { "light" } else { str_field(&current, "mode") });
    };
    let null = Value::Null;
    let p = list.iter().find(|x| str_field(x, "id") == "principal").unwrap_or(&null);

    let wallpaper_fallback = std::fs::read_to_string(state_dir().join("wallpaper/path.txt"))
        .unwrap_or_default();
    let current_mode = str_field(&current, "mode");
    let wallpaper = pick(str_field(a, "wallpaper"), str_field(p, "wallpaper"), wallpaper_fallback.trim());
    let scheme = pick(str_field(a, "scheme"), str_field(p, "scheme"), pick(str_field(&current, "name"), "", "dynamic"));
    let flavour = pick(str_field(a, "flavour"), str_field(p, "flavour"), pick(str_field(&current, "flavour"), "", "default"));
    let variant = pick(str_field(a, "variant"), str_field(p, "variant"), pick(str_field(&current, "variant"), "", "tonalspot"));
    let mode = pick(str_field(a, "mode"), str_field(p, "mode"), pick(current_mode, "", "dark"));

    let light_apps = [tri_field(a, "lightApps"), tri_field(p, "lightApps")]
        .into_iter()
        .find(|v| *v >= 0)
        .unwrap_or(light_apps_default as i64);
    // Apps with lightApps stay light even when the activity wallpaper is dark.
    let app_mode = if light_apps == 1 { "light" } else { mode };

    let colours = if scheme == "dynamic" {
        dynamic_colours(Path::new(wallpaper), variant, flavour, app_mode)
    } else {
        named_colours(scheme, flavour, app_mode)
    };
    match colours {
        Some(colours) => Some(Palette {
            dark: app_mode != "light",
            colours,
        }),
        None => palette_for_mode(&current, app_mode),
    }
}

/// Prefer the wallpaper palette in `app_mode`. If Caelestia has not generated
/// that variant yet, still honour light/dark so we never paint dark widgets
/// on a light-apps activity (or the reverse).
fn palette_for_mode(current: &Value, app_mode: &str) -> Option<Palette> {
    let colours = current["colours"]
        .as_object()
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();
    let scheme_mode = str_field(current, "mode");
    if !colours.is_empty() && scheme_mode == app_mode {
        return Some(Palette {
            dark: app_mode != "light",
            colours,
        });
    }
    // Wrong-mode colours would clash with ForceLight/ForceDark. Fall back to
    // Adwaita defaults in the requested mode (empty map → CSS fallbacks).
    Some(Palette {
        dark: app_mode != "light",
        colours: HashMap::new(),
    })
}

fn sha256_file(path: &Path) -> Option<String> {
    static CACHE: OnceLock<Mutex<HashMap<(PathBuf, SystemTime), String>>> = OnceLock::new();
    let mtime = std::fs::metadata(path).ok()?.modified().ok()?;
    let key = (path.to_path_buf(), mtime);
    let cache = CACHE.get_or_init(Default::default);
    if let Some(h) = cache.lock().ok()?.get(&key) {
        return Some(h.clone());
    }
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let hex: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    cache.lock().ok()?.insert(key, hex.clone());
    Some(hex)
}

/// Caelestia caches dynamic schemes by the hash of the wallpaper thumbnail.
fn dynamic_colours(wallpaper: &Path, variant: &str, flavour: &str, mode: &str) -> Option<HashMap<String, String>> {
    let cache = dirs::cache_dir()?.join("caelestia");
    let thumb = cache
        .join("wallpapers")
        .join(sha256_file(wallpaper)?)
        .join("thumbnail.jpg");
    let dir = cache.join("schemes").join(sha256_file(&thumb)?);
    let file = format!("{flavour}/{mode}.json");
    let path = Some(dir.join(variant).join(&file))
        .filter(|p| p.exists())
        .or_else(|| {
            // Variant not generated yet: any variant in the right mode beats the wrong mode.
            std::fs::read_dir(&dir)
                .ok()?
                .flatten()
                .map(|e| e.path().join(&file))
                .find(|p| p.exists())
        })?;
    let v = read_json(&path)?;
    Some(
        v.as_object()?
            .iter()
            .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
            .collect(),
    )
}

fn named_colours(scheme: &str, flavour: &str, mode: &str) -> Option<HashMap<String, String>> {
    let base = Path::new("/usr/lib")
        .read_dir()
        .ok()?
        .flatten()
        .map(|e| e.path().join("site-packages/caelestia/data/schemes"))
        .find(|p| p.is_dir())?;
    let text = std::fs::read_to_string(base.join(scheme).join(flavour).join(format!("{mode}.txt"))).ok()?;
    Some(
        text.lines()
            .filter_map(|l| {
                let (k, v) = l.split_once(' ')?;
                Some((k.to_string(), v.trim().to_string()))
            })
            .collect(),
    )
}

// ---------------------------------------------------------------------------
// Hyprland event socket

fn spawn_hyprland_listener(dirty: Arc<AtomicBool>) {
    let (Some(runtime), Ok(sig)) = (
        std::env::var_os("XDG_RUNTIME_DIR"),
        std::env::var("HYPRLAND_INSTANCE_SIGNATURE"),
    ) else {
        return;
    };
    let path = PathBuf::from(runtime).join("hypr").join(sig).join(".socket2.sock");
    std::thread::spawn(move || loop {
        if let Ok(stream) = UnixStream::connect(&path) {
            for line in BufReader::new(stream).lines() {
                let Ok(line) = line else {
                    break;
                };
                let event = line.split_once(">>").map_or(line.as_str(), |(e, _)| e);
                if HYPR_EVENTS.contains(&event) {
                    dirty.store(true, Ordering::Relaxed);
                }
            }
        }
        std::thread::sleep(Duration::from_secs(2));
    });
}
