//! The glow round the edge of the desktop: a transparent window over every app, open while the
//! setting is on and TK-Music's own window is. It never takes focus and lets every click through.

use std::cell::Cell;

use gpui::prelude::*;
use gpui::{
    AnyWindowHandle, App, Bounds, Global, WindowBackgroundAppearance, WindowBounds, WindowKind,
    WindowOptions, px,
};
use state::Sonora;
use views::DesktopEdge;

/// How much shorter than the screen the window is. Windows takes a topmost window that covers
/// the whole screen for a fullscreen app, which hides the taskbar and holds back notifications.
const SHORT_OF_SCREEN: f32 = 1.;

/// The edge's window while it is open.
#[derive(Default)]
struct Edge(Cell<Option<AnyWindowHandle>>);

impl Global for Edge {}

/// Opens and closes the edge as the setting and the main window come and go.
pub fn install(cx: &mut App) {
    cx.set_global(Edge::default());
    let settings = Sonora::global(cx).settings.clone();
    cx.observe(&settings, |_, cx| sync(cx)).detach();
    cx.on_window_closed(|cx, _| sync(cx)).detach();
    sync(cx);
}

/// Whether `window` is the edge's, which every check for "is a window open" has to skip.
pub fn owns(window: AnyWindowHandle, cx: &App) -> bool {
    cx.try_global::<Edge>()
        .and_then(|edge| edge.0.get())
        .is_some_and(|open| open == window)
}

/// Brings the edge in line with the setting. Called again whenever a window closes, so the edge
/// goes with the last of TK-Music's own windows rather than keeping the app alive.
pub fn sync(cx: &mut App) {
    let Some(open) = cx.try_global::<Edge>().map(|edge| edge.0.get()) else {
        return;
    };
    // a closed window may still be listed while its close is handled
    let open = open.filter(|handle| cx.windows().contains(handle));
    cx.global::<Edge>().0.set(open);
    let main = cx.windows().into_iter().any(|window| !owns(window, cx));
    let wanted = main && Sonora::global(cx).settings.read(cx).desktop_edge();

    match (wanted, open) {
        (true, None) => {
            let handle = open_edge(cx);
            cx.global::<Edge>().0.set(handle);
        }
        (false, Some(handle)) => {
            cx.global::<Edge>().0.set(None);
            handle
                .update(cx, |_, window, _| window.remove_window())
                .ok();
        }
        _ => {}
    }
}

fn open_edge(cx: &mut App) -> Option<AnyWindowHandle> {
    let display = cx.primary_display()?;
    let screen = display.bounds();
    let bounds = Bounds::new(
        screen.origin,
        gpui::size(screen.size.width, screen.size.height - px(SHORT_OF_SCREEN)),
    );
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            display_id: Some(display.id()),
            titlebar: None,
            focus: false,
            show: true,
            kind: WindowKind::PopUp,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            window_background: WindowBackgroundAppearance::Transparent,
            ..Default::default()
        },
        |window, cx| {
            pass_clicks_through(window);
            cx.new(DesktopEdge::new)
        },
    );
    match opened {
        Ok(handle) => Some(handle.into()),
        Err(error) => {
            log::warn!("edge: cannot open the desktop edge: {error}");
            None
        }
    }
}

/// Makes the window invisible to the mouse, so clicks land on whatever is under it.
#[cfg(windows)]
fn pass_clicks_through(window: &gpui::Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, LWA_ALPHA, SetLayeredWindowAttributes, SetWindowLongPtrW,
        WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TRANSPARENT,
    };

    let Ok(RawWindowHandle::Win32(handle)) =
        HasWindowHandle::window_handle(window).map(|handle| handle.as_raw())
    else {
        return log::warn!("edge: no window handle to let clicks through");
    };
    let hwnd = handle.hwnd.get() as windows_sys::Win32::Foundation::HWND;
    // SAFETY: the handle is the live window gpui just created, on its own thread.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let through = (WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE) as isize;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | through);
        SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
    }
}

/// Makes the window invisible to the mouse, so clicks land on whatever is under it.
#[cfg(target_os = "macos")]
fn pass_clicks_through(window: &gpui::Window) {
    use objc2_app_kit::NSView;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let Ok(RawWindowHandle::AppKit(handle)) =
        HasWindowHandle::window_handle(window).map(|handle| handle.as_raw())
    else {
        return log::warn!("edge: no window handle to let clicks through");
    };
    // SAFETY: the handle points at the live content view of the window gpui just created, and
    // this runs on the main thread.
    let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
    if let Some(window) = view.window() {
        window.setIgnoresMouseEvents(true);
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
fn pass_clicks_through(_window: &gpui::Window) {}
