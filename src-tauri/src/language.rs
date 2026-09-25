//! The language of the parts macOS draws for the app: the menu bar, and the open and save panels.
//!
//! The webview chooses the language (src/state/language.ts) and sends it here with the menu's
//! words already translated, from its own catalogs, so there is one set of translations and it
//! lives in `src/locales`. The menu is built again from macOS's own items (Copy, Hide, Quit…), so
//! each keeps its action and shortcut and only its words change. The menus macOS fills in itself
//! (Services, the Edit menu's dictation and emoji items, Enter Full Screen) and the open panel
//! follow the language the app runs in, which the bundle declares (`CFBundleLocalizations` in
//! Info.plist). A language chosen in the app is kept as the app's own `AppleLanguages`, the same
//! setting System Settings' per-app language writes, so those follow it from the next launch.
//!
//! Windows and Linux have no menu bar here, and their file dialogs follow the system's language,
//! so on them this does nothing.

use serde::Deserialize;

/// The languages the app speaks, as the webview names them (src/i18n/locales.ts), each with the
/// language macOS knows it by.
const LANGUAGES: [(&str, &str); 6] = [
    ("en", "en"),
    ("zh-CN", "zh-Hans"),
    ("ja", "ja"),
    ("ko", "ko"),
    ("fr", "fr"),
    ("es", "es"),
];

/// The language macOS knows `language` by, or `None` for one the app doesn't speak.
pub fn apple_language(language: &str) -> Option<&'static str> {
    LANGUAGES
        .iter()
        .find(|(ours, _)| *ours == language)
        .map(|(_, apple)| *apple)
}

/// The menu bar's words in the chosen language (the `menu` namespace of the catalogs). Anything
/// the webview leaves out stays in English.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MenuWords {
    pub about: String,
    pub services: String,
    pub hide: String,
    pub hide_others: String,
    pub show_all: String,
    pub quit: String,
    pub file: String,
    pub close_window: String,
    pub edit: String,
    pub undo: String,
    pub redo: String,
    pub cut: String,
    pub copy: String,
    pub paste: String,
    pub select_all: String,
    pub view: String,
    pub full_screen: String,
    pub window: String,
    pub minimize: String,
    pub zoom: String,
    pub help: String,
}

impl Default for MenuWords {
    fn default() -> Self {
        let s = |t: &str| t.to_string();
        MenuWords {
            about: s("About FolderSkin"),
            services: s("Services"),
            hide: s("Hide FolderSkin"),
            hide_others: s("Hide Others"),
            show_all: s("Show All"),
            quit: s("Quit FolderSkin"),
            file: s("File"),
            close_window: s("Close Window"),
            edit: s("Edit"),
            undo: s("Undo"),
            redo: s("Redo"),
            cut: s("Cut"),
            copy: s("Copy"),
            paste: s("Paste"),
            select_all: s("Select All"),
            view: s("View"),
            full_screen: s("Enter Full Screen"),
            window: s("Window"),
            minimize: s("Minimize"),
            zoom: s("Zoom"),
            help: s("Help"),
        }
    }
}

impl MenuWords {
    /// The same words with any left empty back in English: a menu item never goes blank.
    fn or_english(self) -> MenuWords {
        let en = MenuWords::default();
        let pick = |ours: String, theirs: String| if ours.trim().is_empty() { theirs } else { ours };
        MenuWords {
            about: pick(self.about, en.about),
            services: pick(self.services, en.services),
            hide: pick(self.hide, en.hide),
            hide_others: pick(self.hide_others, en.hide_others),
            show_all: pick(self.show_all, en.show_all),
            quit: pick(self.quit, en.quit),
            file: pick(self.file, en.file),
            close_window: pick(self.close_window, en.close_window),
            edit: pick(self.edit, en.edit),
            undo: pick(self.undo, en.undo),
            redo: pick(self.redo, en.redo),
            cut: pick(self.cut, en.cut),
            copy: pick(self.copy, en.copy),
            paste: pick(self.paste, en.paste),
            select_all: pick(self.select_all, en.select_all),
            view: pick(self.view, en.view),
            full_screen: pick(self.full_screen, en.full_screen),
            window: pick(self.window, en.window),
            minimize: pick(self.minimize, en.minimize),
            zoom: pick(self.zoom, en.zoom),
            help: pick(self.help, en.help),
        }
    }
}

