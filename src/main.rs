use std::{env::args, error::Error, fs, io::Read, path::{Path, PathBuf}};
extern crate nen_emulator;
use input::handle_input;
use nen_emulator::{cart::Cart, frame::{SCREEN_HEIGHT, SCREEN_WIDTH}, nes::Nes};
use sdl2::{audio::AudioSpecDesired, event::Event, pixels::PixelFormatEnum};
use std::time::{Duration, Instant};

mod sdl2ctx;
use sdl2ctx::Sdl2Context;

mod input;


fn open_rom(path: &Path) -> Result<Cart, Box<dyn Error>> {
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

    Cart::new(&bytes).map_err(|msg| msg.into())
}

fn main() {
    const SCALE: f32 = 3.5;
    const WINDOW_WIDTH:  u32  = (SCALE * SCREEN_WIDTH  as f32* 8.0) as u32;
    const WINDOW_HEIGHT: u32  = (SCALE * SCREEN_HEIGHT as f32* 8.0) as u32;
    let ms_frame: Duration = Duration::from_secs_f64(1.0 / 60.0988);

    let mut sdl = Sdl2Context
        ::new("NenEmulator", WINDOW_WIDTH, WINDOW_HEIGHT)
        .unwrap();
    
    // Keep aspect ratio
    sdl.canvas.set_logical_size(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32).unwrap();

    let filename = args().nth(1);
    let rom_path = if let Some(filename) = filename {
        PathBuf::from(filename)
    } else { PathBuf::from("") };

    let mut emu = Nes::empty();
    if rom_path.exists() {
        let cart = open_rom(&rom_path);
        if let Ok(cart) = cart {
            let rom_name =  rom_path.file_name().unwrap().to_str().unwrap_or("NenEmulator");
            sdl.canvas.window_mut().set_title(rom_name).expect("Couldn't rename window title");
            emu = Nes::with_cart(cart);
        }
    }

    let mut texture = sdl.texture_creator.create_texture_target(
        PixelFormatEnum::RGBA32, emu.get_screen().width as u32, emu.get_screen().height as u32
    ).unwrap();

    let desired_spec = AudioSpecDesired {
        freq: Some(44100),
        channels: Some(1),
        samples: None,
    };

    let audio_dev = sdl.audio_subsystem
        .open_queue::<f32, _>(None, &desired_spec).unwrap();

    audio_dev.resume();

    'running: loop {
        let ms_since_start = Instant::now();

        if !emu.is_paused {
            emu.step_until_vblank();
            
            if audio_dev.size() < 2096 {
                emu.step_until_vblank();
            }

            audio_dev.queue_audio(&emu.get_samples()).unwrap();
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
                        Ok(cart) => {
                            let rom_name =  rom_path.file_name().unwrap().to_str().unwrap_or("NenEmulator");
                            sdl.canvas.window_mut().set_title(rom_name).expect("Couldn't rename window title");
                            emu.load_cart(cart);
                        }
                        Err(msg) => eprintln!("Couldn't load the rom: {msg}\n"),
                    };

                    audio_dev.resume();
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
        texture.update(None, &emu.get_screen().buffer, emu.get_screen().pitch()).unwrap();
        sdl.canvas.copy(&texture, None, None).unwrap();
        sdl.canvas.present();

        let ms_elapsed = Instant::now() - ms_since_start;
        if ms_frame > ms_elapsed {
            std::thread::sleep(ms_frame - ms_elapsed);
        }
    }
}
