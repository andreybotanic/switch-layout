use std::{cell::RefCell, rc::Rc};

use slint::winit_030::{EventResult, WinitWindowAccessor, winit};
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
    bind_winit_shortcut_handler(window);
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

fn bind_winit_shortcut_handler(window: &ui::AppWindow) {
    let state = Rc::new(RefCell::new(ShortcutCaptureState::default()));

    window.window().on_winit_window_event({
        let window = window.as_weak();

        move |_slint_window, event| {
            let Some(window) = window.upgrade() else {
                return EventResult::Propagate;
            };

            match event {
                winit::event::WindowEvent::Focused(false) => {
                    state.borrow_mut().reset();
                    EventResult::Propagate
                }
                winit::event::WindowEvent::ModifiersChanged(modifiers) => {
                    state.borrow_mut().sync_modifiers(modifiers.state());
                    EventResult::Propagate
                }
                winit::event::WindowEvent::KeyboardInput { event, is_synthetic, .. } => {
                    let editing_active = window.get_editing_index() >= 0;
                    let mut state = state.borrow_mut();
                    state.apply_key_event(event.physical_key, event.state);

                    if !editing_active {
                        return EventResult::Propagate;
                    }

                    if !is_synthetic && event.state.is_pressed() && !event.repeat {
                        update_pending_shortcut(
                            &window,
                            ShortcutInput {
                                key: describe_physical_key(event.physical_key),
                                modifiers: state.modifiers,
                            },
                        );
                    }

                    EventResult::PreventDefault
                }
                _ => EventResult::Propagate,
            }
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

fn update_pending_shortcut(window: &ui::AppWindow, input: ShortcutInput) {
    let render = render_shortcut(input);
    window.set_pending_shortcut(render.display.into());
    window.set_pending_shortcut_valid(render.valid);
}

fn render_shortcut(input: ShortcutInput) -> ShortcutRender {
    ShortcutRender {
        valid: input.key.as_ref().is_some_and(|descriptor| !descriptor.is_modifier),
        display: compose_shortcut_display(input),
    }
}

fn compose_shortcut_display(input: ShortcutInput) -> String {
    let mut parts = Vec::new();

    if input.modifiers.control {
        parts.push("Ctrl".to_string());
    }
    if input.modifiers.alt {
        parts.push("Alt".to_string());
    }
    if input.modifiers.shift {
        parts.push("Shift".to_string());
    }
    if input.modifiers.win {
        parts.push("Win".to_string());
    }
    if input.modifiers.function {
        parts.push("Fn".to_string());
    }

    if let Some(key) = input.key.filter(|key| !key.is_modifier) {
        parts.push(key.label.to_string());
    }

    if parts.is_empty() {
        "Нажмите сочетание клавиш".to_string()
    } else {
        parts.join(" + ")
    }
}

fn describe_physical_key(physical_key: winit::keyboard::PhysicalKey) -> Option<KeyDescriptor> {
    let code = match physical_key {
        winit::keyboard::PhysicalKey::Code(code) => code,
        winit::keyboard::PhysicalKey::Unidentified(_) => return None,
    };

    Some(match code {
        winit::keyboard::KeyCode::KeyA => KeyDescriptor { label: "A", is_modifier: false },
        winit::keyboard::KeyCode::KeyB => KeyDescriptor { label: "B", is_modifier: false },
        winit::keyboard::KeyCode::KeyC => KeyDescriptor { label: "C", is_modifier: false },
        winit::keyboard::KeyCode::KeyD => KeyDescriptor { label: "D", is_modifier: false },
        winit::keyboard::KeyCode::KeyE => KeyDescriptor { label: "E", is_modifier: false },
        winit::keyboard::KeyCode::KeyF => KeyDescriptor { label: "F", is_modifier: false },
        winit::keyboard::KeyCode::KeyG => KeyDescriptor { label: "G", is_modifier: false },
        winit::keyboard::KeyCode::KeyH => KeyDescriptor { label: "H", is_modifier: false },
        winit::keyboard::KeyCode::KeyI => KeyDescriptor { label: "I", is_modifier: false },
        winit::keyboard::KeyCode::KeyJ => KeyDescriptor { label: "J", is_modifier: false },
        winit::keyboard::KeyCode::KeyK => KeyDescriptor { label: "K", is_modifier: false },
        winit::keyboard::KeyCode::KeyL => KeyDescriptor { label: "L", is_modifier: false },
        winit::keyboard::KeyCode::KeyM => KeyDescriptor { label: "M", is_modifier: false },
        winit::keyboard::KeyCode::KeyN => KeyDescriptor { label: "N", is_modifier: false },
        winit::keyboard::KeyCode::KeyO => KeyDescriptor { label: "O", is_modifier: false },
        winit::keyboard::KeyCode::KeyP => KeyDescriptor { label: "P", is_modifier: false },
        winit::keyboard::KeyCode::KeyQ => KeyDescriptor { label: "Q", is_modifier: false },
        winit::keyboard::KeyCode::KeyR => KeyDescriptor { label: "R", is_modifier: false },
        winit::keyboard::KeyCode::KeyS => KeyDescriptor { label: "S", is_modifier: false },
        winit::keyboard::KeyCode::KeyT => KeyDescriptor { label: "T", is_modifier: false },
        winit::keyboard::KeyCode::KeyU => KeyDescriptor { label: "U", is_modifier: false },
        winit::keyboard::KeyCode::KeyV => KeyDescriptor { label: "V", is_modifier: false },
        winit::keyboard::KeyCode::KeyW => KeyDescriptor { label: "W", is_modifier: false },
        winit::keyboard::KeyCode::KeyX => KeyDescriptor { label: "X", is_modifier: false },
        winit::keyboard::KeyCode::KeyY => KeyDescriptor { label: "Y", is_modifier: false },
        winit::keyboard::KeyCode::KeyZ => KeyDescriptor { label: "Z", is_modifier: false },
        winit::keyboard::KeyCode::Digit0 => KeyDescriptor { label: "0", is_modifier: false },
        winit::keyboard::KeyCode::Digit1 => KeyDescriptor { label: "1", is_modifier: false },
        winit::keyboard::KeyCode::Digit2 => KeyDescriptor { label: "2", is_modifier: false },
        winit::keyboard::KeyCode::Digit3 => KeyDescriptor { label: "3", is_modifier: false },
        winit::keyboard::KeyCode::Digit4 => KeyDescriptor { label: "4", is_modifier: false },
        winit::keyboard::KeyCode::Digit5 => KeyDescriptor { label: "5", is_modifier: false },
        winit::keyboard::KeyCode::Digit6 => KeyDescriptor { label: "6", is_modifier: false },
        winit::keyboard::KeyCode::Digit7 => KeyDescriptor { label: "7", is_modifier: false },
        winit::keyboard::KeyCode::Digit8 => KeyDescriptor { label: "8", is_modifier: false },
        winit::keyboard::KeyCode::Digit9 => KeyDescriptor { label: "9", is_modifier: false },
        winit::keyboard::KeyCode::Backquote => KeyDescriptor { label: "`", is_modifier: false },
        winit::keyboard::KeyCode::Minus => KeyDescriptor { label: "-", is_modifier: false },
        winit::keyboard::KeyCode::Equal => KeyDescriptor { label: "=", is_modifier: false },
        winit::keyboard::KeyCode::BracketLeft => KeyDescriptor { label: "[", is_modifier: false },
        winit::keyboard::KeyCode::BracketRight => KeyDescriptor { label: "]", is_modifier: false },
        winit::keyboard::KeyCode::Backslash
        | winit::keyboard::KeyCode::IntlBackslash
        | winit::keyboard::KeyCode::IntlYen => KeyDescriptor { label: "\\", is_modifier: false },
        winit::keyboard::KeyCode::Semicolon => KeyDescriptor { label: ";", is_modifier: false },
        winit::keyboard::KeyCode::Quote => KeyDescriptor { label: "'", is_modifier: false },
        winit::keyboard::KeyCode::Comma => KeyDescriptor { label: ",", is_modifier: false },
        winit::keyboard::KeyCode::Period => KeyDescriptor { label: ".", is_modifier: false },
        winit::keyboard::KeyCode::Slash | winit::keyboard::KeyCode::IntlRo => {
            KeyDescriptor { label: "/", is_modifier: false }
        }
        winit::keyboard::KeyCode::Space => KeyDescriptor { label: "Space", is_modifier: false },
        winit::keyboard::KeyCode::Tab => KeyDescriptor { label: "Tab", is_modifier: false },
        winit::keyboard::KeyCode::Enter | winit::keyboard::KeyCode::NumpadEnter => {
            KeyDescriptor { label: "Enter", is_modifier: false }
        }
        winit::keyboard::KeyCode::Escape => KeyDescriptor { label: "Escape", is_modifier: false },
        winit::keyboard::KeyCode::Backspace => {
            KeyDescriptor { label: "Backspace", is_modifier: false }
        }
        winit::keyboard::KeyCode::Delete => KeyDescriptor { label: "Delete", is_modifier: false },
        winit::keyboard::KeyCode::Insert => KeyDescriptor { label: "Insert", is_modifier: false },
        winit::keyboard::KeyCode::Home => KeyDescriptor { label: "Home", is_modifier: false },
        winit::keyboard::KeyCode::End => KeyDescriptor { label: "End", is_modifier: false },
        winit::keyboard::KeyCode::PageUp => KeyDescriptor { label: "Page Up", is_modifier: false },
        winit::keyboard::KeyCode::PageDown => {
            KeyDescriptor { label: "Page Down", is_modifier: false }
        }
        winit::keyboard::KeyCode::ArrowUp => KeyDescriptor { label: "Up", is_modifier: false },
        winit::keyboard::KeyCode::ArrowDown => KeyDescriptor { label: "Down", is_modifier: false },
        winit::keyboard::KeyCode::ArrowLeft => KeyDescriptor { label: "Left", is_modifier: false },
        winit::keyboard::KeyCode::ArrowRight => {
            KeyDescriptor { label: "Right", is_modifier: false }
        }
        winit::keyboard::KeyCode::ContextMenu => {
            KeyDescriptor { label: "Menu", is_modifier: false }
        }
        winit::keyboard::KeyCode::Pause => KeyDescriptor { label: "Pause", is_modifier: false },
        winit::keyboard::KeyCode::ScrollLock => {
            KeyDescriptor { label: "Scroll Lock", is_modifier: false }
        }
        winit::keyboard::KeyCode::CapsLock => {
            KeyDescriptor { label: "Caps Lock", is_modifier: false }
        }
        winit::keyboard::KeyCode::PrintScreen => {
            KeyDescriptor { label: "Print Screen", is_modifier: false }
        }
        winit::keyboard::KeyCode::NumLock => {
            KeyDescriptor { label: "Num Lock", is_modifier: false }
        }
        winit::keyboard::KeyCode::Fn => KeyDescriptor { label: "Fn", is_modifier: true },
        winit::keyboard::KeyCode::FnLock => KeyDescriptor { label: "Fn Lock", is_modifier: false },
        winit::keyboard::KeyCode::ControlLeft | winit::keyboard::KeyCode::ControlRight => {
            KeyDescriptor { label: "Ctrl", is_modifier: true }
        }
        winit::keyboard::KeyCode::AltLeft | winit::keyboard::KeyCode::AltRight => {
            KeyDescriptor { label: "Alt", is_modifier: true }
        }
        winit::keyboard::KeyCode::ShiftLeft | winit::keyboard::KeyCode::ShiftRight => {
            KeyDescriptor { label: "Shift", is_modifier: true }
        }
        winit::keyboard::KeyCode::SuperLeft
        | winit::keyboard::KeyCode::SuperRight
        | winit::keyboard::KeyCode::Meta => KeyDescriptor { label: "Win", is_modifier: true },
        winit::keyboard::KeyCode::F1 => KeyDescriptor { label: "F1", is_modifier: false },
        winit::keyboard::KeyCode::F2 => KeyDescriptor { label: "F2", is_modifier: false },
        winit::keyboard::KeyCode::F3 => KeyDescriptor { label: "F3", is_modifier: false },
        winit::keyboard::KeyCode::F4 => KeyDescriptor { label: "F4", is_modifier: false },
        winit::keyboard::KeyCode::F5 => KeyDescriptor { label: "F5", is_modifier: false },
        winit::keyboard::KeyCode::F6 => KeyDescriptor { label: "F6", is_modifier: false },
        winit::keyboard::KeyCode::F7 => KeyDescriptor { label: "F7", is_modifier: false },
        winit::keyboard::KeyCode::F8 => KeyDescriptor { label: "F8", is_modifier: false },
        winit::keyboard::KeyCode::F9 => KeyDescriptor { label: "F9", is_modifier: false },
        winit::keyboard::KeyCode::F10 => KeyDescriptor { label: "F10", is_modifier: false },
        winit::keyboard::KeyCode::F11 => KeyDescriptor { label: "F11", is_modifier: false },
        winit::keyboard::KeyCode::F12 => KeyDescriptor { label: "F12", is_modifier: false },
        winit::keyboard::KeyCode::F13 => KeyDescriptor { label: "F13", is_modifier: false },
        winit::keyboard::KeyCode::F14 => KeyDescriptor { label: "F14", is_modifier: false },
        winit::keyboard::KeyCode::F15 => KeyDescriptor { label: "F15", is_modifier: false },
        winit::keyboard::KeyCode::F16 => KeyDescriptor { label: "F16", is_modifier: false },
        winit::keyboard::KeyCode::F17 => KeyDescriptor { label: "F17", is_modifier: false },
        winit::keyboard::KeyCode::F18 => KeyDescriptor { label: "F18", is_modifier: false },
        winit::keyboard::KeyCode::F19 => KeyDescriptor { label: "F19", is_modifier: false },
        winit::keyboard::KeyCode::F20 => KeyDescriptor { label: "F20", is_modifier: false },
        winit::keyboard::KeyCode::F21 => KeyDescriptor { label: "F21", is_modifier: false },
        winit::keyboard::KeyCode::F22 => KeyDescriptor { label: "F22", is_modifier: false },
        winit::keyboard::KeyCode::F23 => KeyDescriptor { label: "F23", is_modifier: false },
        winit::keyboard::KeyCode::F24 => KeyDescriptor { label: "F24", is_modifier: false },
        winit::keyboard::KeyCode::Numpad0 => KeyDescriptor { label: "Num 0", is_modifier: false },
        winit::keyboard::KeyCode::Numpad1 => KeyDescriptor { label: "Num 1", is_modifier: false },
        winit::keyboard::KeyCode::Numpad2 => KeyDescriptor { label: "Num 2", is_modifier: false },
        winit::keyboard::KeyCode::Numpad3 => KeyDescriptor { label: "Num 3", is_modifier: false },
        winit::keyboard::KeyCode::Numpad4 => KeyDescriptor { label: "Num 4", is_modifier: false },
        winit::keyboard::KeyCode::Numpad5 => KeyDescriptor { label: "Num 5", is_modifier: false },
        winit::keyboard::KeyCode::Numpad6 => KeyDescriptor { label: "Num 6", is_modifier: false },
        winit::keyboard::KeyCode::Numpad7 => KeyDescriptor { label: "Num 7", is_modifier: false },
        winit::keyboard::KeyCode::Numpad8 => KeyDescriptor { label: "Num 8", is_modifier: false },
        winit::keyboard::KeyCode::Numpad9 => KeyDescriptor { label: "Num 9", is_modifier: false },
        winit::keyboard::KeyCode::NumpadAdd => KeyDescriptor { label: "Num +", is_modifier: false },
        winit::keyboard::KeyCode::NumpadSubtract => {
            KeyDescriptor { label: "Num -", is_modifier: false }
        }
        winit::keyboard::KeyCode::NumpadMultiply => {
            KeyDescriptor { label: "Num *", is_modifier: false }
        }
        winit::keyboard::KeyCode::NumpadDivide => {
            KeyDescriptor { label: "Num /", is_modifier: false }
        }
        winit::keyboard::KeyCode::NumpadDecimal => {
            KeyDescriptor { label: "Num .", is_modifier: false }
        }
        _ => return None,
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ShortcutModifiers {
    control: bool,
    alt: bool,
    shift: bool,
    win: bool,
    function: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ShortcutCaptureState {
    modifiers: ShortcutModifiers,
}

impl ShortcutCaptureState {
    fn reset(&mut self) {
        self.modifiers = ShortcutModifiers::default();
    }

    fn sync_modifiers(&mut self, modifiers: winit::keyboard::ModifiersState) {
        self.modifiers.control = modifiers.control_key();
        self.modifiers.alt = modifiers.alt_key();
        self.modifiers.shift = modifiers.shift_key();
        self.modifiers.win = modifiers.super_key();
    }

    fn apply_key_event(
        &mut self,
        physical_key: winit::keyboard::PhysicalKey,
        state: winit::event::ElementState,
    ) {
        let pressed = state.is_pressed();

        match physical_key {
            winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::ControlLeft)
            | winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::ControlRight) => {
                self.modifiers.control = pressed;
            }
            winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::AltLeft)
            | winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::AltRight) => {
                self.modifiers.alt = pressed;
            }
            winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::ShiftLeft)
            | winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::ShiftRight) => {
                self.modifiers.shift = pressed;
            }
            winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::SuperLeft)
            | winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::SuperRight)
            | winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Meta) => {
                self.modifiers.win = pressed;
            }
            winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Fn) => {
                self.modifiers.function = pressed;
            }
            _ => {}
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ShortcutRender {
    display: String,
    valid: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ShortcutInput {
    key: Option<KeyDescriptor>,
    modifiers: ShortcutModifiers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct KeyDescriptor {
    label: &'static str,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_letter_with_modifiers() {
        let rendered = render_shortcut(shortcut_input(
            winit::keyboard::KeyCode::KeyK,
            ShortcutModifiers { control: true, shift: true, ..Default::default() },
        ));
        assert_eq!(rendered.display, "Ctrl + Shift + K");
        assert!(rendered.valid);
    }

    #[test]
    fn renders_function_key() {
        let rendered = render_shortcut(shortcut_input(
            winit::keyboard::KeyCode::F8,
            ShortcutModifiers::default(),
        ));
        assert_eq!(rendered.display, "F8");
        assert!(rendered.valid);
    }

    #[test]
    fn renders_win_space() {
        let rendered = render_shortcut(shortcut_input(
            winit::keyboard::KeyCode::Space,
            ShortcutModifiers { win: true, ..Default::default() },
        ));
        assert_eq!(rendered.display, "Win + Space");
        assert!(rendered.valid);
    }

    #[test]
    fn treats_modifier_only_as_invalid() {
        let rendered = render_shortcut(shortcut_input(
            winit::keyboard::KeyCode::ControlLeft,
            ShortcutModifiers { control: true, ..Default::default() },
        ));
        assert_eq!(rendered.display, "Ctrl");
        assert!(!rendered.valid);
    }

    #[test]
    fn allows_escape_and_enter_as_shortcuts() {
        let escape_rendered = render_shortcut(shortcut_input(
            winit::keyboard::KeyCode::Escape,
            ShortcutModifiers::default(),
        ));
        assert_eq!(escape_rendered.display, "Escape");
        assert!(escape_rendered.valid);

        let enter_rendered = render_shortcut(shortcut_input(
            winit::keyboard::KeyCode::Enter,
            ShortcutModifiers { control: true, ..Default::default() },
        ));
        assert_eq!(enter_rendered.display, "Ctrl + Enter");
        assert!(enter_rendered.valid);
    }

    #[test]
    fn renders_ctrl_win_g() {
        let rendered = render_shortcut(shortcut_input(
            winit::keyboard::KeyCode::KeyG,
            ShortcutModifiers { control: true, win: true, ..Default::default() },
        ));
        assert_eq!(rendered.display, "Ctrl + Win + G");
        assert!(rendered.valid);
    }

    #[test]
    fn maps_physical_key_to_english_letter() {
        let descriptor = describe_physical_key(winit::keyboard::PhysicalKey::Code(
            winit::keyboard::KeyCode::KeyG,
        ));
        assert_eq!(descriptor, Some(KeyDescriptor { label: "G", is_modifier: false }));
    }

    #[test]
    fn treats_super_keys_as_win() {
        let left = describe_physical_key(winit::keyboard::PhysicalKey::Code(
            winit::keyboard::KeyCode::SuperLeft,
        ));
        let right = describe_physical_key(winit::keyboard::PhysicalKey::Code(
            winit::keyboard::KeyCode::SuperRight,
        ));

        assert_eq!(left, Some(KeyDescriptor { label: "Win", is_modifier: true }));
        assert_eq!(right, Some(KeyDescriptor { label: "Win", is_modifier: true }));
    }

    #[test]
    fn renders_fn_and_fn_lock_when_available() {
        let fn_rendered = render_shortcut(shortcut_input(
            winit::keyboard::KeyCode::Fn,
            ShortcutModifiers { function: true, ..Default::default() },
        ));
        assert_eq!(fn_rendered.display, "Fn");
        assert!(!fn_rendered.valid);

        let fn_lock_rendered = render_shortcut(shortcut_input(
            winit::keyboard::KeyCode::FnLock,
            ShortcutModifiers::default(),
        ));
        assert_eq!(fn_lock_rendered.display, "Fn Lock");
        assert!(fn_lock_rendered.valid);
    }

    #[test]
    fn resets_modifier_state() {
        let mut state = ShortcutCaptureState {
            modifiers: ShortcutModifiers {
                control: true,
                alt: true,
                shift: true,
                win: true,
                function: true,
            },
        };

        state.reset();

        assert_eq!(state.modifiers, ShortcutModifiers::default());
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

    fn shortcut_input(
        key: winit::keyboard::KeyCode,
        modifiers: ShortcutModifiers,
    ) -> ShortcutInput {
        ShortcutInput { key: describe_physical_key(key.into()), modifiers }
    }
}
