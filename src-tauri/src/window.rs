//! Creates the main window from its `tauri.conf.json` entry (which has `create: false`), so the
//! macOS-only parts can be added in code instead of in a duplicated platform config.
//!
//! On macOS the window is transparent over the system's sidebar material (`NSVisualEffectView`),
//! the translucent, wallpaper-tinted backdrop Finder's sidebar uses. Windows and Linux keep an
//! opaque window: Linux has no system material to put behind a webview, and a transparent window
//! there would show the desktop unblurred.
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

    builder.build()?;
    Ok(())
}
