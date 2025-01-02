use std::error::Error;
use sdl2::{controller::GameController, render::{Canvas, TextureCreator}, video::{Window, WindowContext}, AudioSubsystem, EventPump, GameControllerSubsystem, Sdl, VideoSubsystem};

use crate::input::Keymaps;

#[allow(unused)]
pub struct Sdl2Context {
  pub ctx: Sdl,
  pub video_subsystem: VideoSubsystem,
  pub audio_subsystem: AudioSubsystem,
  pub canvas: Canvas<Window>,
  pub texture_creator: TextureCreator<WindowContext>,
  pub events: EventPump,
  pub controller_subsystem: GameControllerSubsystem,
  pub controllers: Vec<GameController>,
  pub keymaps: Keymaps,
}

impl Sdl2Context {
  pub fn new(name: &str, width: u32, height: u32) -> Result<Self, Box<dyn Error>> {
    let ctx = sdl2::init()?;
    let video_subsystem= ctx.video()?;
    let audio_subsystem = ctx.audio()?;
    let window = video_subsystem.window(name, width, height)
        .position_centered()
        .resizable()
        .build()?;
    let canvas = window
        .into_canvas()
        .accelerated()
        // .present_vsync()
        .build()?;
    
    let texture_creator = canvas.texture_creator();
    let controller_subsystem = ctx.game_controller()?;
    let controllers = Vec::new();
    
    let events = ctx.event_pump()?;
    let keymaps = Keymaps::new();

    Ok(
      Self { ctx, video_subsystem, audio_subsystem, canvas, events, texture_creator, controller_subsystem, controllers, keymaps }
    )
  }
}
