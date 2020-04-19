use std::cell::Cell;
bitflags! {
    pub struct StandardControllerButtons: u8 {
        const A = 0x01;
        const B = 0x02;
        const SELECT = 0x04;
        const START = 0x08;
        const UP = 0x10;
        const DOWN = 0x20;
        const LEFT = 0x40;
        const RIGHT = 0x80;
    }
}

bitflags! {
    pub struct ControllerPortDataLines: u8 {
        const D0 = 0x01;
        const D3 = 0x08;
        const D4 = 0x10;
    }
}

pub trait IO {
    fn set_pixel(&self, row: u16, col: u16, r: u8, g: u8, b: u8);
    // Represents a transition in the latch line from the 2A03
    // Only called on CHANGE, not every 4016 write
    fn controller_latch_change(&self, value: bool);
    fn controller_port_1_read(&self) -> ControllerPortDataLines;
    fn controller_port_2_read(&self) -> ControllerPortDataLines;
}

pub struct DummyIO;
impl IO for DummyIO {
    fn set_pixel(&self, _row: u16, _col: u16, _r: u8, _g: u8, _b: u8) {}

    fn controller_latch_change(&self, _value: bool) {}

    fn controller_port_1_read(&self) -> ControllerPortDataLines {
        ControllerPortDataLines::empty()
    }

    fn controller_port_2_read(&self) -> ControllerPortDataLines {
        ControllerPortDataLines::empty()
    }
}

// The only one I want to emulate for now - deals with the latching/shift reg logic
pub trait SingleStandardControllerIO {
    fn set_pixel(&self, row: u16, col: u16, r: u8, g: u8, b: u8);
    fn poll_buttons(&self) -> StandardControllerButtons;
}

pub struct SingleStandardController<I: SingleStandardControllerIO> {
    pub io: I,
    currently_high: Cell<bool>,
    latch: Cell<u8>,
}

impl<I: SingleStandardControllerIO> SingleStandardController<I> {
    pub fn new(io: I) -> SingleStandardController<I> {
        SingleStandardController {
            io,
            currently_high: Cell::new(false),
            latch: Cell::new(0),
        }
    }
}

impl<I: SingleStandardControllerIO> IO for SingleStandardController<I> {
    fn set_pixel(&self, row: u16, col: u16, r: u8, g: u8, b: u8) {
        self.io.set_pixel(row, col, r, g, b);
    }

    fn controller_latch_change(&self, value: bool) {
        self.currently_high.set(value);
        if !value {
            // High-low transition ==> Latch current buttons
            let mut buttons = self.io.poll_buttons();

            // Remove impossible combinations
            if buttons.contains(StandardControllerButtons::UP | StandardControllerButtons::DOWN) {
                buttons.remove(StandardControllerButtons::DOWN);
            }
            if buttons.contains(StandardControllerButtons::LEFT | StandardControllerButtons::RIGHT) {
                buttons.remove(StandardControllerButtons::RIGHT);
            }

            self.latch.set(buttons.bits());
        }
    }

    fn controller_port_1_read(&self) -> ControllerPortDataLines {
        let bit = if self.currently_high.get() {
            // We return the current A value - no need to check for impossible combinations with A
            self.io.poll_buttons().contains(StandardControllerButtons::A)
        } else {
            let latch = self.latch.get();
            self.latch.set((latch >> 1) | 0x80);  // Official NES controllers return 1 after emptying latch
            latch & 1 == 1
        };

        // Note technically D4 and D5 are open bus in this configuration, not 0. I'm not emulating
        // open bus currently - when (and if) I get around to doing open bus I'll have to do that
        // somehow - pass in the open bus value to this function?

        if bit {
            ControllerPortDataLines::D0
        } else {
            ControllerPortDataLines::empty()
        }
    }

    fn controller_port_2_read(&self) -> ControllerPortDataLines {
        // Not connected - this is always 0
        ControllerPortDataLines::empty()
    }
}