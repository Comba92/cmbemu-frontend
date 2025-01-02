use std::collections::HashMap;

use sdl2::{audio::{AudioQueue, AudioStatus}, controller::{self, Axis, Button}, event::Event, keyboard::{self, Keycode}};

use crate::emu::Emulator;

pub enum InputKind {
  Press, Release
}

pub enum InputEvent {
  Game(GameInput),
  Pause, Reset, Save, Load, Mute,
}

pub enum GameInput {
  Up, Down, Left, Right, A, B, Start, Select,
}

const AXIS_DEAD_ZONE: i16 = 10_000;

pub struct Keymaps {
  keymap: HashMap<keyboard::Keycode, InputEvent>,
  padmap: HashMap<controller::Button, InputEvent>,
}
impl Keymaps {
  pub fn new() -> Self {
    use GameInput::*;

    let default_keymap = HashMap::from([
      (Keycode::Z,      InputEvent::Game(A)),
      (Keycode::X,      InputEvent::Game(B)),
      (Keycode::UP,     InputEvent::Game(Up)),
      (Keycode::DOWN,   InputEvent::Game(Down)),
      (Keycode::LEFT,   InputEvent::Game(Left)),
      (Keycode::RIGHT,  InputEvent::Game(Right)),
      (Keycode::N,      InputEvent::Game(Select)),
      (Keycode::M,      InputEvent::Game(Start)),
      (Keycode::Space,  InputEvent::Pause),
      (Keycode::R,      InputEvent::Reset),
    ]);

    let default_padmap = HashMap::from([
      (Button::X,         InputEvent::Game(A)),
      (Button::A,         InputEvent::Game(B)),
      (Button::B,         InputEvent::Game(Start)),
      (Button::Y,         InputEvent::Game(Select)),
      (Button::Back,      InputEvent::Game(Select)),
      (Button::Start,     InputEvent::Game(Start)),
      (Button::DPadLeft,  InputEvent::Game(Left)),
      (Button::DPadRight, InputEvent::Game(Right)),
      (Button::DPadUp,    InputEvent::Game(Up)),
      (Button::DPadDown,  InputEvent::Game(Down)),
    ]);

    Keymaps { keymap: default_keymap, padmap: default_padmap }
  }
}

fn match_input(emu: &mut Box<dyn Emulator>, input: Option<&InputEvent>, kind: InputKind, audio_dev: &AudioQueue<f32>) {
  match input {
    Some(InputEvent::Game(input)) => emu.input_event(input, kind),
    Some(InputEvent::Pause) => {
      emu.pause();
      match audio_dev.status() {
        AudioStatus::Playing => audio_dev.pause(),
        _ => audio_dev.resume(),
      }
    }
    Some(InputEvent::Reset) => { 
      emu.reset();
      audio_dev.pause();
      audio_dev.clear();
      audio_dev.resume();
    }
    _ => {}
  }
}

pub fn handle_input(keys: &Keymaps, event: &Event, emu: &mut Box<dyn Emulator>, audio_dev: &AudioQueue<f32>) {
  match event {
    Event::KeyDown { keycode, .. } => if let Some(keycode) = keycode {
      match_input(emu, keys.keymap.get(keycode), InputKind::Press, audio_dev);
    },
    Event::KeyUp { keycode, .. } => if let Some(keycode) = keycode {
      match_input(emu, keys.keymap.get(keycode), InputKind::Release, audio_dev);
    },

    Event::ControllerButtonDown { button, .. } => {
      match_input(emu, keys.padmap.get(button), InputKind::Press, audio_dev);
    },
    Event::ControllerButtonUp { button, .. } => {
      match_input(emu, keys.padmap.get(button), InputKind::Release, audio_dev);
    },

    Event::ControllerAxisMotion { axis: Axis::LeftX, value, .. } => {
        if *value > AXIS_DEAD_ZONE { emu.input_event(&GameInput::Right, InputKind::Press); }
        else if *value < -AXIS_DEAD_ZONE { emu.input_event(&GameInput::Left, InputKind::Press); }
        else {
          emu.input_event(&GameInput::Left, InputKind::Release);
          emu.input_event(&GameInput::Right, InputKind::Release);
        }
      }
      Event::ControllerAxisMotion { axis: Axis::LeftY, value, .. } => {
        if *value > AXIS_DEAD_ZONE { emu.input_event(&GameInput::Down, InputKind::Press); }
        else if *value < -AXIS_DEAD_ZONE { emu.input_event(&GameInput::Up, InputKind::Press); }
        else {
          emu.input_event(&GameInput::Up, InputKind::Release);
          emu.input_event(&GameInput::Down, InputKind::Release);
        }
      }
    _ => {}
  }
}