//! Window assembly: header, pages, device refresh and the onboard poll.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use adw::prelude::*;
use gtk::glib;
use razer_device::{DeviceSession, MouseState};
use synapsex_i18n as i18n;

use crate::ctx::Ctx;
use crate::header::Header;
use crate::pages::buttons::ButtonsPage;
use crate::pages::lighting::LightingPage;
use crate::pages::performance::PerformancePage;

struct Pages {
    buttons: Rc<ButtonsPage>,
    performance: Rc<PerformancePage>,
    lighting: Rc<LightingPage>,
}

pub fn build_ui(app: &adw::Application) {
    crate::color::install_css();
    crate::theme::install();

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title(i18n::app_title())
        .default_width(880)
        .default_height(720)
        .build();
    let toast_overlay = adw::ToastOverlay::new();
    let ctx = Ctx::new(&window, &toast_overlay);

    let header = Header::new(&ctx);
    let pages = Rc::new(Pages {
        buttons: ButtonsPage::new(&ctx),
        performance: PerformancePage::new(&ctx),
        lighting: LightingPage::new(&ctx),
    });

    let stack = adw::ViewStack::new();
    stack.set_vexpand(true);
    stack.add_titled_with_icon(
        &pages.buttons.widget,
        Some("buttons"),
        i18n::tab_buttons(),
        "input-mouse-symbolic",
    );
    stack.add_titled_with_icon(
        &pages.performance.widget,
        Some("performance"),
        i18n::tab_performance(),
        "speedometer-symbolic",
    );
    stack.add_titled_with_icon(
        &pages.lighting.widget,
        Some("lighting"),
        i18n::tab_lighting(),
        "display-brightness-symbolic",
    );

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header.bar);

    let switcher = adw::ViewSwitcher::builder()
        .stack(&stack)
        .policy(adw::ViewSwitcherPolicy::Wide)
        .halign(gtk::Align::Center)
        .css_classes(["synapsex-switcher"])
        .build();

    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.append(&switcher);
    stack.set_vexpand(true);
    body.append(&stack);

    let loading_page = adw::StatusPage::builder()
        .title(i18n::looking_for_mouse())
        .description(i18n::loading_placeholder_body())
        .vexpand(true)
        .build();
    let spinner = adw::SpinnerPaintable::new(Some(&loading_page));
    loading_page.set_paintable(Some(&spinner));

    let error_page = adw::StatusPage::builder()
        .icon_name("input-mouse-symbolic")
        .title(i18n::app_title())
        .vexpand(true)
        .build();
    let retry = gtk::Button::builder()
        .label(i18n::retry())
        .halign(gtk::Align::Center)
        .css_classes(["pill", "suggested-action"])
        .build();
    error_page.set_child(Some(&retry));
    retry.connect_clicked({
        let ctx = Rc::clone(&ctx);
        move |_| ctx.refresh()
    });

    let shell = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .vexpand(true)
        .build();
    shell.add_named(&loading_page, Some("loading"));
    shell.add_named(&error_page, Some("error"));
    shell.add_named(&body, Some("ready"));
    shell.set_visible_child_name("loading");
    toolbar.set_content(Some(&shell));

    let switcher_bar = adw::ViewSwitcherBar::builder().stack(&stack).build();
    switcher_bar.set_visible(false);
    toolbar.add_bottom_bar(&switcher_bar);

    let narrow = adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 600sp").unwrap());
    narrow.add_setter(&switcher_bar, "reveal", Some(&true.to_value()));
    narrow.add_setter(&switcher, "visible", Some(&false.to_value()));
    window.add_breakpoint(narrow);
    toast_overlay.set_child(Some(&toolbar));
    window.set_content(Some(&toast_overlay));

    ctx.set_refresh(refresh_fn(
        &ctx,
        &header,
        &pages,
        &shell,
        &error_page,
        &switcher_bar,
    ));
    start_poll(&ctx, &pages);

    window.connect_close_request({
        let keep = header.keep_background.clone();
        move |window| {
            if keep.state().and_then(|v| v.get::<bool>()).unwrap_or(false) {
                window.set_visible(false);
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        }
    });

    crate::theme::follow_window(&window);
    window.present();
    ctx.refresh();
}

