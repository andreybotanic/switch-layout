use std::{cell::RefCell, io, rc::Rc};

use slint::{ComponentHandle, ModelRc, VecModel};

use crate::{settings, shortcuts, ui, window};

pub fn run() -> Result<(), slint::PlatformError> {
    select_backend()?;

    let settings_store = Rc::new(settings::SettingsStore::new().map_err(platform_error)?);
    let app_settings =
        Rc::new(RefCell::new(settings_store.load_or_initialize().map_err(platform_error)?));
    let app_window = ui::AppWindow::new()?;
    configure_window(&app_window);

    let shortcut_actions = Rc::new(VecModel::from(app_settings.borrow().to_shortcut_actions()));
    app_window.set_shortcut_actions(ModelRc::from(shortcut_actions.clone()));
    app_window.set_autostart_enabled(app_settings.borrow().autostart_enabled);

    shortcuts::bind_handlers(
        &app_window,
        shortcut_actions,
        Rc::clone(&app_settings),
        Rc::clone(&settings_store),
    );
    bind_autostart_handler(&app_window, Rc::clone(&app_settings), Rc::clone(&settings_store));
    window::bind_handlers(&app_window);

    app_window.run()
}

fn select_backend() -> Result<(), slint::PlatformError> {
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .select()
        .map_err(|error| slint::PlatformError::OtherError(error.to_string().into()))
}

fn configure_window(app_window: &ui::AppWindow) {
    app_window.set_app_name("SwitchLayout".into());
    app_window.set_app_version(format!("v{}", env!("CARGO_PKG_VERSION")).into());
}

fn bind_autostart_handler(
    window: &ui::AppWindow,
    settings: Rc<RefCell<settings::AppSettings>>,
    store: Rc<settings::SettingsStore>,
) {
    window.on_autostart_toggled({
        let window = window.as_weak();

        move |enabled| {
            let previous = {
                let mut settings = settings.borrow_mut();

                if settings.autostart_enabled == enabled {
                    return;
                }

                let previous = settings.autostart_enabled;
                settings.autostart_enabled = enabled;

                if let Err(_error) = store.save(&settings) {
                    settings.autostart_enabled = previous;
                    previous
                } else {
                    return;
                }
            };

            if let Some(window) = window.upgrade() {
                window.set_autostart_enabled(previous);
            }
        }
    });
}

fn platform_error(error: io::Error) -> slint::PlatformError {
    slint::PlatformError::OtherError(error.to_string().into())
}
