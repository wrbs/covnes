use std::{
    cell::Cell,
    mem::swap,
    sync::mpsc::{channel, Receiver, Sender},
    thread::spawn,
};

use covnes::nes::{
    io::{SingleStandardController, SingleStandardControllerIO, StandardControllerButtons},
    mappers::Cartridge,
    Nes,
};

#[derive(Debug)]
struct PixelData {
    pixels: Box<Cell<[(u8, u8, u8); 256 * 240]>>,
}

impl PixelData {
    fn new() -> Self {
        Self {
            pixels: Box::new(Cell::new([(0, 0, 0); 256 * 240])),
        }
    }

    fn pixels(&self) -> &[Cell<(u8, u8, u8)>] {
        let f: &Cell<[(u8, u8, u8)]> = self.pixels.as_ref();
        f.as_slice_of_cells()
    }

    fn set_pixel(&self, row: u16, col: u16, r: u8, g: u8, b: u8) {
        self.pixels()[row as usize * 256 + col as usize].set((r, g, b));
    }
}

// The two threads communicate by passing (boxes of) buffers to write in to between themselves

pub struct Emulator {
    tx: Sender<Message>,
    rx: Receiver<PixelData>,
    // This is always Some, apart from during a step_frame call
    // this is because we only swap two buffers between the threads -- at some
    // point in time, one of the buffers will be in transit in the channel
    buffer: Option<PixelData>,
}

impl Emulator {
    pub fn new(cartridge: Cartridge) -> Self {
        let (msg_tx, msg_rx) = channel();
        let (buffer_tx, buffer_rx) = channel();
        spawn(move || run_emulator(msg_rx, buffer_tx, cartridge));
        Self {
            tx: msg_tx,
            rx: buffer_rx,
            buffer: Some(PixelData::new()),
        }
    }

    pub fn step_frame(&mut self) {
        let mut buffer = None;
        swap(&mut buffer, &mut self.buffer);
        self.tx.send(Message::NewFrame(buffer.unwrap())).unwrap();
        self.buffer = Some(self.rx.recv().unwrap());
    }

    pub fn reset(&mut self) {
        self.tx.send(Message::Reset).unwrap();
    }

    pub fn set_buttons(&mut self, buttons: StandardControllerButtons) {
        self.tx.send(Message::SetInput(buttons)).unwrap()
    }

    pub fn iter_pixels<F>(&mut self, mut f: F)
    where
        F: FnMut(u8, u8, (u8, u8, u8)),
    {
        let mut iter = self.buffer.as_ref().unwrap().pixels().iter().cloned();

        for row in 0..240 {
            for col in 0..=255 {
                f(row, col, iter.next().unwrap().get())
            }
        }
    }
}

#[derive(Debug)]
enum Message {
    SetInput(StandardControllerButtons),
    NewFrame(PixelData),
    Reset,
}

fn run_emulator(rx: Receiver<Message>, tx: Sender<PixelData>, cartridge: Cartridge) {
    let io = SingleStandardController::new(EmulatorIo::new());
    let mut nes = Nes::new(io);
    nes.insert_cartridge(cartridge);

    for message in rx.iter() {
        match message {
            Message::SetInput(input) => {
                nes.io.io.current_key_state.set(input);
            }
            Message::NewFrame(mut buffer) => {
                swap(&mut buffer, &mut nes.io.io.pixels);
                tx.send(buffer).unwrap();
                nes.step_frame();
            }
            Message::Reset => nes.reset(),
        }
    }
}

struct EmulatorIo {
    pixels: PixelData,
    current_key_state: Cell<StandardControllerButtons>,
}

impl EmulatorIo {
    fn new() -> Self {
        Self {
            pixels: PixelData::new(),
            current_key_state: Cell::new(StandardControllerButtons::empty()),
        }
    }
}

impl SingleStandardControllerIO for EmulatorIo {
    fn set_pixel(&self, row: u16, col: u16, r: u8, g: u8, b: u8) {
        self.pixels.set_pixel(row, col, r, g, b);
    }

    fn poll_buttons(&self) -> StandardControllerButtons {
        self.current_key_state.get()
    }
}
