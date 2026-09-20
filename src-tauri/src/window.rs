//! Creates the main window from its `tauri.conf.json` entry (which has `create: false`), so the
//! per-OS parts can be added in code instead of in a duplicated platform config.
//!
//! On macOS the window is transparent over the system's sidebar material (`NSVisualEffectView`),
//! the translucent, wallpaper-tinted backdrop Finder's sidebar uses. Windows and Linux keep an
//! opaque window: Linux has no system material to put behind a webview, and a transparent window
//! there would show the desktop unblurred.
//!
//! On Windows the native title bar is taken off entirely. `titleBarStyle: "Overlay"` and
//! `hiddenTitle` in `tauri.conf.json` are macOS-only, so Windows was the one platform left with a
//! system caption bar sitting above the islands — a grey strip the design never had room for. The
//! window is undecorated instead and the webview draws its own minimise, maximise and close
//! buttons in the folder island (`WindowControls.tsx`). tao keeps the resize frame and Aero Snap
//! on an undecorated window, so only the caption is gone.
//!
//! macOS 26's Liquid Glass (`NSGlassEffectView`) is not used yet: Tauri's own support is an open
//! draft with crash reports for packaged apps whose transparent windows use an overlay title bar,
//! which this window does. Revisit when Tauri ships it.

use tauri::{App, WebviewWindowBuilder};

pub fn create_main(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "main")
        .ok_or("tauri.conf.json has no window labelled \"main\"")?
        .clone();
    let builder = WebviewWindowBuilder::from_config(app.handle(), &config)?;

    #[cfg(target_os = "macos")]
    let builder = {
        use tauri::window::{Effect, EffectState, EffectsBuilder};
        builder
            .transparent(true)
            .effects(
                EffectsBuilder::new()
                    .effect(Effect::Sidebar)
                    .state(EffectState::FollowsWindowActiveState)
                    .build(),
            )
            // Tells the stylesheet to clear the backgrounds the material should show through,
            // before the first paint.
            .initialization_script("document.documentElement.dataset.glass = 'sidebar';")
    };

    // No system caption bar: the webview draws the window's own controls. The flag goes on
    // <html> before the first paint, so the island reserves room for them without a reflow.
    // WebView2 runs this when the document is created, which can be before <html> is parsed
    // (unlike WKWebView, where the macOS branch above can set it straight away), so it waits for
    // the element when it isn't there yet. App.tsx sets the same flag once platform_info answers,
    // which is what makes it right even if this never runs.
    #[cfg(target_os = "windows")]
    let builder = builder.decorations(false).initialization_script(
        "(function () { var set = function () { document.documentElement.dataset.os = 'windows'; };\
         if (document.documentElement) set();\
         else document.addEventListener('DOMContentLoaded', set); })();",
    );

    builder.build()?;
    Ok(())
}
