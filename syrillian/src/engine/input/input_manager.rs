use crate::ViewportId;
use crate::game_thread::GameAppEvent;
use crate::input::gamepad_manager::GamePadManager;
use crossbeam_channel::Sender;
use nalgebra::Vector2;
use num_traits::Zero;
use std::collections::HashMap;
use tracing::{info, trace};
use winit::dpi::PhysicalPosition;
use winit::event::{DeviceEvent, ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

pub type KeyState = ElementState;

#[derive(Debug, Default)]
struct InputState {
    key_states: HashMap<KeyCode, KeyState>,
    key_just_updated: Vec<KeyCode>,
    button_states: HashMap<MouseButton, ElementState>,
    button_just_updated: Vec<MouseButton>,
    mouse_wheel_delta: f32,
    mouse_pos: PhysicalPosition<f32>,
    mouse_delta: Vector2<f32>,
    is_locked: bool,
}

#[derive(Debug)]
pub struct InputManager {
    state: InputState,
    focus: HashMap<ViewportId, bool>,
    active_target: ViewportId,
    pub gamepad: GamePadManager,
    game_event_tx: Sender<GameAppEvent>,
}

#[allow(unused)]
impl InputManager {
    pub fn new(game_event_tx: Sender<GameAppEvent>) -> Self {
        InputManager {
            state: InputState::default(),
            focus: HashMap::default(),
            active_target: ViewportId::PRIMARY,
            gamepad: GamePadManager::default(),
            game_event_tx,
        }
    }

    pub fn set_game_event_tx(&mut self, game_event_tx: Sender<GameAppEvent>) {
        self.game_event_tx = game_event_tx;
    }

    fn state_mut(&mut self) -> &mut InputState {
        &mut self.state
    }

    fn state(&self) -> &InputState {
        &self.state
    }

    pub fn set_active_target(&mut self, target: ViewportId) {
        self.active_target = target;
    }

    pub fn active_target(&self) -> ViewportId {
        self.active_target
    }

    pub fn set_window_focus(&mut self, target: ViewportId, focused: bool) {
        self.focus.insert(target, focused);
    }

    pub fn is_window_focused_for(&self, target: ViewportId) -> bool {
        *self.focus.get(&target).unwrap_or(&true)
    }

    pub fn is_window_focused(&self) -> bool {
        self.is_window_focused_for(self.active_target)
    }

    pub(crate) fn process_device_input_event(&mut self, device_event: &DeviceEvent) {
        let state = self.state_mut();
        if let DeviceEvent::MouseMotion { delta } = device_event {
            state.mouse_delta = Vector2::new(-delta.0 as f32, -delta.1 as f32);
            state.mouse_pos.x += state.mouse_delta.x;
            state.mouse_pos.y += state.mouse_delta.y;
        }
    }

    #[inline]
    pub(crate) fn process_mouse_event(&mut self, position: &PhysicalPosition<f64>) {
        let new_pos = PhysicalPosition::new(position.x as f32, position.y as f32);
        self.state_mut().mouse_pos = new_pos;
    }

    pub fn process_event(&mut self, target: ViewportId, event: &WindowEvent) {
        if let WindowEvent::Focused(focused) = event {
            self.set_window_focus(target, *focused);
        }
        if !self.is_window_focused_for(target)
            && !matches!(event, WindowEvent::Focused(_) | WindowEvent::Resized(_))
        {
            return;
        }
        self.handle_window_event(event);
    }

    fn handle_window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                let state = self.state_mut();
                if let PhysicalKey::Code(code) = event.physical_key {
                    if !event.state.is_pressed()
                        || state
                            .key_states
                            .get(&code)
                            .is_none_or(|state| !state.is_pressed())
                    {
                        state.key_just_updated.push(code);
                    }

                    state.key_states.insert(code, event.state);
                }
            }
            WindowEvent::CursorMoved {
                position,
                device_id: _,
            } => self.process_mouse_event(position),
            WindowEvent::MouseWheel { delta, .. } => {
                let y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y as f64,
                    MouseScrollDelta::PixelDelta(pos) => pos.y,
                };
                self.state_mut().mouse_wheel_delta += y as f32;
            }
            WindowEvent::MouseInput { button, state, .. } => {
                let state_entry = self.state_mut();
                if !state.is_pressed()
                    || state_entry
                        .button_states
                        .get(button)
                        .is_none_or(|state| !state.is_pressed())
                {
                    state_entry.button_just_updated.push(*button);
                }
                state_entry.button_states.insert(*button, *state);
            }
            _ => {}
        }
    }

    pub fn key_state(&self, key_code: KeyCode) -> KeyState {
        *self
            .state()
            .key_states
            .get(&key_code)
            .unwrap_or(&KeyState::Released)
    }

    // Only is true if the key was JUST pressed
    pub fn is_key_down(&self, key_code: KeyCode) -> bool {
        self.key_state(key_code) == KeyState::Pressed
            && self.state().key_just_updated.contains(&key_code)
    }

    // true if the key was JUST pressed or is being held
    pub fn is_key_pressed(&self, key_code: KeyCode) -> bool {
        self.key_state(key_code) == KeyState::Pressed
    }

    // true if the key was JUST released or is unpressed
    pub fn is_key_released(&self, key_code: KeyCode) -> bool {
        self.key_state(key_code) == KeyState::Released
            && self.state().key_just_updated.contains(&key_code)
    }

    // Only is true if the key was JUST released
    pub fn is_key_up(&self, key_code: KeyCode) -> bool {
        self.key_state(key_code) == KeyState::Released
    }

    pub fn button_state(&self, button: MouseButton) -> ElementState {
        *self
            .state()
            .button_states
            .get(&button)
            .unwrap_or(&ElementState::Released)
    }

    pub fn is_button_down(&self, button: MouseButton) -> bool {
        self.button_state(button) == ElementState::Pressed
            && self.state().button_just_updated.contains(&button)
    }

    pub fn is_button_pressed(&self, button: MouseButton) -> bool {
        self.button_state(button) == ElementState::Pressed
    }

    pub fn is_button_released(&self, button: MouseButton) -> bool {
        self.button_state(button) == ElementState::Released
            && self.state().button_just_updated.contains(&button)
    }

    #[inline]
    pub fn mouse_position(&self) -> PhysicalPosition<f32> {
        self.state().mouse_pos
    }

    pub fn mouse_delta(&self) -> &Vector2<f32> {
        &self.state().mouse_delta
    }

    pub fn lock_cursor(&mut self) {
        trace!("GT: Locked cursor");
        let state = self.state_mut();
        state.is_locked = true;
        let _ = self
            .game_event_tx
            .send(GameAppEvent::cursor_mode(self.active_target, true, false));
    }

    pub fn unlock_cursor(&mut self) {
        trace!("GT: Unlocked cursor");
        let state = self.state_mut();
        state.is_locked = false;
        let _ = self
            .game_event_tx
            .send(GameAppEvent::cursor_mode(self.active_target, false, true));
    }

    pub fn is_cursor_locked(&self) -> bool {
        self.state().is_locked
    }

    pub fn next_frame_all(&mut self) {
        self.state.key_just_updated.clear();
        self.state.button_just_updated.clear();
        self.state.mouse_delta = Vector2::zero();
        self.gamepad.poll();
    }

    pub fn mouse_wheel_delta(&self) -> f32 {
        self.state().mouse_wheel_delta
    }

    pub fn gamepad(&self) -> &GamePadManager {
        &self.gamepad
    }

    pub fn auto_cursor_lock(&mut self) {
        if self.is_cursor_locked() {
            if self.is_key_down(KeyCode::Escape) {
                self.unlock_cursor();
            }
            return;
        }

        if self.is_button_down(MouseButton::Left) {
            self.lock_cursor();
        }
    }

    pub fn auto_quit_on_escape(&mut self) {
        if self.is_key_down(KeyCode::Escape) && !self.is_cursor_locked() {
            info!("Shutting down world from escape press");
            let _ = self.game_event_tx.send(GameAppEvent::Shutdown);
        }
    }

    pub fn is_sprinting(&self) -> bool {
        self.is_key_pressed(KeyCode::ShiftLeft)
    }

    pub fn is_jump_down(&self) -> bool {
        self.is_key_down(KeyCode::Space)
    }
}
