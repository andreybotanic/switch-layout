use std::rc::Rc;

use slint::{ComponentHandle, Model, SharedString, VecModel};

use crate::ui;

pub(crate) fn default_shortcut_actions() -> Rc<VecModel<ui::ShortcutAction>> {
    Rc::new(VecModel::from(vec![
        ui::ShortcutAction {
            label: "Последнее слово".into(), shortcut: "Ctrl + Alt + 1".into()
        },
        ui::ShortcutAction {
            label: "Весь текст".into(), shortcut: "Ctrl + Alt + 2".into()
        },
    ]))
}

pub(crate) fn bind_handlers(window: &ui::AppWindow, shortcuts: Rc<VecModel<ui::ShortcutAction>>) {
    bind_request_edit_handler(window, Rc::clone(&shortcuts));
    bind_cancel_edit_handler(window);
    bind_accept_edit_handler(window, Rc::clone(&shortcuts));
    bind_capture_shortcut_handler(window);
}

fn bind_request_edit_handler(window: &ui::AppWindow, shortcuts: Rc<VecModel<ui::ShortcutAction>>) {
    window.on_request_edit({
        let window = window.as_weak();

        move |index| {
            let Some(window) = window.upgrade() else {
                return;
            };

            let Ok(index) = usize::try_from(index) else {
                return;
            };

            let Some(action) = shortcuts.row_data(index) else {
                return;
            };

            let Some(state) = edit_start_state(index, action) else {
                return;
            };

            apply_edit_start_state(&window, state);
        }
    });
}

fn bind_cancel_edit_handler(window: &ui::AppWindow) {
    window.on_cancel_edit({
        let window = window.as_weak();

        move || {
            let Some(window) = window.upgrade() else {
                return;
            };

            close_editor(&window);
        }
    });
}

fn bind_accept_edit_handler(window: &ui::AppWindow, shortcuts: Rc<VecModel<ui::ShortcutAction>>) {
    window.on_accept_edit({
        let window = window.as_weak();

        move || {
            let Some(window) = window.upgrade() else {
                return;
            };

            if !window.get_pending_shortcut_valid() {
                return;
            }

            let Ok(index) = usize::try_from(window.get_editing_index()) else {
                return;
            };

            if save_shortcut(&shortcuts, index, window.get_pending_shortcut()) {
                close_editor(&window);
            }
        }
    });
}

fn bind_capture_shortcut_handler(window: &ui::AppWindow) {
    window.on_capture_shortcut({
        let window = window.as_weak();

        move |text, control, alt, shift, meta| {
            let Some(window) = window.upgrade() else {
                return;
            };

            if window.get_editing_index() < 0 {
                return;
            }

            update_pending_shortcut(&window, text.as_str(), control, alt, shift, meta);
        }
    });
}

fn apply_edit_start_state(window: &ui::AppWindow, state: EditStartState) {
    window.set_editing_index(state.editing_index);
    window.set_editing_label(state.editing_label);
    window.set_pending_shortcut(state.pending_shortcut);
    window.set_pending_shortcut_valid(state.pending_shortcut_valid);
}

fn edit_start_state(index: usize, action: ui::ShortcutAction) -> Option<EditStartState> {
    let editing_index = i32::try_from(index).ok()?;

    Some(EditStartState {
        editing_index,
        editing_label: action.label,
        pending_shortcut: action.shortcut.clone(),
        pending_shortcut_valid: !action.shortcut.is_empty(),
    })
}

fn close_editor(window: &ui::AppWindow) {
    let state = closed_editor_state();
    window.set_editing_index(state.editing_index);
    window.set_pending_shortcut_valid(state.pending_shortcut_valid);
}

fn closed_editor_state() -> ClosedEditorState {
    ClosedEditorState { editing_index: -1, pending_shortcut_valid: false }
}

fn save_shortcut(
    shortcuts: &VecModel<ui::ShortcutAction>,
    index: usize,
    pending_shortcut: SharedString,
) -> bool {
    let Some(mut action) = shortcuts.row_data(index) else {
        return false;
    };

    action.shortcut = pending_shortcut;
    shortcuts.set_row_data(index, action);
    true
}

fn update_pending_shortcut(
    window: &ui::AppWindow,
    text: &str,
    control: bool,
    alt: bool,
    shift: bool,
    meta: bool,
) {
    let render = render_shortcut(text, control, alt, shift, meta);
    window.set_pending_shortcut(render.display.into());
    window.set_pending_shortcut_valid(render.valid);
}

