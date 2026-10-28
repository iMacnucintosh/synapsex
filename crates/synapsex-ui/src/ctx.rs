//! Shared UI context: device session, window, toasts and the refresh hook.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use razer_device::{DeviceSession, MouseState};
use synapsex_i18n as i18n;

pub type Session = Arc<Mutex<Option<DeviceSession>>>;

pub struct Ctx {
    pub session: Session,
    pub window: adw::ApplicationWindow,
    pub toast_overlay: adw::ToastOverlay,
    /// Last state read from the mouse; pages render from it.
    pub state: RefCell<Option<MouseState>>,
    /// Set while a full read or profile switch owns the HID link, so the
    /// background poll never queues behind it.
    pub busy: Cell<bool>,
    refresh_fn: RefCell<Option<Rc<dyn Fn()>>>,
}

impl Ctx {
    pub fn new(window: &adw::ApplicationWindow, toast_overlay: &adw::ToastOverlay) -> Rc<Self> {
        Rc::new(Self {
            session: Arc::new(Mutex::new(None)),
            window: window.clone(),
            toast_overlay: toast_overlay.clone(),
            state: RefCell::new(None),
            busy: Cell::new(false),
            refresh_fn: RefCell::new(None),
        })
    }

    pub fn set_refresh(&self, f: Rc<dyn Fn()>) {
        *self.refresh_fn.borrow_mut() = Some(f);
    }

    /// Re-read the active profile from the mouse and re-render every page.
    pub fn refresh(&self) {
        let f = self.refresh_fn.borrow().clone();
        if let Some(f) = f {
            f();
        }
    }

    pub fn toast(&self, msg: &str) {
        self.toast_overlay.add_toast(adw::Toast::new(msg));
    }

    /// Run `work` against the open session on a worker thread and hand the
    /// result back on the GTK thread.
    pub fn hid_then<T, W, F>(self: &Rc<Self>, work: W, on_done: F)
    where
        T: Send + 'static,
        W: FnOnce(&mut DeviceSession) -> anyhow::Result<T> + Send + 'static,
        F: FnOnce(&Rc<Ctx>, Result<T, String>) + 'static,
    {
        let session = Arc::clone(&self.session);
        let ctx = Rc::clone(self);
        crate::hid_async::spawn_hid(
            move || -> Result<T, String> {
                let mut guard = session.lock().map_err(|e| e.to_string())?;
                let s = guard.as_mut().ok_or_else(|| i18n::no_session().to_string())?;
                work(s).map_err(|e| e.to_string())
            },
            move |res| on_done(&ctx, res),
        );
    }

    /// Like [`Ctx::hid_then`], but errors become a toast and only success is forwarded.
    pub fn hid<T, W, F>(self: &Rc<Self>, work: W, on_ok: F)
    where
        T: Send + 'static,
        W: FnOnce(&mut DeviceSession) -> anyhow::Result<T> + Send + 'static,
        F: FnOnce(&Rc<Ctx>, T) + 'static,
    {
        self.hid_then(work, move |ctx, res| match res {
            Ok(v) => on_ok(ctx, v),
            Err(e) => ctx.toast(&i18n::toast_error(&e)),
        });
    }

    /// Write a setting, toast `ok_msg` and re-read the device.
    pub fn write_and_refresh<W>(self: &Rc<Self>, ok_msg: &'static str, work: W)
    where
        W: FnOnce(&mut DeviceSession) -> anyhow::Result<()> + Send + 'static,
    {
        self.hid(work, move |ctx, ()| {
            ctx.toast(ok_msg);
            ctx.refresh();
        });
    }
}