/// Full read of the active profile, then render every page from it.
fn refresh_fn(
    ctx: &Rc<Ctx>,
    header: &Rc<Header>,
    pages: &Rc<Pages>,
    shell: &gtk::Stack,
    error_page: &adw::StatusPage,
    switcher_bar: &adw::ViewSwitcherBar,
) -> Rc<dyn Fn()> {
    let ctx = Rc::downgrade(ctx);
    let header = Rc::clone(header);
    let pages = Rc::clone(pages);
    let shell = shell.clone();
    let error_page = error_page.clone();
    let switcher_bar = switcher_bar.clone();
    let ready = Rc::new(Cell::new(false));
    Rc::new(move || {
        let Some(ctx) = ctx.upgrade() else {
            return;
        };
        if ctx.busy.get() {
            return;
        }
        ctx.busy.set(true);
        header.set_loading();
        if !ready.get() {
            shell.set_visible_child_name("loading");
        }

        let session = Arc::clone(&ctx.session);
        let header = Rc::clone(&header);
        let pages = Rc::clone(&pages);
        let shell = shell.clone();
        let error_page = error_page.clone();
        let switcher_bar = switcher_bar.clone();
        let ready = Rc::clone(&ready);
        crate::hid_async::spawn_hid(
            move || -> Result<MouseState, String> {
                let mut guard = session.lock().map_err(|e| e.to_string())?;
                if guard.is_none() {
                    *guard = Some(DeviceSession::open().map_err(|e| e.to_string())?);
                }
                guard.as_mut().unwrap().read_state().map_err(|e| e.to_string())
            },
            move |result| {
                ctx.busy.set(false);
                match result {
                    Ok(st) => {
                        header.apply(&st);
                        pages.performance.apply(&st);
                        pages.lighting.apply(&st);
                        *ctx.state.borrow_mut() = Some(st);
                        pages.buttons.render(&ctx);
                        ready.set(true);
                        switcher_bar.set_visible(true);
                        shell.set_visible_child_name("ready");
                    }
                    Err(e) => {
                        header.set_error(&e);
                        if !ready.get() {
                            error_page.set_description(Some(&i18n::no_mouse(&e)));
                            shell.set_visible_child_name("error");
                        }
                    }
                }
            },
        );
    })
}

type PollResult = (bool, Option<(u16, usize, usize)>, u8);

/// Follow profile and DPI changes made on the mouse itself. Never waits on
/// the session lock: if another HID job holds it, this tick is skipped.
fn start_poll(ctx: &Rc<Ctx>, pages: &Rc<Pages>) {
    let ctx = Rc::clone(ctx);
    let pages = Rc::clone(pages);
    let last_slot = Rc::new(Cell::new(0u8));
    let inflight = Rc::new(Cell::new(false));
    glib::timeout_add_local(Duration::from_millis(400), move || {
        if ctx.busy.get() || inflight.get() {
            return glib::ControlFlow::Continue;
        }
        let (prev_dpi, prev_stage) = match ctx.state.borrow().as_ref() {
            Some(st) => (st.dpi_x, st.active_dpi_stage),
            None => return glib::ControlFlow::Continue,
        };
        inflight.set(true);
        let prev_slot = last_slot.get();
        let session = Arc::clone(&ctx.session);
        let ctx = Rc::clone(&ctx);
        let pages = Rc::clone(&pages);
        let last_slot = Rc::clone(&last_slot);
        let inflight = Rc::clone(&inflight);
        crate::hid_async::spawn_hid(
            move || -> Result<PollResult, String> {
                let Ok(mut guard) = session.try_lock() else {
                    return Ok((false, None, prev_slot));
                };
                let Some(s) = guard.as_mut() else {
                    return Ok((false, None, prev_slot));
                };
                let changed = matches!(s.poll_slot_change(), Ok(true));
                let dpi = if changed {
                    None
                } else {
                    s.poll_dpi_change(prev_dpi, prev_stage).ok().flatten()
                };
                Ok((changed, dpi, s.active_slot))
            },
            move |result| {
                inflight.set(false);
                let Ok((changed, dpi, active)) = result else {
                    return;
                };
                last_slot.set(active);
                if changed {
                    ctx.toast(i18n::toast_profile_changed());
                    ctx.refresh();
                } else if let Some((dpi, stage, count)) = dpi {
                    pages.performance.show_live_dpi(dpi, stage, count);
                    if let Some(st) = ctx.state.borrow_mut().as_mut() {
                        st.dpi_x = dpi;
                        st.dpi_y = dpi;
                        st.active_dpi_stage = stage;
                    }
                }
            },
        );
        glib::ControlFlow::Continue
    });
}