fn compose_shortcut_display(
    key: Option<KeyDescriptor>,
    control: bool,
    alt: bool,
    shift: bool,
    meta: bool,
) -> String {
    let mut parts = Vec::new();

    if control {
        parts.push("Ctrl".to_string());
    }
    if alt {
        parts.push("Alt".to_string());
    }
    if shift {
        parts.push("Shift".to_string());
    }
    if meta {
        parts.push("Meta".to_string());
    }

    if let Some(key) = key.filter(|key| !key.is_modifier) {
        parts.push(key.label);
    }

    if parts.is_empty() {
        "Нажмите сочетание клавиш".to_string()
    } else {
        parts.join(" + ")
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ShortcutRender {
    display: String,
    valid: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct KeyDescriptor {
    label: String,
    is_modifier: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct EditStartState {
    editing_index: i32,
    editing_label: SharedString,
    pending_shortcut: SharedString,
    pending_shortcut_valid: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct ClosedEditorState {
    editing_index: i32,
    pending_shortcut_valid: bool,
}

fn render_shortcut(
    text: &str,
    control: bool,
    alt: bool,
    shift: bool,
    meta: bool,
) -> ShortcutRender {
    let key = describe_key(text);

    ShortcutRender {
        valid: key.as_ref().is_some_and(|descriptor| !descriptor.is_modifier),
        display: compose_shortcut_display(key, control, alt, shift, meta),
    }
}

fn describe_key(text: &str) -> Option<KeyDescriptor> {
    for (key, label, is_modifier) in special_key_labels() {
        if matches_key(text, *key) {
            return Some(KeyDescriptor { label: (*label).to_string(), is_modifier: *is_modifier });
        }
    }

    let mut chars = text.chars();
    let ch = chars.next()?;

    if chars.next().is_some() || ch.is_control() {
        return None;
    }

    Some(KeyDescriptor {
        label: if ch == ' ' { "Space".to_string() } else { ch.to_uppercase().collect() },
        is_modifier: false,
    })
}

fn special_key_labels() -> &'static [(slint::platform::Key, &'static str, bool)] {
    use slint::platform::Key;

    &[
        (Key::Control, "Ctrl", true),
        (Key::ControlR, "Ctrl", true),
        (Key::Alt, "Alt", true),
        (Key::AltGr, "AltGr", true),
        (Key::Shift, "Shift", true),
        (Key::ShiftR, "Shift", true),
        (Key::Meta, "Meta", true),
        (Key::MetaR, "Meta", true),
        (Key::Return, "Enter", false),
        (Key::Escape, "Escape", false),
        (Key::Space, "Space", false),
        (Key::Tab, "Tab", false),
        (Key::Backtab, "Tab", false),
        (Key::Backspace, "Backspace", false),
        (Key::Delete, "Delete", false),
        (Key::Insert, "Insert", false),
        (Key::Home, "Home", false),
        (Key::End, "End", false),
        (Key::PageUp, "Page Up", false),
        (Key::PageDown, "Page Down", false),
        (Key::UpArrow, "Up", false),
        (Key::DownArrow, "Down", false),
        (Key::LeftArrow, "Left", false),
        (Key::RightArrow, "Right", false),
        (Key::Menu, "Menu", false),
        (Key::Pause, "Pause", false),
        (Key::ScrollLock, "Scroll Lock", false),
        (Key::CapsLock, "Caps Lock", false),
        (Key::SysReq, "SysRq", false),
        (Key::Stop, "Stop", false),
        (Key::Back, "Back", false),
        (Key::F1, "F1", false),
        (Key::F2, "F2", false),
        (Key::F3, "F3", false),
        (Key::F4, "F4", false),
        (Key::F5, "F5", false),
        (Key::F6, "F6", false),
        (Key::F7, "F7", false),
        (Key::F8, "F8", false),
        (Key::F9, "F9", false),
        (Key::F10, "F10", false),
        (Key::F11, "F11", false),
        (Key::F12, "F12", false),
        (Key::F13, "F13", false),
        (Key::F14, "F14", false),
        (Key::F15, "F15", false),
        (Key::F16, "F16", false),
        (Key::F17, "F17", false),
        (Key::F18, "F18", false),
        (Key::F19, "F19", false),
        (Key::F20, "F20", false),
        (Key::F21, "F21", false),
        (Key::F22, "F22", false),
        (Key::F23, "F23", false),
        (Key::F24, "F24", false),
    ]
}

fn matches_key(text: &str, key: slint::platform::Key) -> bool {
    let key_text: SharedString = key.into();
    text == key_text.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_letter_with_modifiers() {
        let rendered = render_shortcut("k", true, false, true, false);
        assert_eq!(rendered.display, "Ctrl + Shift + K");
        assert!(rendered.valid);
    }

    #[test]
    fn renders_function_key() {
        let rendered = render_shortcut(
            SharedString::from(slint::platform::Key::F8).as_str(),
            false,
            false,
            false,
            false,
        );
        assert_eq!(rendered.display, "F8");
        assert!(rendered.valid);
    }

    #[test]
    fn renders_meta_space() {
        let rendered = render_shortcut(
            SharedString::from(slint::platform::Key::Space).as_str(),
            false,
            false,
            false,
            true,
        );
        assert_eq!(rendered.display, "Meta + Space");
        assert!(rendered.valid);
    }

    #[test]
    fn treats_modifier_only_as_invalid() {
        let rendered = render_shortcut(
            SharedString::from(slint::platform::Key::Control).as_str(),
            true,
            false,
            false,
            false,
        );
        assert_eq!(rendered.display, "Ctrl");
        assert!(!rendered.valid);
    }

    #[test]
    fn allows_escape_and_enter_as_shortcuts() {
        let escape_rendered = render_shortcut(
            SharedString::from(slint::platform::Key::Escape).as_str(),
            false,
            false,
            false,
            false,
        );
        assert_eq!(escape_rendered.display, "Escape");
        assert!(escape_rendered.valid);

        let enter_rendered = render_shortcut(
            SharedString::from(slint::platform::Key::Return).as_str(),
            true,
            false,
            false,
            false,
        );
        assert_eq!(enter_rendered.display, "Ctrl + Enter");
        assert!(enter_rendered.valid);
    }

    #[test]
    fn saves_shortcut_into_model() {
        let shortcuts = VecModel::from(vec![ui::ShortcutAction {
            label: "Последнее слово".into(),
            shortcut: "Ctrl + Alt + 1".into(),
        }]);

        assert!(save_shortcut(&shortcuts, 0, "Alt + Shift + K".into()));

        let action = shortcuts.row_data(0).expect("expected shortcut row");
        assert_eq!(action.shortcut, "Alt + Shift + K");
    }

    #[test]
    fn closes_editor_state() {
        let state = closed_editor_state();

        assert_eq!(state.editing_index, -1);
        assert!(!state.pending_shortcut_valid);
    }
}
