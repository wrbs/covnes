use covnes::romfiles::RomFile;
use covnes::system::Nes;
use covnes::mappers;
use failure::{err_msg, Error};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use std::time::Duration;
use structopt::StructOpt;
use std::path::PathBuf;

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
    let mut nes = Nes::new();
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
    'running: loop {
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();
        for row in 0..240 {
            for col in 0..256 {
                let colour = nes.get_pixel(row, col);
                canvas.set_draw_color(Color::RGB(colour.r, colour.g, colour.b));
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

        canvas.present();
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }

    Ok(())
}
