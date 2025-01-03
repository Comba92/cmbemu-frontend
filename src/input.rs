use std::collections::HashMap;

use sdl2::{audio::AudioStatus, controller::{self, Axis, Button}, event::Event, keyboard::{self, Keycode}};

use crate::EmuContext;

pub enum InputKind {
  Press, Release
}

#[derive(Clone, Copy)]
pub enum InputEvent {
  Game(GameInput),
  Pause, Reset, Save, Load, Mute,
}

#[derive(Clone, Copy)]
pub enum GameInput {
  Up, Down, Left, Right, A, B, Start, Select,
}

const AXIS_DEAD_ZONE: i16 = 10_000;

pub struct Keymaps {
  keymap: HashMap<keyboard::Keycode, InputEvent>,
  padmap: HashMap<controller::Button, InputEvent>,
}
impl Default for Keymaps {
  fn default() -> Self {
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

fn match_input(ctx: &mut EmuContext, input: Option<InputEvent>, kind: InputKind) {
  let emu = &mut ctx.emu;
  let audio_dev = &ctx.audio_dev;

  match input {
    Some(InputEvent::Game(input)) => emu.input_event(&input, kind),
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

pub fn handle_input(ctx: &mut EmuContext, event: &Event) {
  match event {
    Event::KeyDown { keycode, .. } => if let Some(keycode) = keycode {
      let input = ctx.keys.keymap.get(keycode).map(|x| x.to_owned());
      match_input(ctx, input, InputKind::Press);
    },
    Event::KeyUp { keycode, .. } => if let Some(keycode) = keycode {
      let input = ctx.keys.keymap.get(keycode).map(|x| x.to_owned());
      match_input(ctx, input, InputKind::Release);
    },

    Event::ControllerButtonDown { button, .. } => {
      let input = ctx.keys.padmap.get(button).map(|x| x.to_owned());
      match_input(ctx, input, InputKind::Press);
    },
    Event::ControllerButtonUp { button, .. } => {
      let input = ctx.keys.padmap.get(button).map(|x| x.to_owned());
      match_input(ctx, input, InputKind::Release);
    },

    Event::ControllerAxisMotion { axis: Axis::LeftX, value, .. } => {
        if *value > AXIS_DEAD_ZONE { ctx.emu.input_event(&GameInput::Right, InputKind::Press); }
        else if *value < -AXIS_DEAD_ZONE { ctx.emu.input_event(&GameInput::Left, InputKind::Press); }
        else {
          ctx.emu.input_event(&GameInput::Left, InputKind::Release);
          ctx.emu.input_event(&GameInput::Right, InputKind::Release);
        }
      }
      Event::ControllerAxisMotion { axis: Axis::LeftY, value, .. } => {
        if *value > AXIS_DEAD_ZONE { ctx.emu.input_event(&GameInput::Down, InputKind::Press); }
        else if *value < -AXIS_DEAD_ZONE { ctx.emu.input_event(&GameInput::Up, InputKind::Press); }
        else {
          ctx.emu.input_event(&GameInput::Up, InputKind::Release);
          ctx.emu.input_event(&GameInput::Down, InputKind::Release);
        }
      }
    _ => {}
  }
}