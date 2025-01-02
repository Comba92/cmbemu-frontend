use std::{error::Error, fs, io::Read, path::{Path, PathBuf}};
use sdl2::{audio::AudioQueue, event::Event, pixels::PixelFormatEnum, render::{Canvas, Texture, TextureCreator}, video::{Window, WindowContext}, AudioSubsystem};
use std::time::{Duration, Instant};

mod emu;
use emu::Emulator;

mod sdl2ctx;
use sdl2ctx::Sdl2Context;

mod input;
use input::handle_input;

extern crate nen_emulator;
use nen_emulator::nes::Nes;

extern crate tomboy_emulator;
use tomboy_emulator::cpu::Cpu as Gb;

fn open_rom(path: &Path) -> Result<Box<dyn Emulator>, Box<dyn Error>> {
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

	Nes::from_bytes(&bytes)
		.map(|x| Box::new(x) as Box<dyn Emulator>)
		.map_err(|msg| msg.into())
		.or_else(|_: Box<dyn Error>| Ok(Box::new(Gb::new(&bytes))))
}

fn init_emu<'a>(
	emu: &mut Box<dyn Emulator>,
	audio: &AudioSubsystem,
	canvas: &mut Canvas<Window>,
	creator: &'a TextureCreator<WindowContext>,
) -> (Duration, Texture<'a>, AudioQueue<f32>) {
	let (width, height) = emu.resolution();
	canvas.set_logical_size(width as u32, height as u32).unwrap();

	let texture = creator
		.create_texture_target(PixelFormatEnum::RGBA32, width as u32, height as u32).unwrap();

	let audio_dev = audio
		.open_queue(None, &emu.audio_spec()).unwrap();
			
	if !emu.is_muted() { audio_dev.resume(); }

	let mut fps = emu.fps();
	fps = if fps == 0.0 { 0.0 } else { 1.0 / fps };
	let frame_ms = Duration::from_secs_f32(fps);

	(frame_ms, texture, audio_dev)
}

fn main() {
	const SCALE: f32 = 3.5;
	const WINDOW_WIDTH:  u32  = (SCALE * 30  as f32* 8.0) as u32;
	const WINDOW_HEIGHT: u32  = (SCALE * 30 as f32* 8.0) as u32;
			
	let mut sdl = Sdl2Context
		::new("NenEmulator", WINDOW_WIDTH, WINDOW_HEIGHT)
		.unwrap();
	let texture_creator = sdl.canvas.texture_creator();

	// Just default it to NES
	let mut emu = Box::new(Nes::empty()) as Box<dyn Emulator>;
	let (mut ms_frame, mut texture, mut audio_dev) = init_emu(&mut emu, &sdl.audio_subsystem, &mut sdl.canvas, &texture_creator);

	'running: loop {
		let ms_since_start = Instant::now();

		if !emu.is_paused() {
			emu.step_one_frame();
			
			if !emu.is_muted() && audio_dev.size() < audio_dev.spec().size*2 {
				emu.step_one_frame();
			}

			audio_dev.queue_audio(&emu.samples()).unwrap();
		}

		for event in sdl.events.poll_iter() {
			handle_input(&sdl.keymaps, &event, &mut emu, &audio_dev);

			match event {
				Event::Quit { .. } => {
					audio_dev.pause();
					break 'running;
				}
				Event::DropFile { filename, .. } => {
					audio_dev.pause();
					audio_dev.clear();

					let rom_path = &PathBuf::from(filename);
					let rom_result = open_rom(&rom_path);

					match rom_result {
						Ok(new_emu) => {
							emu = new_emu;
							(ms_frame, texture, audio_dev) = 
								init_emu(&mut emu, &sdl.audio_subsystem, &mut sdl.canvas, &texture_creator);
						}
						Err(msg) => eprintln!("Couldn't load the rom: {msg}\n"),
					};
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
		let (framebuf, pitch) = emu.framebuf();
		texture.update(None, &framebuf, pitch).unwrap();
		sdl.canvas.copy(&texture, None, None).unwrap();
		sdl.canvas.present();

		let ms_elapsed = Instant::now() - ms_since_start;
		if ms_frame > ms_elapsed {
			std::thread::sleep(ms_frame - ms_elapsed);
		}
	}
}
