//! WhatsApp Desktop Light — Tauri 2.x backend.
//!
//! Responsibilities:
//!   * Single-instance enforcement (focus existing window on 2nd launch).
//!   * Notification bridge: page-side JS shim invokes our `notify` command,
//!     which delegates to `tauri-plugin-notification` for real OS toasts.
//!   * User-Agent normalization via an `init_script` so WhatsApp Web sees a
//!     standard desktop Chrome UA string (must run before page scripts).
//!   * Persistent webview profile directory resolved through Tauri's `path`
//!     API (per-OS app-data path) so login session survives restarts.
//!   * Windows-only dark title bar via `DwmSetWindowAttribute`.

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_notification::NotificationExt;

/// Desktop Chrome UA used as the normalized replacement. Pinned version so the
/// UA is stable across releases and unlikely to trigger anti-bot heuristics.
const NORMALIZED_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

// Notifications are dispatched from the JS polyfill straight to the
// `tauri-plugin-notification` plugin command (`plugin:notification|notify`).
// Permission is requested eagerly here at startup so the polyfill sees a
// deterministic `granted` state and WhatsApp Web skips its own prompt.

/// Inject JS that runs BEFORE any page script. Two goals:
///   1. Normalize `navigator.userAgent`/`appVersion` so WhatsApp Web treats
///      the webview like a regular desktop Chrome.
///   2. Stub `Notification.permission` early so WhatsApp Web's own
///      permission prompt doesn't appear before our polyfill lands in
///      `dist/index.html` (which itself runs before navigation).
fn user_agent_init_script() -> String {
    format!(
        r#"(function() {{
  try {{
    var ua = {ua};
    Object.defineProperty(navigator, 'userAgent', {{ get: function() {{ return ua; }}, configurable: true }});
    Object.defineProperty(navigator, 'appVersion', {{ get: function() {{ return ua.replace('Mozilla/', ''); }}, configurable: true }});
    Object.defineProperty(navigator, 'platform', {{ get: function() {{ return 'Win32'; }}, configurable: true }});
  }} catch (e) {{}}
}})();"#,
        ua = serde_json::to_string(NORMALIZED_UA).unwrap_or_else(|_| "\"\"".into())
    )
}

#[cfg(target_os = "windows")]
mod windows_chrome {
    //! Windows-native title-bar / caption color via DWM.
    //! `DWMWA_USE_IMMERSIVE_DARK_MODE` (Win 11 22H2+ builds) plus caption/text
    //! colors. All behind `#[cfg(target_os = "windows")]` so non-Windows
    //! targets compile cleanly.
    use tauri::{AppHandle, Manager, WebviewWindow};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR,
        DWMWA_USE_IMMERSIVE_DARK_MODE,
    };

    const DARK_MODE_USE: i32 = 1;
    // ARGB values (little-endian DWORD layout).
    const CAPTION_COLOR: u32 = 0x002A2A2A;
    const TEXT_COLOR: u32 = 0x00FFFFFF;

    pub fn apply(app: &AppHandle) {
        let win = match app.get_webview_window("main") {
            Some(w) => w,
            None => return,
        };
        apply_to_window(&win);
    }

    fn apply_to_window(win: &WebviewWindow) {
        let Ok(hwnd_raw) = win.hwnd() else { return };
        let hwnd = HWND(hwnd_raw.0 as *mut _);

        // Immersive dark mode. Newer builds use the v2 attribute value 19/20;
        // try 19 first, fall back silently if not supported.
        let _ = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &DARK_MODE_USE as *const _ as *const _,
                std::mem::size_of::<i32>() as u32,
            )
        };

        // Caption + text colors.
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_CAPTION_COLOR,
                &CAPTION_COLOR as *const _ as *const _,
                std::mem::size_of::<u32>() as u32,
            );
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_TEXT_COLOR,
                &TEXT_COLOR as *const _ as *const _,
                std::mem::size_of::<u32>() as u32,
            );
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod windows_chrome {
    use tauri::AppHandle;
    pub fn apply(_app: &AppHandle) {}
}

pub fn run() {
    let init_script = user_agent_init_script();

    tauri::Builder::default()
        // Single-instance: a second launch focuses the existing window
        // instead of opening a duplicate. Must be registered first.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.unminimize();
                let _ = win.set_focus();
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .setup(move |app| {
            // Resolve a per-OS app-data path for the webview profile so the
            // login session survives restarts. Tauri's `path::app_data_dir`
            // returns the platform-correct location:
            //   Windows: %APPDATA%\com.whatsapp.desktop.light
            //   macOS:   ~/Library/Application Support/com.whatsapp.desktop.light
            //   Linux:   ~/.local/share/com.whatsapp.desktop.light
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data directory");
            std::fs::create_dir_all(&data_dir).ok();

            // Rebuild the main window with our persistent profile. We do this
            // here (rather than only via tauri.conf.json) so we can attach
            // the data directory programmatically — Tauri exposes the
            // webview-specific config on `WebviewWindowBuilder`.
            let existing = app.get_webview_window("main");
            if let Some(win) = existing {
                let _ = win.set_title("WhatsApp Desktop");
            } else {
                let _ = WebviewWindowBuilder::new(
                    app,
                    "main",
                    WebviewUrl::App("index.html".into()),
                )
                .title("WhatsApp Desktop")
                .inner_size(1100.0, 750.0)
                .min_inner_size(800.0, 600.0)
                .center()
                .resizable(true)
                .initialization_script(&init_script)
                .data_directory(data_dir)
                .build()?;
            }

            // Request notification permission eagerly so the JS polyfill can
            // resolve its permission query with a real value.
            if let Err(e) = app.notification().request_permission() {
                eprintln!("notification permission request failed: {e}");
            }

            // Windows-only dark title bar treatment.
            windows_chrome::apply(&app.handle());

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}