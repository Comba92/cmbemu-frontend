use std::{error::Error, fs, io::Read, path::{Path, PathBuf}};
use sdl2::{audio::AudioQueue, event::Event, pixels::PixelFormatEnum, render::{Canvas, Texture, TextureCreator}, video::{Window, WindowContext}, AudioSubsystem};
use std::time::{Duration, Instant};

mod emu;
use emu::Emulator;

mod sdl2ctx;
use sdl2ctx::Sdl2Context;

mod input;
use input::{handle_input, Keymaps};

extern crate nen_emulator;
use nen_emulator::{cart::is_nes_rom, nes::Nes};

extern crate tomboy_emulator;
use tomboy_emulator::{cart::is_gb_rom, gb::Gameboy};

fn open_rom(path: &Path) -> Result<Emulator, Box<dyn Error>> {
	let mut bytes = Vec::new();
	let file = fs::File::open(path)?;
			
	let _ = zip::read::ZipArchive::new(file)
		.and_then(|mut archive|
			// we only take the first file in the archive, might be done in a smarter way
			archive.by_index(0)
			.map(|mut f| f.read_to_end(&mut bytes))
		).or_else(|_| 
			fs::File::open(path).map(|mut f| f.read_to_end(&mut bytes))
		)?;

	
	if is_nes_rom(&bytes) {
		Nes::boot_from_bytes(&bytes)
		.map(|x| Box::new(x) as Emulator)
		.map_err(|msg| msg.into())
	} else if is_gb_rom(&bytes) {
		Gameboy::boot_from_bytes(&bytes)
		.map(|x| Box::new(x) as Emulator)
		.map_err(|msg| msg.into())
	} else {
		Err("No valid ROM".into())
	}
}

struct EmuContext {
	emu: Emulator,
	is_paused: bool,
	is_muted: bool,
	ms_frame: Duration,

	audio_dev: AudioQueue<f32>,
	rom_path: PathBuf,

	keys: Keymaps,
}
impl EmuContext {
	pub fn new(sdl: &Sdl2Context) -> Self {
		let emu = Box::new(Nes::boot_empty()) as Emulator;

		let audio_dev = sdl.audio_subsystem
			.open_queue(None, &emu.audio_spec().1).unwrap();

		let ms_frame = Duration::ZERO;
		let keys = Keymaps::default();

		Self { emu, ms_frame, audio_dev, rom_path: PathBuf::new(), keys, is_muted: true, is_paused: true, }
	}

	pub fn try_init(&mut self, rom_path: &Path, canvas: &mut Canvas<Window>, audio: &AudioSubsystem) -> Result<(), Box<dyn Error>> {
		let emu = open_rom(rom_path)?;

		let (width, height) = emu.resolution();
		canvas.set_logical_size(width as u32, height as u32)?;

		let (audio_enabled, spec) = emu.audio_spec();
		let audio_dev = audio
			.open_queue(None, &spec)?;

		audio_dev.clear();
		if audio_enabled { audio_dev.resume(); }

		self.is_paused = false;
		self.is_muted = !audio_enabled;
		self.ms_frame = Duration::from_secs_f32(1.0 / emu.fps());		
		self.rom_path = rom_path.into();
		self.audio_dev = audio_dev;
		self.emu = emu;

		Ok(())
	}
}

fn new_texture<'a>(ctx: &EmuContext, creator: &'a TextureCreator<WindowContext>) -> Texture<'a> {
	let (width, height) = ctx.emu.resolution();
	creator
		.create_texture_target(PixelFormatEnum::RGBA32, width as u32, height as u32)
		.unwrap()
}

fn main() {
	const SCALE: f32 = 3.5;
	const WINDOW_WIDTH:  u32  = (SCALE * 30 as f32 * 8.0) as u32;
	const WINDOW_HEIGHT: u32  = (SCALE * 30 as f32 * 8.0) as u32;
			
	let mut sdl = Sdl2Context
		::new("CMB Emu", WINDOW_WIDTH, WINDOW_HEIGHT)
		.unwrap();
	
	// Just default it to NES
	let mut ctx = EmuContext::new(&sdl);

	let texture_creator = sdl.canvas.texture_creator();
	let mut texture = new_texture(&ctx, &texture_creator);

	'running: loop {
		let ms_since_start = Instant::now();

		if !ctx.is_paused {
			ctx.emu.step_one_frame();
			
			if !ctx.is_muted && ctx.audio_dev.size() < 735*3 {
				ctx.emu.step_one_frame();
			}
			
			if ctx.is_muted {
				ctx.emu.samples();
			} else {
				ctx.audio_dev.queue_audio(&ctx.emu.samples()).unwrap();
			}
		}

		for event in sdl.events.poll_iter() {
			handle_input(&mut ctx, &event);

			match event {
				Event::Quit { .. } => {
					ctx.audio_dev.pause();
					break 'running;
				}
				Event::DropFile { filename, .. } => {
					let _  = ctx
					.try_init(&PathBuf::from(filename), &mut sdl.canvas, &sdl.audio_subsystem)
					.inspect_err(|msg| eprintln!("{msg}\n"));

					texture = new_texture(&ctx, &texture_creator);
				}
				Event::ControllerDeviceAdded { which , .. } => {
					match sdl.controller_subsystem.open(which) {
						Ok(controller) => {
							eprintln!("Found controller: {}\n", controller.name());
							sdl.controllers.push(controller);
						}
						Err(_) => eprintln!("A controller was connected, but I couldn't initialize it\n")
					}
				}
				_ => {}
			}
		}

		sdl.canvas.clear();
		let (framebuf, pitch) = ctx.emu.framebuf();
		texture.update(None, &framebuf, pitch).unwrap();
		sdl.canvas.copy(&texture, None, None).unwrap();
		sdl.canvas.present();

		let ms_elapsed = Instant::now() - ms_since_start;
		if ctx.ms_frame > ms_elapsed {
			std::thread::sleep(ctx.ms_frame - ms_elapsed);
		}
	}
}

#[cfg(test)]
mod testing {
    use std::io::{Read, Write};

    use nen_emulator::nes::Nes;

	#[test]
	fn ser_de() {
		let test_rom = std::fs::read("./nen-emulator/roms/Tetris (USA).nes").unwrap();

		let mut nes = Nes::boot_from_bytes(&test_rom).unwrap();

		let mut file = std::fs::File::create("test.sav").unwrap();		
		bincode::serialize_into(&file, &nes).unwrap();
		// let ser = ron::to_string(&nes).unwrap();
		// file.write_fmt(format_args!("{ser}")).unwrap();

		let mut file = std::fs::File::open("test.sav").unwrap();
		nes = bincode::deserialize_from(file).unwrap();
		// let mut de = String::new();
		// file.read_to_string(&mut de).unwrap();
		// nes = ron::from_str(&de).unwrap(); 
	}
}