/// Puts the menu bar in `language`, with `menu`'s words. With `pin`, the person chose the
/// language in the app, so macOS's own panels and menu items follow it from the next launch.
#[tauri::command]
pub fn set_language(
    app: tauri::AppHandle,
    language: String,
    menu: MenuWords,
    pin: bool,
) -> Result<(), String> {
    let Some(apple) = apple_language(&language) else {
        return Err(format!("FolderSkin doesn't speak {language:?}"));
    };
    let words = menu.or_english();
    #[cfg(target_os = "macos")]
    {
        let menu = build_menu(&app, &words).map_err(|e| e.to_string())?;
        app.set_menu(menu).map_err(|e| e.to_string())?;
        if pin {
            pin_language(apple);
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, words, pin, apple);
    Ok(())
}

/// Tauri's own menu bar for macOS (`Menu::default`), item for item, with `words` for its words.
#[cfg(target_os = "macos")]
fn build_menu(
    app: &tauri::AppHandle,
    words: &MenuWords,
) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{
        AboutMetadata, Menu, PredefinedMenuItem as Item, Submenu, HELP_SUBMENU_ID,
        WINDOW_SUBMENU_ID,
    };
    let info = app.package_info();
    let config = app.config();
    let about = AboutMetadata {
        name: Some(info.name.clone()),
        version: Some(info.version.to_string()),
        copyright: config.bundle.copyright.clone(),
        authors: config.bundle.publisher.clone().map(|p| vec![p]),
        ..Default::default()
    };
    let w = words;
    let app_menu = Submenu::with_items(
        app,
        info.name.clone(),
        true,
        &[
            &Item::about(app, Some(&w.about), Some(about))?,
            &Item::separator(app)?,
            &Item::services(app, Some(&w.services))?,
            &Item::separator(app)?,
            &Item::hide(app, Some(&w.hide))?,
            &Item::hide_others(app, Some(&w.hide_others))?,
            &Item::show_all(app, Some(&w.show_all))?,
            &Item::separator(app)?,
            &Item::quit(app, Some(&w.quit))?,
        ],
    )?;
    let file = Submenu::with_items(
        app,
        &w.file,
        true,
        &[&Item::close_window(app, Some(&w.close_window))?],
    )?;
    let edit = Submenu::with_items(
        app,
        &w.edit,
        true,
        &[
            &Item::undo(app, Some(&w.undo))?,
            &Item::redo(app, Some(&w.redo))?,
            &Item::separator(app)?,
            &Item::cut(app, Some(&w.cut))?,
            &Item::copy(app, Some(&w.copy))?,
            &Item::paste(app, Some(&w.paste))?,
            &Item::select_all(app, Some(&w.select_all))?,
        ],
    )?;
    let view = Submenu::with_items(
        app,
        &w.view,
        true,
        &[&Item::fullscreen(app, Some(&w.full_screen))?],
    )?;
    let window = Submenu::with_id_and_items(
        app,
        WINDOW_SUBMENU_ID,
        &w.window,
        true,
        &[
            &Item::minimize(app, Some(&w.minimize))?,
            &Item::maximize(app, Some(&w.zoom))?,
            &Item::separator(app)?,
            &Item::close_window(app, Some(&w.close_window))?,
        ],
    )?;
    let help = Submenu::with_id_and_items(app, HELP_SUBMENU_ID, &w.help, true, &[])?;
    Menu::with_items(app, &[&app_menu, &file, &edit, &view, &window, &help])
}

/// Keeps `apple` as this app's language (`defaults write app.folderskin.desktop AppleLanguages`),
/// which macOS reads as the app starts.
#[cfg(target_os = "macos")]
fn pin_language(apple: &str) {
    use objc2_foundation::{NSArray, NSString, NSUserDefaults};
    let languages = NSArray::from_retained_slice(&[NSString::from_str(apple)]);
    let defaults = NSUserDefaults::standardUserDefaults();
    // SAFETY: an array of strings is a property-list object, which is what a default may hold.
    unsafe { defaults.setObject_forKey(Some(&languages), &NSString::from_str("AppleLanguages")) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_the_app_speaks_has_a_name_macos_knows() {
        assert_eq!(apple_language("en"), Some("en"));
        assert_eq!(apple_language("zh-CN"), Some("zh-Hans"));
        assert_eq!(apple_language("ja"), Some("ja"));
        assert_eq!(apple_language("fr"), Some("fr"));
        assert_eq!(apple_language("de"), None);
        assert_eq!(apple_language(""), None);
    }

    #[test]
    fn words_the_webview_leaves_out_or_empty_stay_in_english() {
        let words: MenuWords =
            serde_json::from_str(r#"{"copy": "Copier", "paste": "  ", "selectAll": "Tout sélectionner"}"#)
                .unwrap();
        let words = words.or_english();
        assert_eq!(words.copy, "Copier");
        assert_eq!(words.select_all, "Tout sélectionner");
        assert_eq!(words.paste, "Paste");
        assert_eq!(words.quit, "Quit FolderSkin");
    }
}
