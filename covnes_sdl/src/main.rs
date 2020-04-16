use covnes::romfiles::RomFile;
use covnes::system::{Nes, IO};
use covnes::mappers;
use failure::{err_msg, Error};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use std::time::{Duration, SystemTime};
use structopt::StructOpt;
use std::path::PathBuf;
use std::cell::Cell;
use sdl2::Sdl;

#[derive(Debug, StructOpt)]
struct Opt {
    /// ROM file to load in iNES format
    #[structopt(parse(from_os_str))]
    romfile: PathBuf,
}

fn main() -> Result<(), Error> {
    let opt = Opt::from_args();
    let scale = 3;
    let rom = RomFile::from_filename(opt.romfile)?;
    let io = SdlIO::new();
    let mut nes = Nes::new(io);
    let cart = mappers::from_rom(rom)?;

    nes.insert_cartridge(cart);

    let sdl_context = sdl2::init().map_err(err_msg)?;
    let video_subsystem = sdl_context.video().map_err(err_msg)?;

    let window = video_subsystem
        .window("covnes", 256 * scale, 240 * scale)
        .position_centered()
        .build()?;

    let mut canvas = window.into_canvas().build().map_err(err_msg)?;

    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();
    canvas.present();
    
    let mut event_pump = sdl_context.event_pump().map_err(err_msg)?;
    let mut fc = 0;
    let start = SystemTime::now();
    'running: loop {
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();
        for row in 0..240 {
            for col in 0..256 {
                let (r, g, b) = nes.io.get_pixel(row, col);
                canvas.set_draw_color(Color::RGB(r, g, b));
                canvas
                    .fill_rect(Rect::new(
                        col as i32 * scale as i32,
                        row as i32 * scale as i32,
                        scale,
                        scale,
                    ))
                    .map_err(err_msg)?;
            }
        }

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                Event::KeyDown {
                    keycode: Some(k), ..
                } => match k {
                    Keycode::Escape => break 'running,
                    Keycode::T => nes.tick(),
                    Keycode::C => {
                        let cycles = nes.step_cpu_instruction();
                        println!("{} cpu cycles", cycles);
                    }
                    Keycode::F => {
                        let ticks = nes.step_frame();
                        println!("{} ppu ticks", ticks);
                    }
                    _ => (),
                },
                _ => (),
            }
        }
        // The rest of the game loop goes here...

        nes.step_frame();
        fc += 1;

        canvas.present();
    }

    let elapsed = start.elapsed().unwrap();
    println!("{} frames in {:?} = {} average fps", fc, elapsed, fc as f32 / elapsed.as_secs_f32());

    Ok(())
}

struct SdlIO {
    pixels: Cell<[(u8, u8, u8); 256 * 240]>,
}

impl SdlIO {
    fn new() -> SdlIO {
        SdlIO {
            pixels: Cell::new([(0, 0, 0); 256 * 240])
        }
    }

    fn get_pixel(&self, row: u16, col: u16) -> (u8, u8, u8) {
        self.pixels()[row as usize * 256 + col as usize].get()
    }

    fn pixels(&self) -> &[Cell<(u8, u8, u8)>] {
        let f: &Cell<[(u8, u8, u8)]> = &self.pixels;
        f.as_slice_of_cells()
    }
}

impl IO for SdlIO {
    fn set_pixel(&self, row: u16, col: u16, r: u8, g: u8, b: u8) {
        self.pixels()[row as usize * 256 + col as usize].set((r, g, b));
    }
}