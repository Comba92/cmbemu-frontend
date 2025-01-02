use nen_emulator::{nes::Nes, joypad::JoypadButton as NesButton};
use sdl2::audio::AudioSpecDesired;

use crate::input::{GameInput, InputEvent, InputKind};

pub trait Emulator {
  fn step_one_frame(&mut self);
  fn framebuf(&mut self) -> (&[u8], usize);
  fn samples(&mut self) -> Vec<f32>;
  fn resolution(&self) -> (usize, usize);
  fn fps(&mut self) -> f32;
  fn audio_spec(&self) -> AudioSpecDesired;
  fn input_event(&mut self, event: &GameInput, kind: InputKind);

  fn pause(&mut self);
  fn is_paused(&self) -> bool;
  fn reset(&mut self);
}

impl Emulator for Nes {
  fn step_one_frame(&mut self) { self.step_until_vblank(); }

  fn framebuf(&mut self) -> (&[u8], usize) { (&self.get_screen().buffer, self.get_screen().pitch()) }
  fn samples(&mut self) -> Vec<f32> { self.get_samples() }

  fn resolution(&self) -> (usize, usize) { (32*8, 30*8) }
  fn fps(&mut self) -> f32 { self.get_fps() }

  fn audio_spec(&self) -> AudioSpecDesired {
    AudioSpecDesired {
      freq: Some(44100),
      channels: Some(1),
      samples: None,
    }
  }

  fn input_event(&mut self, button: &GameInput, kind: InputKind) {
    let method: fn(&mut Nes, NesButton) = match kind {
      InputKind::Press   => |nes, btn| nes.get_joypad().buttons1.insert(btn),
      InputKind::Release => |nes, btn| nes.get_joypad().buttons1.remove(btn),
    };

    match button {
        GameInput::Up     => method(self, NesButton::UP),
        GameInput::Down   => method(self, NesButton::DOWN),
        GameInput::Left   => method(self, NesButton::LEFT),
        GameInput::Right  => method(self, NesButton::RIGHT),
        GameInput::A      => method(self, NesButton::A),
        GameInput::B      => method(self, NesButton::B),
        GameInput::Start  => method(self, NesButton::START),
        GameInput::Select => method(self, NesButton::SELECT),
    }
  }

  fn pause(&mut self) { self.is_paused = !self.is_paused; }
  fn is_paused(&self) -> bool { self.is_paused }
  fn reset(&mut self) { self.reset(); }
}