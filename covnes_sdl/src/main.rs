mod emulator;
mod timer;
use std::{
    fs::File,
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::{anyhow, bail, Result};
use covnes::{
    fm2_movie_file::{Command, ControllerConfiguration, FM2File, GamepadInput, InputDevice},
    nes::{io::StandardControllerButtons, mappers},
    romfiles::RomFile,
};
use sdl2::{
    event::Event,
    keyboard::{Keycode, Scancode},
    pixels::Color,
    rect::Rect,
    render::Canvas,
    video::Window,
    EventPump,
};
use structopt::StructOpt;
use timer::{TickResult, Timer};

use crate::emulator::Emulator;

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

pub const TARGET_FRAMERATE: f32 = 1789772.7272727 / 29780.5;
pub const SCALE: u32 = 3;

#[derive(Debug, StructOpt)]
struct Opt {
    /// ROM file to load in iNES format
    #[structopt(parse(from_os_str))]
    romfile: PathBuf,

    #[structopt(short = "m", long = "movie_file", parse(from_os_str))]
    movie_file: Option<PathBuf>,
}

struct Ui {
    emulator: Emulator,
    movie: Option<(Vec<Command>, Vec<StandardControllerButtons>)>,
    canvas: Canvas<Window>,
    event_pump: EventPump,
    timer: Timer,
    time_rendering: f32,
    time_waiting_for_next_frame: f32,
}

fn sdl_error(error: String) -> anyhow::Error {
    anyhow!("SDL error: {}", error)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BreakOrContinue {
    Break,
    Continue,
}

fn main() -> Result<()> {
    let opt: Opt = Opt::from_args();
    let movie = if let Some(m) = opt.movie_file {
        Some(parse_movie_file(&m)?)
    } else {
        None
    };

    let scale = 3;
    let rom = RomFile::from_filename(opt.romfile)?;
    let cart = mappers::from_rom(rom)?;

    let emulator = Emulator::new(cart);

    let sdl_context = sdl2::init().map_err(sdl_error)?;
    let video_subsystem = sdl_context.video().map_err(sdl_error)?;

    let window = video_subsystem
        .window("covnes", 256 * scale, 240 * scale)
        .position_centered()
        .build()?;

    let mut canvas = window.into_canvas().present_vsync().build()?;

    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();
    canvas.present();

    let event_pump = sdl_context.event_pump().map_err(sdl_error)?;

    let mut ui = Ui {
        emulator,
        movie,
        canvas,
        event_pump,
        timer: Timer::new(TARGET_FRAMERATE),
        time_rendering: 0.0,
        time_waiting_for_next_frame: 0.0,
    };

    ui.run()
}

impl Ui {
    fn run(&mut self) -> Result<()> {
        'outer: loop {
            let TickResult {
                frames_to_step,
                frame_rate_display_update,
            } = self.timer.tick();

            for _ in 0..frames_to_step {
                match self.process_input() {
                    BreakOrContinue::Break => break 'outer,
                    BreakOrContinue::Continue => (),
                }
                self.emulator.step_frame();
            }

            let ps = Instant::now();
            self.draw_frame();
            self.time_waiting_for_next_frame += ps.elapsed().as_secs_f32();

            if let Some(update) = frame_rate_display_update {
                self.canvas
                    .window_mut()
                    .set_title(&format!("covnes: {}", update))?;
            }
        }

        self.show_counts();
        Ok(())
    }

    fn draw_frame(&mut self) {
        let ps = Instant::now();
        self.canvas.set_draw_color(Color::RGB(0, 0, 0));
        self.canvas.clear();
        let canvas = &mut self.canvas;
        self.emulator.iter_pixels(|row, col, (r, g, b)| {
            canvas.set_draw_color(Color::RGB(r, g, b));
            canvas
                .fill_rect(Rect::new(
                    col as i32 * SCALE as i32,
                    row as i32 * SCALE as i32,
                    SCALE,
                    SCALE,
                ))
                .unwrap()
        });

        self.time_rendering += ps.elapsed().as_secs_f32();
        self.canvas.present();
    }

    fn process_input(&mut self) -> BreakOrContinue {
        for event in self.event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => return BreakOrContinue::Break,
                Event::KeyDown {
                    keycode: Some(k), ..
                } => match k {
                    Keycode::Escape => return BreakOrContinue::Break,
                    _ => (),
                },
                _ => (),
            }
        }
        // The rest of the game loop goes here...

        match &mut self.movie {
            Some((commands, buttons)) => {
                if let Some(c) = commands.pop() {
                    if c.contains(Command::SOFT_RESET) {
                        self.emulator.reset();
                    }
                }
                if let Some(b) = buttons.pop() {
                    self.emulator.set_buttons(b);
                } else {
                    self.emulator
                        .set_buttons(StandardControllerButtons::empty());
                }
            }
            None => {
                let mut buttons = StandardControllerButtons::empty();
                let keys = self.event_pump.keyboard_state();
                for &(sc, k) in KEYMAP {
                    if keys.is_scancode_pressed(sc) {
                        buttons |= k;
                    }
                }
                self.emulator.set_buttons(buttons);
            }
        }

        BreakOrContinue::Continue
    }

    fn show_counts(&self) {
        println!("{}", self.timer.summary_counts());

        let render_per_frame = self.time_rendering / self.timer.render_frame_count() as f32;
        println!(
            "Spent {}ms rendering each frame: {}%",
            render_per_frame * 1000.0,
            self.time_rendering / self.timer.elapsed()
        );
        let wait_per_frame =
            self.time_waiting_for_next_frame / self.timer.render_frame_count() as f32;
        println!(
            "Spent {}ms waiting for steps: {}%",
            wait_per_frame * 1000.0,
            self.time_waiting_for_next_frame / self.timer.elapsed()
        );
    }
}

fn parse_movie_file(filename: &Path) -> Result<(Vec<Command>, Vec<GamepadInput>)> {
    let mut f = File::open(filename)?;
    let fm2 = FM2File::parse(&mut f)?;
    if fm2.pal_flag || fm2.fds {
        bail!("Unsupported movie (pal or fds)");
    }
    let mut commands = fm2.commands;
    let mut buttons = match fm2.controllers {
        ControllerConfiguration::Fourscore(_) => bail!("No fourescore please"),
        ControllerConfiguration::Ports { port0, .. } => match port0 {
            InputDevice::None => bail!("At least give me a controller!"),
            InputDevice::Gamepad(b) => b,
            InputDevice::Zapper(_) => bail!("I don't get zapper"),
        },
    };
    commands.reverse();
    buttons.reverse();

    // We tend to be one frame ahead of FCEUX
    //    commands.pop();
    //    buttons.pop();

    Ok((commands, buttons))
}
