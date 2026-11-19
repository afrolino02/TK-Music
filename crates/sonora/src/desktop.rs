//! The windows TK-Music puts over the rest of the desktop: the glow round the screen's edge and
//! the dock at its top. Both are open only while their setting is on and TK-Music's own window
//! is, so they never keep the app alive on their own.

use std::cell::Cell;
use std::time::{Duration, Instant};

use gpui::prelude::*;
use gpui::{
    AnyWindowHandle, App, Bounds, Global, Pixels, WindowBackgroundAppearance, WindowBounds,
    WindowKind, WindowOptions, point, px, size,
};
use state::Sonora;
use views::{DOCK_HEIGHT, DOCK_WIDTH, DesktopDock, DesktopEdge};

/// How much shorter than the screen the edge's window is. Windows takes a topmost window that
/// covers the whole screen for a fullscreen app, which hides the taskbar and holds back
/// notifications.
const SHORT_OF_SCREEN: Pixels = px(1.);
/// How far below the top of the usable screen the dock floats.
const DOCK_DROP: Pixels = px(10.);
/// How often the dock checks whether TK-Music went to the background or came back.
const DOCK_POLL: Duration = Duration::from_millis(400);
/// How long the dock stays after a pause, so it can be used to start the track again.
const DOCK_LINGER: Duration = Duration::from_secs(30);

/// The overlays' windows while they are open.
#[derive(Default)]
struct Overlays {
    edge: Cell<Option<AnyWindowHandle>>,
    dock: Cell<Option<AnyWindowHandle>>,
    /// When a track last played, which the dock lingers after.
    played: Cell<Option<Instant>>,
}

impl Global for Overlays {}

/// Opens and closes the overlays as their settings, the playback and the main window change.
pub fn install(cx: &mut App) {
    cx.set_global(Overlays::default());
    let settings = Sonora::global(cx).settings.clone();
    cx.observe(&settings, |_, cx| sync(cx)).detach();
    cx.on_window_closed(|cx, _| sync(cx)).detach();
    // Nothing tells the app when another app takes the focus, so the dock looks every so often.
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(DOCK_POLL).await;
            cx.update(sync);
        }
    })
    .detach();
    sync(cx);
}

/// Whether `window` is an overlay's, which every check for "is a window open" has to skip.
pub fn owns(window: AnyWindowHandle, cx: &App) -> bool {
    cx.try_global::<Overlays>().is_some_and(|overlays| {
        overlays.edge.get() == Some(window) || overlays.dock.get() == Some(window)
    })
}

/// Brings the overlays in line with what is wanted now. Called again whenever a window closes,
/// so they go with the last of TK-Music's own windows.
pub fn sync(cx: &mut App) {
    if cx.try_global::<Overlays>().is_none() {
        return;
    }
    let listed = cx.windows();
    let main: Vec<AnyWindowHandle> = listed
        .iter()
        .copied()
        .filter(|window| !owns(*window, cx))
        .collect();
    let settings = Sonora::global(cx).settings.read(cx);
    let (edge_on, dock_on) = (settings.desktop_edge(), settings.desktop_dock());
    let playback = Sonora::global(cx).playback.read(cx);
    let (playing, loaded) = (playback.wants_playing(), playback.track().is_some());

    let overlays = cx.global::<Overlays>();
    if playing {
        overlays.played.set(Some(Instant::now()));
    }
    let lately = overlays
        .played
        .get()
        .is_some_and(|played| played.elapsed() < DOCK_LINGER);
    let away = cx
        .active_window()
        .is_none_or(|active| !main.contains(&active));
    let up = !main.is_empty();

    keep(
        |overlays| &overlays.edge,
        up && edge_on,
        open_edge,
        &listed,
        cx,
    );
    keep(
        |overlays| &overlays.dock,
        up && dock_on && loaded && lately && away,
        open_dock,
        &listed,
        cx,
    );
}

/// Opens or closes one overlay so it is open exactly when `wanted`.
fn keep(
    slot: fn(&Overlays) -> &Cell<Option<AnyWindowHandle>>,
    wanted: bool,
    open: fn(&mut App) -> Option<AnyWindowHandle>,
    listed: &[AnyWindowHandle],
    cx: &mut App,
) {
    // a closed window may still be held while its close is handled
    let held = slot(cx.global::<Overlays>())
        .get()
        .filter(|handle| listed.contains(handle));
    slot(cx.global::<Overlays>()).set(held);
    match (wanted, held) {
        (true, None) => {
            let handle = open(cx);
            slot(cx.global::<Overlays>()).set(handle);
        }
        (false, Some(handle)) => {
            slot(cx.global::<Overlays>()).set(None);
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
        size(screen.size.width, screen.size.height - SHORT_OF_SCREEN),
    );
    let opened = cx.open_window(overlay(bounds, display.id()), |window, cx| {
        pass_clicks_through(window);
        cx.new(DesktopEdge::new)
    });
    match opened {
        Ok(handle) => Some(handle.into()),
        Err(error) => {
            log::warn!("desktop: cannot open the edge: {error}");
            None
        }
    }
}

fn open_dock(cx: &mut App) -> Option<AnyWindowHandle> {
    let display = cx.primary_display()?;
    let screen = display.visible_bounds();
    let bounds = Bounds::new(
        point(
            screen.origin.x + (screen.size.width - DOCK_WIDTH) / 2.,
            screen.origin.y + DOCK_DROP,
        ),
        size(DOCK_WIDTH, DOCK_HEIGHT),
    );
    let opened = cx.open_window(overlay(bounds, display.id()), |window, cx| {
        window.set_rem_size(ui::ActiveTheme::theme(cx).font_size);
        cx.new(DesktopDock::new)
    });
    match opened {
        Ok(handle) => Some(handle.into()),
        Err(error) => {
            log::warn!("desktop: cannot open the dock: {error}");
            None
        }
    }
}

/// A borderless, see-through window above every other that never takes the focus on opening.
fn overlay(bounds: Bounds<Pixels>, display: gpui::DisplayId) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        display_id: Some(display),
        titlebar: None,
        focus: false,
        show: true,
        kind: WindowKind::PopUp,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
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
        return log::warn!("desktop: no window handle to let clicks through");
    };
    let hwnd = handle.hwnd.get() as *mut std::ffi::c_void;
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
        return log::warn!("desktop: no window handle to let clicks through");
    };
    // SAFETY: the handle points at the live content view of the window gpui just created, and
    // this runs on the main thread.
    let view: &NSView = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
    if let Some(window) = view.window() {
        window.setIgnoresMouseEvents(true);
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
fn pass_clicks_through(_window: &gpui::Window) {}
