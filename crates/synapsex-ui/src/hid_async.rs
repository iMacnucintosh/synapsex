//! Run blocking HID work off the GTK main thread, then resume on the UI loop.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use gtk::glib;

/// Spawn `work` on a background thread; invoke `on_done` on the GTK main context.
pub fn spawn_hid<T, F, C>(work: F, on_done: C)
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
    C: FnOnce(T) + 'static,
{
    let (tx, rx) = mpsc::sync_channel::<T>(1);
    std::thread::spawn(move || {
        let _ = tx.send(work());
    });
    let state: Rc<RefCell<Option<(mpsc::Receiver<T>, C)>>> =
        Rc::new(RefCell::new(Some((rx, on_done))));
    glib::timeout_add_local(Duration::from_millis(16), move || {
        let mut slot = state.borrow_mut();
        let Some((rx, _)) = slot.as_mut() else {
            return glib::ControlFlow::Break;
        };
        match rx.try_recv() {
            Ok(value) => {
                if let Some((_, cb)) = slot.take() {
                    drop(slot);
                    cb(value);
                }
                glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
        }
    });
}
