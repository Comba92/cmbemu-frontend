use std::collections::HashMap;

use nen_emulator::{joypad::JoypadButton, nes::Nes};
use sdl2::{audio::{AudioQueue, AudioStatus}, controller::{self, Axis, Button}, event::Event, keyboard::{self, Keycode}};

enum InputAction {
  Game(JoypadButton), Pause, Reset, DebugSpr0Hit
}
const AXIS_DEAD_ZONE: i16 = 10_000;

pub struct Keymaps {
  keymap: HashMap<keyboard::Keycode, InputAction>,
  padmap: HashMap<controller::Button, InputAction>,
}
impl Keymaps {
  pub fn new() -> Self {
    let default_keymap = HashMap::from([
      (Keycode::Z,      InputAction::Game(JoypadButton::A)),
      (Keycode::X,      InputAction::Game(JoypadButton::B)),
      (Keycode::UP,     InputAction::Game(JoypadButton::UP)),
      (Keycode::DOWN,   InputAction::Game(JoypadButton::DOWN)),
      (Keycode::LEFT,   InputAction::Game(JoypadButton::LEFT)),
      (Keycode::RIGHT,  InputAction::Game(JoypadButton::RIGHT)),
      (Keycode::N,      InputAction::Game(JoypadButton::SELECT)),
      (Keycode::M,      InputAction::Game(JoypadButton::START)),
      (Keycode::Space,  InputAction::Pause),
      (Keycode::R,      InputAction::Reset),
      (Keycode::D,      InputAction::DebugSpr0Hit),
    ]);

    let default_padmap = HashMap::from([
      (Button::X,         InputAction::Game(JoypadButton::A)),
      (Button::A,         InputAction::Game(JoypadButton::B)),
      (Button::B,         InputAction::Game(JoypadButton::START)),
      (Button::Y,         InputAction::Game(JoypadButton::SELECT)),
      (Button::Back,      InputAction::Game(JoypadButton::SELECT)),
      (Button::Start,     InputAction::Game(JoypadButton::START)),
      (Button::DPadLeft,  InputAction::Game(JoypadButton::LEFT)),
      (Button::DPadRight, InputAction::Game(JoypadButton::RIGHT)),
      (Button::DPadUp,    InputAction::Game(JoypadButton::UP)),
      (Button::DPadDown,  InputAction::Game(JoypadButton::DOWN)),
    ]);

    Keymaps { keymap: default_keymap, padmap: default_padmap }
  }
}

pub fn handle_input(keys: &Keymaps, event: &Event, emu: &mut Nes, audio_dev: &AudioQueue<f32>) {
  let joypad = emu.get_joypad();

  match event {
    Event::KeyDown { keycode, .. } 
    | Event::KeyUp { keycode, .. } => {
      if let Some(keycode) = keycode {
        if let Some(action) = keys.keymap.get(keycode) {
          match (action, event) {
            (InputAction::Game(button), Event::KeyDown {..}) => joypad.buttons1.insert(*button),
            (InputAction::Game(button), Event::KeyUp {..}) => joypad.buttons1.remove(*button),
            (InputAction::Pause, Event::KeyDown {..}) => {
              emu.is_paused = !emu.is_paused;
              match &audio_dev.status() {
                AudioStatus::Playing => audio_dev.pause(),
                _=> audio_dev.resume(),
              }
            },
            (InputAction::Reset, Event::KeyDown {..}) => emu.reset(),
            (InputAction::DebugSpr0Hit, Event::KeyDown { .. }) => emu.get_ppu().force_spr0_hit(),
            _ => {}
          }
        }
      }
    }
    Event::ControllerButtonDown { button, .. } 
    | Event::ControllerButtonUp { button, .. }  => {
      if let Some(action) = keys.padmap.get(button) {
        match (action, event) {
          (InputAction::Game(button), Event::ControllerButtonDown {..}) => joypad.buttons1.insert(*button),
          (InputAction::Game(button), Event::ControllerButtonUp {..}) => joypad.buttons1.remove(*button),
          (InputAction::Pause, Event::ControllerButtonDown {..}) => {
            emu.is_paused = !emu.is_paused;
            match &audio_dev.status() {
              AudioStatus::Playing => audio_dev.pause(),
              _=> audio_dev.resume(),
            }
          }
          (InputAction::Reset, Event::ControllerButtonDown {..}) => emu.reset(),
          _ => {}
        }
      }
    }

    Event::ControllerAxisMotion { axis: Axis::LeftX, value, .. } => {
      if *value > AXIS_DEAD_ZONE { joypad.buttons1.insert(JoypadButton::RIGHT); }
      else if *value < -AXIS_DEAD_ZONE { joypad.buttons1.insert(JoypadButton::LEFT); }
      else {
        joypad.buttons1.remove(JoypadButton::LEFT);
        joypad.buttons1.remove(JoypadButton::RIGHT);
      }
    }
    Event::ControllerAxisMotion { axis: Axis::LeftY, value, .. } => {
      if *value > AXIS_DEAD_ZONE { joypad.buttons1.insert(JoypadButton::DOWN); }
      else if *value < -AXIS_DEAD_ZONE { joypad.buttons1.insert(JoypadButton::UP); }
      else {
        joypad.buttons1.remove(JoypadButton::UP);
        joypad.buttons1.remove(JoypadButton::DOWN);
      }
    }
    _ => {}
  }
}
