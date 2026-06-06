#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use slint::ComponentHandle;

mod ui {
    #![allow(missing_debug_implementations)]

    slint::include_modules!();
}

fn main() -> Result<(), slint::PlatformError> {
    let window = ui::AppWindow::new()?;

    window.run()
}
