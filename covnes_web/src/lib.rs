mod utils;

use wasm_bindgen::prelude::*;
use covnes::system::Nes;
use covnes::io::{StandardControllerButtons, SingleStandardControllerIO, SingleStandardController};
use std::cell::Cell;
use covnes::romfiles::RomFile;
use covnes::mappers;

#[wasm_bindgen]
pub fn init() {
    utils::set_panic_hook();
}

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
extern {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn greet() {
    alert("Hello, covnes-web!");
}

#[wasm_bindgen]
pub struct EmulatorState {
    nes: Nes<SingleStandardController<WasmIO>>,
}

#[wasm_bindgen]
impl EmulatorState {
    pub fn new() -> EmulatorState {
        let io = SingleStandardController::new(WasmIO::new());
        EmulatorState {
            nes: Nes::new(io),
        }
    }

    pub fn tick_cycle(&self) -> usize {
        self.nes.step_frame()
    }

    pub fn get_video(&self) -> *mut [u8; 256 * 240 * 3] {
        self.nes.io.io.video_mem.as_ptr()
    }

    pub fn load_rom(&mut self, mut rom: &[u8]) -> Result<(), JsValue> {
        let rom= RomFile::from_read(&mut rom).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let cart = mappers::from_rom(rom).map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.nes.insert_cartridge(cart);
        self.nes.reset();

        Ok(())
    }
}

#[wasm_bindgen]
pub struct WasmIO {
    video_mem: Cell<[u8; 240 * 256 * 3]>,
}

impl WasmIO {
    fn new() -> WasmIO {
        WasmIO {
            video_mem: Cell::new([0; 240 * 256 * 3]),
        }
    }
}

impl SingleStandardControllerIO for WasmIO {
    fn set_pixel(&self, row: u16, col: u16, r: u8, g: u8, b: u8) {
        let f: &Cell<[u8]> = &self.video_mem;
        let idx = (row as usize * 256 + col as usize) * 3;
        let s = f.as_slice_of_cells();
        s[idx].set(r);
        s[idx + 1].set(g);
        s[idx + 2].set(b);
    }

    fn poll_buttons(&self) -> StandardControllerButtons {
        StandardControllerButtons::empty()
    }
}