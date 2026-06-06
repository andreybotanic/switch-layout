use slint::ComponentHandle;
use slint::winit_030::WinitWindowAccessor;

use crate::ui;

pub(crate) fn bind_handlers(window: &ui::AppWindow) {
    bind_tab_handler(window);
    bind_drag_handler(window);
    bind_minimize_handler(window);
    bind_close_handler(window);
}

fn bind_tab_handler(window: &ui::AppWindow) {
    window.on_select_tab({
        let window = window.as_weak();

        move |tab| {
            let Some(window) = window.upgrade() else {
                return;
            };

            window.set_current_tab(tab);
        }
    });
}

fn bind_drag_handler(window: &ui::AppWindow) {
    window.on_begin_window_drag({
        let window = window.as_weak();

        move || {
            let Some(window) = window.upgrade() else {
                return;
            };

            let _ = window.window().with_winit_window(|winit_window| winit_window.drag_window());
        }
    });
}

fn bind_minimize_handler(window: &ui::AppWindow) {
    window.on_request_minimize({
        let window = window.as_weak();

        move || {
            let Some(window) = window.upgrade() else {
                return;
            };

            let _ =
                window.window().with_winit_window(|winit_window| winit_window.set_minimized(true));
        }
    });
}

fn bind_close_handler(window: &ui::AppWindow) {
    window.on_request_close({
        let window = window.as_weak();

        move || {
            let Some(window) = window.upgrade() else {
                return;
            };

            let _ = window.hide();
            let _ = slint::quit_event_loop();
        }
    });
}
