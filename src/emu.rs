use std::{fs, io::{Read, Write}, path::{Path, PathBuf}};

use nen_emulator::{nes::Nes, joypad::JoypadButton as NesButton};
use tomboy_emulator::{gb::Gameboy, joypad::Flags as GbButton};
use sdl2::audio::AudioSpecDesired;

use crate::input::{GameInput, InputKind};

pub type Emulator = Box<dyn EmuInterface>;
pub trait EmuInterface {
  fn step_one_frame(&mut self);
  fn framebuf(&mut self) -> (&[u8], usize);
  fn samples(&mut self) -> Vec<f32>;
  fn resolution(&self) -> (usize, usize);
  fn fps(&self) -> f32;
  fn audio_spec(&self) -> (bool, AudioSpecDesired);
  fn input_event(&mut self, button: &GameInput, kind: InputKind);
  fn reset(&mut self);

  fn save(&self, _path: &Path) {}
  fn load(&mut self, _path: &Path) {}
}

impl EmuInterface for Nes {
  fn step_one_frame(&mut self) { self.step_until_vblank(); }

  fn framebuf(&mut self) -> (&[u8], usize) { (&self.get_screen().buffer, self.get_screen().pitch()) }
  fn samples(&mut self) -> Vec<f32> { self.get_samples() }

  fn resolution(&self) -> (usize, usize) { (32*8, 30*8) }
  fn fps(&self) -> f32 { self.get_fps() }

  fn audio_spec(&self) -> (bool, AudioSpecDesired) {
    let spec = AudioSpecDesired { freq: Some(44100), channels: Some(1), samples: None, };
    (true, spec)
  }

  fn input_event(&mut self, button: &GameInput, kind: InputKind) {
    let method: fn(&mut Nes, NesButton) = match kind {
      InputKind::Press   => |nes, btn| nes.get_joypad().buttons1.insert(btn),
      InputKind::Release => |nes, btn| nes.get_joypad().buttons1.remove(btn),
    };

    match button {
        GameInput::Up     => method(self, NesButton::up),
        GameInput::Down   => method(self, NesButton::down),
        GameInput::Left   => method(self, NesButton::left),
        GameInput::Right  => method(self, NesButton::right),
        GameInput::A      => method(self, NesButton::a),
        GameInput::B      => method(self, NesButton::b),
        GameInput::Start  => method(self, NesButton::start),
        GameInput::Select => method(self, NesButton::select),
    }
  }

  fn reset(&mut self) { self.reset(); }
  
  fn save(&self, path: &Path) {
    let filename = PathBuf::from(path).with_extension(".sav");
    let mut file = fs::File::create(filename).unwrap();

    // let _ = bincode::serialize_into(file, self)
    //   .map_err(|msg| eprintln!("Couldn't save: {msg}\n"));

    let ser = ron::to_string(&self).unwrap();
		file.write_fmt(format_args!("{ser}")).unwrap();
  }

  fn load(&mut self, path: &Path) {
    let filename = PathBuf::from(path).with_extension(".sav");
    let file = fs::File::open(filename);

    match file {
      Ok(mut file) => {
        // let mut new_emu: Self = bincode::deserialize_from(file).unwrap();

        let mut de = String::new();
        file.read_to_string(&mut de).unwrap();
        let mut new_emu: Self = ron::from_str(&de).unwrap(); 

        new_emu.load_rom_only(&self.get_bus().cart.borrow());
        *self = new_emu;
      }
      Err(e) => eprintln!("No save found: {e}\n"),
    }
    
  }
}

impl EmuInterface for Gameboy {
  fn step_one_frame(&mut self) { self.step_until_vblank(); }
 
  fn framebuf(&mut self) -> (&[u8], usize) {
    let lcd = &self.get_ppu().lcd;
    (&lcd.buffer, lcd.pitch())
  }

  fn samples(&mut self) -> Vec<f32> { Vec::new() }
  fn resolution(&self) -> (usize, usize) { (160, 144) }
  fn fps(&self) -> f32 { 60.0 }

  fn audio_spec(&self) -> (bool, AudioSpecDesired) {
    let spec = AudioSpecDesired { channels: Some(2), freq: Some(44100), samples: None };
    // TODO: as gb doesn't yet have sound functionality, it is disabled by default
    (false, spec)
  }

  fn input_event(&mut self, button: &GameInput, kind: InputKind) {
    let method_btn: fn(&mut Gameboy, GbButton) = match kind {
      InputKind::Press   => |gb, btn| gb.get_joypad().button_pressed(btn),
      InputKind::Release => |gb, btn| gb.get_joypad().button_released(btn)
    };
    let method_dpad: fn(&mut Gameboy, GbButton) = match kind {
      InputKind::Press   => |gb, btn| gb.get_joypad().dpad_pressed(btn),
      InputKind::Release => |gb, btn| gb.get_joypad().dpad_released(btn)
    };

    match button {
        GameInput::Up     => method_dpad(self, GbButton::select_up),
        GameInput::Down   => method_dpad(self, GbButton::start_down),
        GameInput::Left   => method_dpad(self, GbButton::b_left),
        GameInput::Right  => method_dpad(self, GbButton::a_right),
        GameInput::A      => method_btn(self, GbButton::a_right),
        GameInput::B      => method_btn(self, GbButton::b_left),
        GameInput::Start  => method_btn(self, GbButton::start_down),
        GameInput::Select => method_btn(self, GbButton::select_up),
    }
  }

  fn reset(&mut self) {}
}