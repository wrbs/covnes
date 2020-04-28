use covnes::nes::Nes;
use covnes::nes::io::{StandardControllerButtons, SingleStandardControllerIO, SingleStandardController};
use covnes::nes::{mappers, palette};
use failure::{err_msg, Error, bail};
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Scancode};
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use std::time::{Duration, Instant};
use structopt::StructOpt;
use std::path::{PathBuf, Path};
use std::cell::Cell;
use sdl2::Sdl;
use covnes::romfiles::RomFile;
use covnes::nes::cpu::CpuHostAccess;
use covnes::nes::ppu::PPUHostAccess;
use covnes::fm2_movie_file::{FM2File, Command, InputDevice, GamepadInput, ControllerConfiguration};
use std::fs::File;

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

    #[structopt(short="m", long="movie_file", parse(from_os_str))]
    movie_file: Option<PathBuf>,

    #[structopt(short="s", long="sync_frames")]
    sync_frames: Option<i32>
}

fn main() -> Result<(), Error> {
    let opt: Opt = Opt::from_args();
    let mut movie = if let Some(m) = opt.movie_file {
        Some(parse_movie_file(&m)?)
    } else {
        None
    };

    let scale = 3;
    let rom = RomFile::from_filename(opt.romfile)?;
    let io = SdlIO::new();
    let mut nes = Nes::new(SingleStandardController::new(io));
    let cart = mappers::from_rom(rom)?;

    nes.insert_cartridge(cart);
    // nes.step_frame();
    // nes.step_frame();

    let sdl_context = sdl2::init().map_err(err_msg)?;
    let video_subsystem = sdl_context.video().map_err(err_msg)?;

    let window = video_subsystem
        .window("covnes", 256 * scale, 240 * scale)
        .position_centered()
        .build()?;

    let mut canvas = window.into_canvas().present_vsync().build().map_err(err_msg)?;

    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();
    canvas.present();
    
    let mut event_pump = sdl_context.event_pump().map_err(err_msg)?;
    let start = Instant::now();
    let mut offset = Duration::from_secs(0);
    let mut time_stepping = 0.0;
    let mut time_rendering = 0.0;
    let mut fc = 0;
    let mut last_fc = 0;
    let mut last_time = Instant::now();
    'running: loop {
        let ps = Instant::now();
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
        time_rendering += ps.elapsed().as_secs_f32();

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

        match &mut movie {
            Some((commands, buttons)) => {
                if let Some(c) = commands.pop() {
                    if c.contains(Command::SOFT_RESET) {
                        nes.reset();
                    }
                }
                if let Some(b) = buttons.pop() {
                    nes.io.io.current_key_state.set(b);
                } else {
                    nes.io.io.current_key_state.set(StandardControllerButtons::empty());
                }
            },
            None => {
                let mut buttons = StandardControllerButtons::empty();
                let keys = event_pump.keyboard_state();
                for &(sc, k) in KEYMAP {
                    if keys.is_scancode_pressed(sc) {
                        buttons |= k;
                    }
                }
                nes.io.io.current_key_state.set(buttons);
            }
        }


        let ps = Instant::now();
        nes.step_frame();
        time_stepping += ps.elapsed().as_secs_f32();

        canvas.present();

        fc += 1;

        if last_time.elapsed().as_secs_f32() > 1.0 {

            let ms_per_frame = 1000.0 / (fc - last_fc) as f32;
            canvas.window_mut().set_title(format!("covnes: {:.2}ms/frame", ms_per_frame).as_str());
            last_fc = fc;
            last_time += Duration::from_secs(1);
        }

    }

    let elapsed = start.elapsed();
    println!("{} frames in {:?} = {} ms/frame, {} average fps", fc, elapsed, 1000.0 * elapsed.as_secs_f32() / fc as f32, fc as f32 / elapsed.as_secs_f32());

    let step_per_frame = time_stepping / fc as f32;
    println!("Spent {}ms stepping each frame: {}%", step_per_frame * 1000.0, time_stepping / elapsed.as_secs_f32());
    let render_per_frame = time_rendering / fc as f32;
    println!("Spent {}ms rendering each frame: {}%", render_per_frame * 1000.0, time_rendering / elapsed.as_secs_f32());
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

fn parse_movie_file(filename: &Path) -> Result<(Vec<Command>, Vec<GamepadInput>), Error> {
    let mut f = File::open(filename)?;
    let mut fm2 = FM2File::parse(&mut f)?;
    if fm2.pal_flag || fm2.fds {
        bail!("Unsupported movie (pal or fds)");
    }
    let mut commands = fm2.commands;
    let mut buttons = match fm2.controllers {
        ControllerConfiguration::Fourscore(_) => bail!("No fourescore please"),
        ControllerConfiguration::Ports { port0, .. } => {
            match port0 {
                InputDevice::None => bail!("At least give me a controller!"),
                InputDevice::Gamepad(b) => b,
                InputDevice::Zapper(_) => bail!("I don't get zapper"),
            }
        },
    };
    commands.reverse();
    buttons.reverse();

    // We tend to be one frame ahead of FCEUX
//    commands.pop();
//    buttons.pop();

    Ok((commands, buttons))
}
