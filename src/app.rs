use slint::{ComponentHandle, ModelRc};

use crate::{shortcuts, ui, window};

pub fn run() -> Result<(), slint::PlatformError> {
    select_backend()?;

    let app_window = ui::AppWindow::new()?;
    configure_window(&app_window);

    let shortcut_actions = shortcuts::default_shortcut_actions();
    app_window.set_shortcut_actions(ModelRc::from(shortcut_actions.clone()));
    app_window.set_autostart_enabled(true);

    shortcuts::bind_handlers(&app_window, shortcut_actions);
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
