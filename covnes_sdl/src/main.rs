use covnes::system::Nes;
use covnes::io::{StandardControllerButtons, SingleStandardControllerIO, SingleStandardController};
use covnes::{mappers, palette};
use failure::{err_msg, Error};
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Scancode};
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use std::time::{Duration, Instant};
use structopt::StructOpt;
use std::path::PathBuf;
use std::cell::Cell;
use sdl2::Sdl;
use covnes::romfiles::RomFile;
use covnes::cpu::CpuHostAccess;
use covnes::ppu::PPUHostAccess;
use covnes::system::DMAState::DummyRead;

const KEYMAP: &[(Scancode, StandardControllerButtons)] = &[
    (Scancode::W, StandardControllerButtons::UP),
    (Scancode::A, StandardControllerButtons::LEFT),
    (Scancode::S, StandardControllerButtons::DOWN),
    (Scancode::D, StandardControllerButtons::RIGHT),
    (Scancode::J, StandardControllerButtons::A),
    (Scancode::K, StandardControllerButtons::B),
    (Scancode::U, StandardControllerButtons::SELECT),
    (Scancode::I, StandardControllerButtons::START),
];

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
    let mut nes = Nes::new(SingleStandardController::new(io));
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
    let start = Instant::now();
    let mut offset = Duration::from_secs(0);
    let mut fc = 0;
    'running: loop {
        let frame_start = Instant::now();
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();
        for row in 0..240 {
            for col in 0..256 {
                let (r, g, b) = nes.io.io.get_pixel(row, col);
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

        let mut buttons = StandardControllerButtons::empty();
        let keys = event_pump.keyboard_state();
        for &(sc, k) in KEYMAP {
            if keys.is_scancode_pressed(sc) {
                buttons |= k;
            }
        }

        nes.io.io.current_key_state.set(buttons);

        nes.step_frame();

        fc += 1;

        canvas.present();

        // let elapsed = frame_start.elapsed();
        // let target = Duration::from_secs_f32(1.0 / 60.0);
        // if elapsed < target {
        //     std::thread::sleep(target - elapsed);
        // }
    }

    let elapsed = start.elapsed();
    println!("{} frames in {:?} = {} average fps", fc, elapsed, fc as f32 / elapsed.as_secs_f32());

    Ok(())
}

struct SdlIO {
    pixels: Cell<[(u8, u8, u8); 256 * 240]>,
    current_key_state: Cell<StandardControllerButtons>,
}

impl SdlIO {
    fn new() -> SdlIO {
        SdlIO {
            pixels: Cell::new([(0, 0, 0); 256 * 240]),
            current_key_state: Cell::new(StandardControllerButtons::empty())
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

impl SingleStandardControllerIO for SdlIO {
    fn set_pixel(&self, row: u16, col: u16, r: u8, g: u8, b: u8) {
        self.pixels()[row as usize * 256 + col as usize].set((r, g, b));
    }

    fn poll_buttons(&self) -> StandardControllerButtons {
        self.current_key_state.get()
    }
}