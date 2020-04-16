use std::cell::Cell;

bitflags! {
    pub struct Flags: u8 {
        const N = 0b1000_0000;
        const V = 0b0100_0000;
        const D = 0b0000_1000;
        const I = 0b0000_0100;
        const Z = 0b0000_0010;
        const C = 0b0000_0001;
    }
}

pub struct CPU {
    pub pc: Cell<u16>,
    pub s: Cell<u8>,
    pub flags: Cell<Flags>,
    pub a: Cell<u8>,
    pub x: Cell<u8>,
    pub y: Cell<u8>,
    pub state: Cell<State>,
    pub nmi: Cell<Option<bool>>,
    pub irq: Cell<Option<bool>>,
}

impl CPU {
    pub fn new() -> CPU {
        // We need to jump to the reset vector - we can trick this by setting the pc to $FFFD and
        // pretending we're executing an absolute jump instruction
        CPU {
            pc: Cell::new(0x0000),
            s: Cell::new(0x00),
            flags: Cell::new(Flags::empty()),
            a: Cell::new(0),
            x: Cell::new(0),
            y: Cell::new(0),
            state: Cell::new(State(S::Reset)),
            nmi: Cell::new(None),
            irq: Cell::new(None),
        }
    }

    pub fn reset(&self) {
        self.state.set(State(S::Reset));
    }

    pub fn set_nmi(&self) {
        self.nmi.set(Some(false));
    }

    // Use to initialise test roms with automated modes
    pub fn jump_to_pc(&self, addr: u16) {
        self.pc.set(addr);
        self.state.set(State(S::FetchOpcode));
    }

    pub fn get_p(&self) -> u8 {
        self.flags.get().bits()
    }

    pub fn set_p(&self, value: u8) {
        self.flags.set(Flags::from_bits_truncate(value))
    }

    pub fn get_flag(&self, flag: Flags) -> bool {
        self.flags.get().contains(flag)
    }

    pub fn set_flag(&self, flag: Flags, value: bool) {
        let mut f = self.flags.get();
        if value {
            f.insert(flag);
        } else {
            f.remove(flag);
        }
        self.flags.set(f);
    }

    pub fn set_zn(&self, val: u8) {
        self.set_flag(Flags::Z, val == 0);
        self.set_flag(Flags::N, val >> 7 == 1)
    }
}

pub trait CpuHostAccess {
    fn read(&self, addr: u16) -> u8;
    fn write(&self, addr: u16, value: u8);
}

// We want a cycle-accurate generator so we're making a state machine
// There are a few approaches to this - I want to do a particularly rusty one and I want to minimise
// the states required to try to somewhat maximise the readability

// This means abstracting away the concept of an addressing mode (in the states)
// and an operation

// The common operations fall into these categories
// Anything implied mode is done in the decode phase
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
enum ReadOp {
    ADC,
    AND,
    BIT,
    CMP,
    CPX,
    CPY,
    EOR,
    LDA,
    LDX,
    LDY,
    ORA,
    SBC,
    // Undocumented
    NOP,
    LAX,
    // Not really read-ops
    ANC,
    ALR,
    ARR,
    AXS
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
enum ImpliedOp {
    CLC,
    CLD,
    CLI,
    CLV,
    DEX,
    DEY,
    INX,
    INY,
    NOP,
    SEC,
    SED,
    SEI,
    TAX,
    TAY,
    TSX,
    TXA,
    TXS,
    TYA,
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
enum WriteOp {
    STA,
    STX,
    STY,
    SAX,
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
enum ReadWriteOp {
    ASL,
    DEC,
    INC,
    LSR,
    ROL,
    ROR,
    DCP,
    ISC,
    SLO,
    RLA,
    SRE,
    RRA,
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
enum BranchOp {
    BCC,
    BCS,
    BEQ,
    BMI,
    BNE,
    BPL,
    BVC,
    BVS,
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
enum SHOp {
    SHY,
    SHX
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
enum Op {
    Read(ReadOp),
    ReadWrite(ReadWriteOp),
    Write(WriteOp),
    SH(SHOp),
}

impl Op {
    fn is_read(&self) -> bool {
        if let Op::Read(_) = self {
            true
        } else {
            false
        }
    }
}

impl From<ReadOp> for Op {
    fn from(o: ReadOp) -> Self {
        Op::Read(o)
    }
}

impl From<ReadWriteOp> for Op {
    fn from(o: ReadWriteOp) -> Self {
        Op::ReadWrite(o)
    }
}

impl From<WriteOp> for Op {
    fn from(o: WriteOp) -> Self {
        Op::Write(o)
    }
}

impl From<SHOp> for Op {
    fn from(o: SHOp) -> Self {
        Op::SH(o)
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
enum Interrupt {
    BRK,
    NMI,
    IRQ,
    Reset,
}

#[derive(Debug, Eq, PartialEq, Copy, Clone, Default)]
pub struct State(S);

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
enum S {
    // Core
    FetchOpcode,
    // Generic R/RMW/W
    ZeroPage(Op),
    ZeroPageX(Op),
    ZeroPageY(Op),
    AbsoluteX(Op),
    AbsoluteY(Op),
    Absolute(Op),
    IndexedIndirect(Op),
    IndexedIndirect2(Op, u8),
    IndexedIndirect3(Op, u8),
    IndirectIndexed(Op),
    IndirectIndexed2(Op, u8),
    // Utils for generic
    AddLowHigh(Op, u16, u16),
    AddLowHighNoPen(Op, u16, u8),
    FakeThenActual(Op, u16, u16),
    ExecuteOnAddress(Op, u16, u8),
    WriteBackThenWrite(u16, u8, u8),
    Write(u16, u8),
    // Read only
    ImmediateR(ReadOp),
    // ReadWrite only
    AccRW(ReadWriteOp),
    // Relative (branches)
    Relative(BranchOp),
    Relative2(u8),
    Relative3(u16),
    // Normal implied
    Implied(ImpliedOp),
    // Odd timings
    Reset,
    Int(Interrupt),
    Int2(Interrupt),
    Int3(Interrupt),
    Int4(Interrupt, bool),
    Int5(Interrupt),
    Int6(Interrupt, u8),
    RTI,
    RTI2,
    RTI3,
    RTI4,
    RTI5(u8),
    RTS,
    RTS2,
    RTS3,
    RTS4(u8),
    RTS5,
    PHA,
    PHP,
    PHPA(u8),
    PLA,
    PLA2,
    PLA3,
    PLP,
    PLP2,
    PLP3,
    JSR,
    JSR2(u8),
    JSR3(u8),
    JSR4(u8),
    JSR5(u8),
    JMPAbsolute,
    JMPAbsolute2(u8),
    JMPIndirect,
    JMPIndirect2(u8),
    JMPIndirect3(u16),
    JMPIndirect4(u16, u8),
}

impl Default for S {
    fn default() -> Self {
        S::FetchOpcode
    }
}

impl CPU {
    pub fn is_at_instruction(&self) -> bool {
        self.state.get().0 == S::FetchOpcode
    }

    pub fn tick<H: CpuHostAccess>(&self, host: &H) {
        let next_state = match self.state.get().0 {
            S::FetchOpcode => {
                if self.irq.get() != None && self.get_flag(Flags::I) {
                    self.irq.set(None);
                }

                if self.nmi.get() == Some(true) {
                    self.nmi.set(None);
                    S::Int(Interrupt::NMI)
                } else if self.irq.get() == Some(true) {
                    self.nmi.set(None);
                    S::Int(Interrupt::IRQ)
                } else {
                    let opcode = {
                        let pc = self.pc.get();
                        let opcode = host.read(pc);
                        self.pc.set(pc.wrapping_add(1));
                        opcode
                    };
                    match opcode {
                        // ADC
                        0x69 => S::ImmediateR(ReadOp::ADC),
                        0x65 => S::ZeroPage(ReadOp::ADC.into()),
                        0x75 => S::ZeroPageX(ReadOp::ADC.into()),
                        0x6D => S::Absolute(ReadOp::ADC.into()),
                        0x7D => S::AbsoluteX(ReadOp::ADC.into()),
                        0x79 => S::AbsoluteY(ReadOp::ADC.into()),
                        0x61 => S::IndexedIndirect(ReadOp::ADC.into()),
                        0x71 => S::IndirectIndexed(ReadOp::ADC.into()),
                        // AND
                        0x29 => S::ImmediateR(ReadOp::AND),
                        0x25 => S::ZeroPage(ReadOp::AND.into()),
                        0x35 => S::ZeroPageX(ReadOp::AND.into()),
                        0x2D => S::Absolute(ReadOp::AND.into()),
                        0x3D => S::AbsoluteX(ReadOp::AND.into()),
                        0x39 => S::AbsoluteY(ReadOp::AND.into()),
                        0x21 => S::IndexedIndirect(ReadOp::AND.into()),
                        0x31 => S::IndirectIndexed(ReadOp::AND.into()),
                        // ASL
                        0x0A => S::AccRW(ReadWriteOp::ASL),
                        0x06 => S::ZeroPage(ReadWriteOp::ASL.into()),
                        0x16 => S::ZeroPageX(ReadWriteOp::ASL.into()),
                        0x0E => S::Absolute(ReadWriteOp::ASL.into()),
                        0x1E => S::AbsoluteX(ReadWriteOp::ASL.into()),
                        // BCC
                        0x90 => S::Relative(BranchOp::BCC),
                        // BCS
                        0xB0 => S::Relative(BranchOp::BCS),
                        // BEQ
                        0xF0 => S::Relative(BranchOp::BEQ),
                        // BIT
                        0x24 => S::ZeroPage(ReadOp::BIT.into()),
                        0x2C => S::Absolute(ReadOp::BIT.into()),
                        // BMI
                        0x30 => S::Relative(BranchOp::BMI),
                        // BNE
                        0xD0 => S::Relative(BranchOp::BNE),
                        // BPL
                        0x10 => S::Relative(BranchOp::BPL),
                        // BRK,
                        0x00 => S::Int(Interrupt::BRK),
                        // BVC
                        0x50 => S::Relative(BranchOp::BVC),
                        // BVS
                        0x70 => S::Relative(BranchOp::BVS),
                        // CLC
                        0x18 => S::Implied(ImpliedOp::CLC),
                        // CLD
                        0xD8 => S::Implied(ImpliedOp::CLD),
                        // CLI
                        0x58 => S::Implied(ImpliedOp::CLI),
                        // CLV
                        0xB8 => S::Implied(ImpliedOp::CLV),
                        // CMP
                        0xC9 => S::ImmediateR(ReadOp::CMP),
                        0xC5 => S::ZeroPage(ReadOp::CMP.into()),
                        0xD5 => S::ZeroPageX(ReadOp::CMP.into()),
                        0xCD => S::Absolute(ReadOp::CMP.into()),
                        0xDD => S::AbsoluteX(ReadOp::CMP.into()),
                        0xD9 => S::AbsoluteY(ReadOp::CMP.into()),
                        0xC1 => S::IndexedIndirect(ReadOp::CMP.into()),
                        0xD1 => S::IndirectIndexed(ReadOp::CMP.into()),
                        // CPX
                        0xE0 => S::ImmediateR(ReadOp::CPX),
                        0xE4 => S::ZeroPage(ReadOp::CPX.into()),
                        0xEC => S::Absolute(ReadOp::CPX.into()),
                        // CPY
                        0xC0 => S::ImmediateR(ReadOp::CPY),
                        0xC4 => S::ZeroPage(ReadOp::CPY.into()),
                        0xCC => S::Absolute(ReadOp::CPY.into()),
                        // DEC
                        0xC6 => S::ZeroPage(ReadWriteOp::DEC.into()),
                        0xD6 => S::ZeroPageX(ReadWriteOp::DEC.into()),
                        0xCE => S::Absolute(ReadWriteOp::DEC.into()),
                        0xDE => S::AbsoluteX(ReadWriteOp::DEC.into()),
                        // DEX
                        0xCA => S::Implied(ImpliedOp::DEX),
                        // DEY
                        0x88 => S::Implied(ImpliedOp::DEY),
                        // EOR
                        0x49 => S::ImmediateR(ReadOp::EOR),
                        0x45 => S::ZeroPage(ReadOp::EOR.into()),
                        0x55 => S::ZeroPageX(ReadOp::EOR.into()),
                        0x4D => S::Absolute(ReadOp::EOR.into()),
                        0x5D => S::AbsoluteX(ReadOp::EOR.into()),
                        0x59 => S::AbsoluteY(ReadOp::EOR.into()),
                        0x41 => S::IndexedIndirect(ReadOp::EOR.into()),
                        0x51 => S::IndirectIndexed(ReadOp::EOR.into()),
                        // INC
                        0xE6 => S::ZeroPage(ReadWriteOp::INC.into()),
                        0xF6 => S::ZeroPageX(ReadWriteOp::INC.into()),
                        0xEE => S::Absolute(ReadWriteOp::INC.into()),
                        0xFE => S::AbsoluteX(ReadWriteOp::INC.into()),
                        // INX
                        0xE8 => S::Implied(ImpliedOp::INX),
                        // INY
                        0xC8 => S::Implied(ImpliedOp::INY),
                        // JMP
                        0x4C => S::JMPAbsolute,
                        0x6C => S::JMPIndirect,
                        // JSR
                        0x20 => S::JSR,
                        // LDA
                        0xA9 => S::ImmediateR(ReadOp::LDA),
                        0xA5 => S::ZeroPage(ReadOp::LDA.into()),
                        0xB5 => S::ZeroPageX(ReadOp::LDA.into()),
                        0xAD => S::Absolute(ReadOp::LDA.into()),
                        0xBD => S::AbsoluteX(ReadOp::LDA.into()),
                        0xB9 => S::AbsoluteY(ReadOp::LDA.into()),
                        0xA1 => S::IndexedIndirect(ReadOp::LDA.into()),
                        0xB1 => S::IndirectIndexed(ReadOp::LDA.into()),
                        // LDX
                        0xA2 => S::ImmediateR(ReadOp::LDX),
                        0xA6 => S::ZeroPage(ReadOp::LDX.into()),
                        0xB6 => S::ZeroPageY(ReadOp::LDX.into()),
                        0xAE => S::Absolute(ReadOp::LDX.into()),
                        0xBE => S::AbsoluteY(ReadOp::LDX.into()),
                        // LDY
                        0xA0 => S::ImmediateR(ReadOp::LDY),
                        0xA4 => S::ZeroPage(ReadOp::LDY.into()),
                        0xB4 => S::ZeroPageX(ReadOp::LDY.into()),
                        0xAC => S::Absolute(ReadOp::LDY.into()),
                        0xBC => S::AbsoluteX(ReadOp::LDY.into()),
                        // LSR
                        0x4A => S::AccRW(ReadWriteOp::LSR),
                        0x46 => S::ZeroPage(ReadWriteOp::LSR.into()),
                        0x56 => S::ZeroPageX(ReadWriteOp::LSR.into()),
                        0x4E => S::Absolute(ReadWriteOp::LSR.into()),
                        0x5E => S::AbsoluteX(ReadWriteOp::LSR.into()),
                        // NOP
                        0xEA => S::Implied(ImpliedOp::NOP),
                        // ORA
                        0x09 => S::ImmediateR(ReadOp::ORA),
                        0x05 => S::ZeroPage(ReadOp::ORA.into()),
                        0x15 => S::ZeroPageX(ReadOp::ORA.into()),
                        0x0D => S::Absolute(ReadOp::ORA.into()),
                        0x1D => S::AbsoluteX(ReadOp::ORA.into()),
                        0x19 => S::AbsoluteY(ReadOp::ORA.into()),
                        0x01 => S::IndexedIndirect(ReadOp::ORA.into()),
                        0x11 => S::IndirectIndexed(ReadOp::ORA.into()),
                        // PHA
                        0x48 => S::PHA,
                        // PHP
                        0x08 => S::PHP,
                        // PLA
                        0x68 => S::PLA,
                        // PLP
                        0x28 => S::PLP,
                        // ROL
                        0x2A => S::AccRW(ReadWriteOp::ROL),
                        0x26 => S::ZeroPage(ReadWriteOp::ROL.into()),
                        0x36 => S::ZeroPageX(ReadWriteOp::ROL.into()),
                        0x2E => S::Absolute(ReadWriteOp::ROL.into()),
                        0x3E => S::AbsoluteX(ReadWriteOp::ROL.into()),
                        // ROR
                        0x6A => S::AccRW(ReadWriteOp::ROR),
                        0x66 => S::ZeroPage(ReadWriteOp::ROR.into()),
                        0x76 => S::ZeroPageX(ReadWriteOp::ROR.into()),
                        0x6E => S::Absolute(ReadWriteOp::ROR.into()),
                        0x7E => S::AbsoluteX(ReadWriteOp::ROR.into()),
                        // RTI
                        0x40 => S::RTI,
                        // RTS
                        0x60 => S::RTS,
                        // SBC
                        0xE9 => S::ImmediateR(ReadOp::SBC),
                        0xE5 => S::ZeroPage(ReadOp::SBC.into()),
                        0xF5 => S::ZeroPageX(ReadOp::SBC.into()),
                        0xED => S::Absolute(ReadOp::SBC.into()),
                        0xFD => S::AbsoluteX(ReadOp::SBC.into()),
                        0xF9 => S::AbsoluteY(ReadOp::SBC.into()),
                        0xE1 => S::IndexedIndirect(ReadOp::SBC.into()),
                        0xF1 => S::IndirectIndexed(ReadOp::SBC.into()),
                        // SEC
                        0x38 => S::Implied(ImpliedOp::SEC),
                        // SED
                        0xF8 => S::Implied(ImpliedOp::SED),
                        // SEI
                        0x78 => S::Implied(ImpliedOp::SEI),
                        // STA
                        0x85 => S::ZeroPage(WriteOp::STA.into()),
                        0x95 => S::ZeroPageX(WriteOp::STA.into()),
                        0x8D => S::Absolute(WriteOp::STA.into()),
                        0x9D => S::AbsoluteX(WriteOp::STA.into()),
                        0x99 => S::AbsoluteY(WriteOp::STA.into()),
                        0x81 => S::IndexedIndirect(WriteOp::STA.into()),
                        0x91 => S::IndirectIndexed(WriteOp::STA.into()),
                        // STX
                        0x86 => S::ZeroPage(WriteOp::STX.into()),
                        0x96 => S::ZeroPageY(WriteOp::STX.into()),
                        0x8E => S::Absolute(WriteOp::STX.into()),
                        // STY
                        0x84 => S::ZeroPage(WriteOp::STY.into()),
                        0x94 => S::ZeroPageX(WriteOp::STY.into()),
                        0x8C => S::Absolute(WriteOp::STY.into()),
                        // TAX
                        0xAA => S::Implied(ImpliedOp::TAX),
                        // TAY
                        0xA8 => S::Implied(ImpliedOp::TAY),
                        // TSX
                        0xBA => S::Implied(ImpliedOp::TSX),
                        // TXA
                        0x8A => S::Implied(ImpliedOp::TXA),
                        // TXS
                        0x9A => S::Implied(ImpliedOp::TXS),
                        // TYA
                        0x98 => S::Implied(ImpliedOp::TYA),

                        // Undocumented opcodes
                        // Various NOPs
                        0x04 | 0x44 | 0x64 => S::ZeroPage(ReadOp::NOP.into()),
                        0x0C => S::Absolute(ReadOp::NOP.into()),
                        0x14 | 0x34 | 0x54 | 0x74 | 0xD4 | 0xF4 => S::ZeroPageX(ReadOp::NOP.into()),
                        0x1A | 0x3A | 0x5A | 0x7A | 0xDA | 0xFA => S::Implied(ImpliedOp::NOP),
                        0x1C | 0x3C | 0x5C | 0x7C | 0xDC | 0xFC => S::AbsoluteX(ReadOp::NOP.into()),
                        0x80 | 0x82 | 0x89 | 0xC2 | 0xE2 => S::ImmediateR(ReadOp::NOP),
                        // LAX
                        0xA3 => S::IndexedIndirect(ReadOp::LAX.into()),
                        0xA7 => S::ZeroPage(ReadOp::LAX.into()),
                        0xAB => S::ImmediateR(ReadOp::LAX),
                        0xAF => S::Absolute(ReadOp::LAX.into()),
                        0xB3 => S::IndirectIndexed(ReadOp::LAX.into()),
                        0xB7 => S::ZeroPageY(ReadOp::LAX.into()),
                        0xBF => S::AbsoluteY(ReadOp::LAX.into()),
                        // SAX
                        0x83 => S::IndexedIndirect(WriteOp::SAX.into()),
                        0x87 => S::ZeroPage(WriteOp::SAX.into()),
                        0x8F => S::Absolute(WriteOp::SAX.into()),
                        0x97 => S::ZeroPageY(WriteOp::SAX.into()),
                        // SBC
                        0xEB => S::ImmediateR(ReadOp::SBC),
                        // DCP
                        0xC3 => S::IndexedIndirect(ReadWriteOp::DCP.into()),
                        0xC7 => S::ZeroPage(ReadWriteOp::DCP.into()),
                        0xCF => S::Absolute(ReadWriteOp::DCP.into()),
                        0xD3 => S::IndirectIndexed(ReadWriteOp::DCP.into()),
                        0xD7 => S::ZeroPageX(ReadWriteOp::DCP.into()),
                        0xDB => S::AbsoluteY(ReadWriteOp::DCP.into()),
                        0xDF => S::AbsoluteX(ReadWriteOp::DCP.into()),
                        // ISC
                        0xE3 => S::IndexedIndirect(ReadWriteOp::ISC.into()),
                        0xE7 => S::ZeroPage(ReadWriteOp::ISC.into()),
                        0xEF => S::Absolute(ReadWriteOp::ISC.into()),
                        0xF3 => S::IndirectIndexed(ReadWriteOp::ISC.into()),
                        0xF7 => S::ZeroPageX(ReadWriteOp::ISC.into()),
                        0xFB => S::AbsoluteY(ReadWriteOp::ISC.into()),
                        0xFF => S::AbsoluteX(ReadWriteOp::ISC.into()),
                        // SLO
                        0x03 => S::IndexedIndirect(ReadWriteOp::SLO.into()),
                        0x07 => S::ZeroPage(ReadWriteOp::SLO.into()),
                        0x0F => S::Absolute(ReadWriteOp::SLO.into()),
                        0x13 => S::IndirectIndexed(ReadWriteOp::SLO.into()),
                        0x17 => S::ZeroPageX(ReadWriteOp::SLO.into()),
                        0x1B => S::AbsoluteY(ReadWriteOp::SLO.into()),
                        0x1F => S::AbsoluteX(ReadWriteOp::SLO.into()),
                        // RLA
                        0x23 => S::IndexedIndirect(ReadWriteOp::RLA.into()),
                        0x27 => S::ZeroPage(ReadWriteOp::RLA.into()),
                        0x2F => S::Absolute(ReadWriteOp::RLA.into()),
                        0x33 => S::IndirectIndexed(ReadWriteOp::RLA.into()),
                        0x37 => S::ZeroPageX(ReadWriteOp::RLA.into()),
                        0x3B => S::AbsoluteY(ReadWriteOp::RLA.into()),
                        0x3F => S::AbsoluteX(ReadWriteOp::RLA.into()),
                        // SRE
                        0x43 => S::IndexedIndirect(ReadWriteOp::SRE.into()),
                        0x47 => S::ZeroPage(ReadWriteOp::SRE.into()),
                        0x4F => S::Absolute(ReadWriteOp::SRE.into()),
                        0x53 => S::IndirectIndexed(ReadWriteOp::SRE.into()),
                        0x57 => S::ZeroPageX(ReadWriteOp::SRE.into()),
                        0x5B => S::AbsoluteY(ReadWriteOp::SRE.into()),
                        0x5F => S::AbsoluteX(ReadWriteOp::SRE.into()),
                        // RRA
                        0x63 => S::IndexedIndirect(ReadWriteOp::RRA.into()),
                        0x67 => S::ZeroPage(ReadWriteOp::RRA.into()),
                        0x6F => S::Absolute(ReadWriteOp::RRA.into()),
                        0x73 => S::IndirectIndexed(ReadWriteOp::RRA.into()),
                        0x77 => S::ZeroPageX(ReadWriteOp::RRA.into()),
                        0x7B => S::AbsoluteY(ReadWriteOp::RRA.into()),
                        0x7F => S::AbsoluteX(ReadWriteOp::RRA.into()),
                        // ANC
                        0x0B => S::ImmediateR(ReadOp::ANC),
                        0x2B => S::ImmediateR(ReadOp::ANC),
                        // ALR
                        0x4B => S::ImmediateR(ReadOp::ALR),
                        // ARR
                        0x6B => S::ImmediateR(ReadOp::ARR),
                        // AXS
                        0xCB => S::ImmediateR(ReadOp::AXS),
                        // SHY
                        0x9C => S::AbsoluteX(SHOp::SHY.into()),
                        // SHX
                        0x9E => S::AbsoluteY(SHOp::SHX.into()),

                        x => panic!("Illegal opcode: {:X}", x),
                    }
                }
            }
            S::ImmediateR(oc) => {
                let pc = self.pc.get();
                let operand = host.read(pc);
                self.pc.set(pc.wrapping_add(1));

                oc.execute(self, operand);

                S::FetchOpcode
            }
            S::ZeroPage(oc) => {
                let pc = self.pc.get();
                let addr = host.read(pc);
                self.pc.set(pc.wrapping_add(1));

                S::ExecuteOnAddress(oc, addr as u16, 0)
            }
            S::ZeroPageX(oc) => {
                let pc = self.pc.get();
                let base = host.read(pc);
                self.pc.set(pc.wrapping_add(1));

                let addr = base.wrapping_add(self.x.get()) as u16;

                S::FakeThenActual(oc, base as u16, addr)
            }
            S::ZeroPageY(oc) => {
                let pc = self.pc.get();
                let base = host.read(pc);
                self.pc.set(pc.wrapping_add(1));

                let addr = base.wrapping_add(self.y.get()) as u16;

                S::FakeThenActual(oc, base as u16, addr)
            }
            S::FakeThenActual(oc, base, addr) => {
                host.read(base);
                S::ExecuteOnAddress(oc, addr, (base >> 8) as u8)
            }
            S::ExecuteOnAddress(oc, addr, orig_high) => match oc {
                Op::Read(ro) => {
                    let val = host.read(addr);
                    ro.execute(self, val);
                    S::FetchOpcode
                }
                Op::ReadWrite(rw) => {
                    let val = host.read(addr);
                    let next_val = rw.execute(self, val);

                    S::WriteBackThenWrite(addr, val, next_val)
                }
                Op::Write(wo) => {
                    let val = wo.execute(self);
                    host.write(addr, val);
                    S::FetchOpcode
                }
                Op::SH(sh) => {
                    let val = sh.execute(self, orig_high);
                    host.write(addr, val);
                    S::FetchOpcode
                }
            },
            S::WriteBackThenWrite(addr, val, next_val) => {
                host.write(addr as u16, val);

                S::Write(addr, next_val)
            }
            S::Write(addr, val) => {
                host.write(addr, val);
                S::FetchOpcode
            }
            S::Absolute(oc) => {
                let pc = self.pc.get();
                let low_word = host.read(pc);
                self.pc.set(pc.wrapping_add(2));

                S::AddLowHighNoPen(oc, pc.wrapping_add(1), low_word)
            }
            S::AbsoluteX(oc) => {
                let pc = self.pc.get();
                let base = host.read(pc);
                self.pc.set(pc.wrapping_add(2));

                let low_word = base as u16 + self.x.get() as u16;
                S::AddLowHigh(oc, pc.wrapping_add(1), low_word)
            }
            S::AbsoluteY(oc) => {
                let pc = self.pc.get();
                let base = host.read(pc);
                self.pc.set(pc.wrapping_add(2));

                let low_word = base as u16 + self.y.get() as u16;
                S::AddLowHigh(oc, pc.wrapping_add(1), low_word)
            }
            S::AddLowHighNoPen(oc, addr, low_word) => {
                let high_word = host.read(addr);

                let addr = (high_word as u16) << 8 | low_word as u16;

                S::ExecuteOnAddress(oc.into(), addr, high_word)
            }
            S::AddLowHigh(oc, addr, low_word) => {
                let high_word = host.read(addr);

                let addr = (high_word as u16) << 8 | (low_word & 0xFF);
                if let Op::SH(sh) = oc {
                    let real_addr = if low_word > 0xFF {
                        (sh.execute(self, high_word) as u16) << 8 | (low_word & 0xFF)
                    } else {
                        (high_word as u16) << 8 | (low_word & 0xFF)
                    };
                    S::FakeThenActual(oc, addr, real_addr)
                } else if low_word > 0xFF {
                    S::FakeThenActual(oc, addr, addr.wrapping_add(0x100))
                } else {
                    if oc.is_read() {
                        S::ExecuteOnAddress(oc, addr, high_word)
                    } else {
                        S::FakeThenActual(oc, addr, addr)
                    }
                }
            }
            S::IndexedIndirect(oc) => {
                let pc = self.pc.get();
                let base = host.read(pc);
                self.pc.set(pc.wrapping_add(1));

                S::IndexedIndirect2(oc, base)
            }
            S::IndexedIndirect2(oc, base) => {
                host.read(base as u16);
                S::IndexedIndirect3(oc, base)
            }
            S::IndexedIndirect3(oc, base) => {
                let pointer = base.wrapping_add(self.x.get());
                let low = host.read(pointer as u16);

                S::AddLowHighNoPen(oc.into(), pointer.wrapping_add(1) as u16, low)
            }
            S::IndirectIndexed(oc) => {
                let pc = self.pc.get();
                let base = host.read(pc);
                self.pc.set(pc.wrapping_add(1));

                S::IndirectIndexed2(oc, base)
            }
            S::IndirectIndexed2(oc, base) => {
                let low = host.read(base as u16) as u16 + self.y.get() as u16;

                S::AddLowHigh(oc.into(), base.wrapping_add(1) as u16, low)
            }
            S::AccRW(oc) => {
                host.read(self.pc.get());
                self.a.set(oc.execute(self, self.a.get()));

                S::FetchOpcode
            }
            S::Relative(oc) => {
                let pc = self.pc.get();
                let offset = host.read(pc);
                self.pc.set(pc.wrapping_add(1));

                if oc.execute(self) {
                    S::Relative2(offset)
                } else {
                    S::FetchOpcode
                }
            }
            S::Relative2(offset) => {
                let old_pc = self.pc.get();
                let next_pc = if offset as i8 >= 0 {
                    old_pc.wrapping_add(offset as u16)
                } else {
                    old_pc.wrapping_sub(256 - (offset as u16))
                };

                self.pc.set(next_pc);

                if (old_pc >> 8) != (next_pc >> 8) {
                    S::Relative3(
                        old_pc & 0xFF00 | (((next_pc & 0xFF) as u8).wrapping_add(offset) as u16),
                    )
                } else {
                    S::FetchOpcode
                }
            }
            S::Relative3(addr) => {
                host.read(addr);

                S::FetchOpcode
            }
            S::Implied(oc) => {
                oc.execute(self);

                S::FetchOpcode
            }
            S::Reset => {
                let pc = self.pc.get();
                host.read(pc);
                S::Int(Interrupt::Reset)
            }
            S::Int(interrupt) => {
                let mut pc = self.pc.get();
                if interrupt == Interrupt::BRK {
                    pc = pc.wrapping_add(1);
                    self.pc.set(pc);
                }
                host.read(pc);
                S::Int2(interrupt)
            }
            S::Int2(interrupt) => {
                let pc = self.pc.get();
                let pch = (pc >> 8) as u8;
                let s = self.s.get();

                let addr = 0x100 | s as u16;
                if interrupt == Interrupt::Reset {
                    host.read(addr);
                } else {
                    host.write(addr, pch);
                }
                self.s.set(s.wrapping_sub(1));
                S::Int3(interrupt)
            }
            S::Int3(interrupt) => {
                let pc = self.pc.get();
                let pcl = (pc & 0xff) as u8;
                let s = self.s.get();

                let addr = 0x100 | s as u16;
                if interrupt == Interrupt::Reset {
                    host.read(addr);
                } else {
                    host.write(addr, pcl);
                }
                self.s.set(s.wrapping_sub(1));

                // Hijacking happens here
                match interrupt {
                    Interrupt::BRK if self.nmi.get() != None => S::Int4(Interrupt::NMI, true),
                    Interrupt::BRK if self.irq.get() != None => S::Int4(Interrupt::IRQ, true),
                    Interrupt::BRK => S::Int4(Interrupt::BRK, true),
                    Interrupt::IRQ if self.nmi.get() != None => S::Int4(Interrupt::NMI, false),
                    // what would happen on reset? I think this behaviour is fine for emulation
                    // but I wonder what visual 6502 would do
                    _ => S::Int4(interrupt, false),
                }
            }
            S::Int4(interrupt, b_flag) => {
                let s = self.s.get();

                let mut p = self.get_p();
                p |= 0x20;
                if b_flag {
                    p |= 0x10;
                }
                let addr = 0x100 | s as u16;
                if interrupt == Interrupt::Reset {
                    host.read(addr);
                } else {
                    host.write(addr, p);
                }
                self.s.set(s.wrapping_sub(1));
                S::Int5(interrupt)
            }
            S::Int5(interrupt) => {
                let npcl = host.read(match interrupt {
                    Interrupt::BRK => 0xFFFE,
                    Interrupt::NMI => 0xFFFA,
                    Interrupt::IRQ => 0xFFFE,
                    Interrupt::Reset => 0xFFFC,
                });
                self.set_flag(Flags::I, true);
                S::Int6(interrupt, npcl)
            }
            S::Int6(interrupt, npcl) => {
                let npch = host.read(match interrupt {
                    Interrupt::BRK => 0xFFFF,
                    Interrupt::NMI => 0xFFFB,
                    Interrupt::IRQ => 0xFFFF,
                    Interrupt::Reset => 0xFFFD,
                });

                self.pc.set((npch as u16) << 8 | npcl as u16);

                S::FetchOpcode
            }
            S::RTI => S::RTI2,
            S::RTI2 => {
                let s = self.s.get();

                host.read(0x100 | s as u16);

                self.s.set(s.wrapping_add(1));
                S::RTI3
            }
            S::RTI3 => {
                let s = self.s.get();

                let p = host.read(0x100 | s as u16) & !0x10 | 0x20;
                self.set_p(p);

                self.s.set(s.wrapping_add(1));

                S::RTI4
            }
            S::RTI4 => {
                let s = self.s.get();

                let pcl = host.read(0x100 | s as u16);
                self.s.set(s.wrapping_add(1));

                S::RTI5(pcl)
            }
            S::RTI5(pcl) => {
                let s = self.s.get();

                let pch = host.read(0x100 | s as u16);

                self.pc.set((pch as u16) << 8 | pcl as u16);

                S::FetchOpcode
            }
            S::RTS => S::RTS2,
            S::RTS2 => {
                let s = self.s.get();

                host.read(0x100 | s as u16);

                self.s.set(s.wrapping_add(1));
                S::RTS3
            }
            S::RTS3 => {
                let s = self.s.get();

                let pcl = host.read(0x100 | s as u16);

                self.s.set(s.wrapping_add(1));

                S::RTS4(pcl)
            }
            S::RTS4(pcl) => {
                let s = self.s.get();

                let pch = host.read(0x100 | s as u16);
                let new_pc = (pch as u16) << 8 | pcl as u16;
                self.pc.set(new_pc);

                S::RTS5
            }
            S::RTS5 => {
                let pc = self.pc.get();
                host.read(pc);
                self.pc.set(pc.wrapping_add(1));

                S::FetchOpcode
            }
            S::PHP => S::PHPA(self.get_p() | 0x30),
            S::PHA => S::PHPA(self.a.get()),
            S::PHPA(val) => {
                let s = self.s.get();
                host.write(0x100 | s as u16, val);
                self.s.set(s.wrapping_sub(1));

                S::FetchOpcode
            }
            S::PLP => S::PLP2,
            S::PLP2 => {
                let s = self.s.get();
                host.read(0x100 | s as u16);
                S::PLP3
            }
            S::PLP3 => {
                let s = self.s.get().wrapping_add(1);
                self.s.set(s);
                let p = host.read(0x100 | s as u16) & !0x10 | 0x20;

                self.set_p(p);

                S::FetchOpcode
            }
            S::PLA => S::PLA2,
            S::PLA2 => {
                let s = self.s.get();
                host.read(0x100 | s as u16);
                S::PLA3
            }
            S::PLA3 => {
                let s = self.s.get().wrapping_add(1);
                self.s.set(s);
                let a = host.read(0x100 | s as u16);

                self.set_zn(a);
                self.a.set(a);

                S::FetchOpcode
            }
            S::JSR => {
                let pc = self.pc.get();
                let new_pcl = host.read(pc);
                let pc = pc.wrapping_add(1);
                self.pc.set(pc);
                S::JSR2(new_pcl)
            }
            S::JSR2(new_pcl) => {
                let s = self.s.get();

                host.read(0x100 | s as u16);
                S::JSR3(new_pcl)
            }
            S::JSR3(new_pcl) => {
                let pch = (self.pc.get() >> 8) as u8;
                let s = self.s.get();

                host.write(0x100 | s as u16, pch);
                self.s.set(s.wrapping_sub(1));
                S::JSR4(new_pcl)
            }
            S::JSR4(new_pcl) => {
                let pcl = (self.pc.get() & 0xff) as u8;
                let s = self.s.get();

                host.write(0x100 | s as u16, pcl);
                self.s.set(s.wrapping_sub(1));

                S::JSR5(new_pcl)
            }
            S::JSR5(new_pcl) => {
                let new_pch = host.read(self.pc.get());
                let new_pc = (new_pch as u16) << 8 | new_pcl as u16;

                self.pc.set(new_pc);

                S::FetchOpcode
            }
            S::JMPAbsolute => {
                let pc = self.pc.get();
                let new_pcl = host.read(pc);
                self.pc.set(pc.wrapping_add(1));
                S::JMPAbsolute2(new_pcl)
            }
            S::JMPAbsolute2(new_pcl) => {
                let new_pch = host.read(self.pc.get());
                let new_pc = (new_pch as u16) << 8 | new_pcl as u16;

                self.pc.set(new_pc);

                S::FetchOpcode
            }
            S::JMPIndirect => {
                let pc = self.pc.get();
                let pointer_low = host.read(pc);
                self.pc.set(pc.wrapping_add(1));
                S::JMPIndirect2(pointer_low)
            }
            S::JMPIndirect2(pointer_low) => {
                let pointer_high = host.read(self.pc.get());
                let pointer = (pointer_high as u16) << 8 | pointer_low as u16;

                S::JMPIndirect3(pointer)
            }
            S::JMPIndirect3(pointer) => {
                let pcl = host.read(pointer);
                S::JMPIndirect4(pointer, pcl)
            }
            S::JMPIndirect4(pointer, pcl) => {
                let pointer_low = (pointer & 0xff) as u8;
                let pointer_high = ((pointer & 0xff00) >> 8) as u8;
                let pointer_plus_1 =
                    (pointer_high as u16) << 8 | (pointer_low.wrapping_add(1)) as u16;
                let pch = host.read(pointer_plus_1);

                self.pc.set((pch as u16) << 8 | pcl as u16);

                S::FetchOpcode
            }
        };

        self.state.set(State(next_state));
        match self.nmi.get() {
            Some(false) => self.nmi.set(Some(true)),
            _ => (),
        }

        match self.irq.get() {
            Some(false) => self.irq.set(Some(true)),
            _ => (),
        }

    }
}

impl ReadOp {
    fn execute(&self, cpu: &CPU, operand: u8) {
        match self {
            ReadOp::ADC => {
                let a = cpu.a.get();
                let sum = a as u16 + operand as u16 + if cpu.get_flag(Flags::C) { 1 } else { 0 };
                let result = (sum & 0xff) as u8;

                cpu.set_flag(Flags::C, sum > 0xff);
                cpu.set_flag(Flags::V, (!(a ^ operand) & (a ^ result) & 0x80) == 0x80);

                cpu.a.set(result);
                cpu.set_zn(result);
            }
            ReadOp::AND => {
                let result = cpu.a.get() & operand;
                cpu.a.set(result);
                cpu.set_zn(result);
            }
            ReadOp::BIT => {
                let result = cpu.a.get() & operand;
                cpu.set_zn(result);

                cpu.set_flag(Flags::N, operand >> 7 == 1);
                cpu.set_flag(Flags::V, (operand >> 6) & 1 == 1);
            }
            ReadOp::CMP => {
                let (result, carry) = cpu.a.get().overflowing_sub(operand);
                cpu.set_zn(result);
                cpu.set_flag(Flags::C, !carry);
            }
            ReadOp::CPX => {
                let (result, carry) = cpu.x.get().overflowing_sub(operand);
                cpu.set_zn(result);
                cpu.set_flag(Flags::C, !carry);
            }
            ReadOp::CPY => {
                let (result, carry) = cpu.y.get().overflowing_sub(operand);
                cpu.set_zn(result);
                cpu.set_flag(Flags::C, !carry);
            }
            ReadOp::EOR => {
                let result = cpu.a.get() ^ operand;
                cpu.set_zn(result);
                cpu.a.set(result);
            }
            ReadOp::LDA => {
                cpu.a.set(operand);
                cpu.set_zn(operand);
            }
            ReadOp::LDX => {
                cpu.x.set(operand);
                cpu.set_zn(operand);
            }
            ReadOp::LDY => {
                cpu.y.set(operand);
                cpu.set_zn(operand);
            }
            ReadOp::ORA => {
                let result = cpu.a.get() | operand;
                cpu.set_zn(result);
                cpu.a.set(result);
            }
            ReadOp::SBC => {
                ReadOp::ADC.execute(cpu, !operand);
            }
            ReadOp::NOP => {}
            ReadOp::LAX => {
                cpu.a.set(operand);
                cpu.x.set(operand);
                cpu.set_zn(operand);
            }
            ReadOp::ANC => {
                ReadOp::AND.execute(cpu, operand);
                cpu.set_flag(Flags::C, cpu.get_flag(Flags::N));
            }
            ReadOp::ALR => {
                ReadOp::AND.execute(cpu, operand);
                cpu.a.set(ReadWriteOp::LSR.execute(cpu, cpu.a.get()));
            }
            ReadOp::ARR => {
                ReadOp::AND.execute(cpu, operand);
                let result = ReadWriteOp::ROR.execute(cpu, cpu.a.get());
                cpu.set_flag(Flags::C, result & 0x40 == 0x40);
                cpu.set_flag(Flags::V, ((result >> 6) ^ (result >> 5)) & 1 == 1);
                cpu.a.set(result);
            }
            ReadOp::AXS => {
                let (result, carry) = (cpu.a.get() & cpu.x.get()).overflowing_sub(operand);
                cpu.set_zn(result);
                cpu.set_flag(Flags::C, !carry);
                cpu.x.set(result);
            }
        }
    }
}

impl ReadWriteOp {
    fn execute(&self, cpu: &CPU, operand: u8) -> u8 {
        match self {
            ReadWriteOp::ASL => {
                let result = operand << 1;

                cpu.set_flag(Flags::C, operand >> 7 == 1);
                cpu.set_zn(result);

                result
            }
            ReadWriteOp::DEC => {
                let result = operand.wrapping_sub(1);
                cpu.set_zn(result);

                result
            }
            ReadWriteOp::INC => {
                let result = operand.wrapping_add(1);
                cpu.set_zn(result);
                result
            }
            ReadWriteOp::LSR => {
                let result = operand >> 1;
                cpu.set_flag(Flags::C, operand & 1 == 1);

                cpu.set_zn(result);
                result
            }
            ReadWriteOp::ROL => {
                let result = operand << 1;
                let result = result | if cpu.get_flag(Flags::C) { 1 } else { 0 };

                cpu.set_flag(Flags::C, operand >> 7 == 1);
                cpu.set_zn(result);

                result
            }
            ReadWriteOp::ROR => {
                let carry_bit = if cpu.get_flag(Flags::C) { 1 } else { 0 };
                let result = (operand >> 1) | (carry_bit << 7);

                cpu.set_flag(Flags::C, operand & 1 == 1);
                cpu.set_zn(result);

                result
            }
            ReadWriteOp::DCP => {
                let result = ReadWriteOp::DEC.execute(cpu, operand);
                ReadOp::CMP.execute(cpu, result);

                result
            }
            ReadWriteOp::ISC => {
                let result = ReadWriteOp::INC.execute(cpu, operand);
                ReadOp::SBC.execute(cpu, result);

                result
            }
            ReadWriteOp::SLO => {
                let result = ReadWriteOp::ASL.execute(cpu, operand);
                ReadOp::ORA.execute(cpu, result);

                result
            }
            ReadWriteOp::RLA => {
                let result = ReadWriteOp::ROL.execute(cpu, operand);
                ReadOp::AND.execute(cpu, result);

                result
            }
            ReadWriteOp::SRE => {
                let result = ReadWriteOp::LSR.execute(cpu, operand);
                ReadOp::EOR.execute(cpu, result);

                result
            }
            ReadWriteOp::RRA => {
                let result = ReadWriteOp::ROR.execute(cpu, operand);
                ReadOp::ADC.execute(cpu, result);

                result
            }
        }
    }
}

impl WriteOp {
    fn execute(&self, cpu: &CPU) -> u8 {
        match self {
            WriteOp::STA => cpu.a.get(),
            WriteOp::STX => cpu.x.get(),
            WriteOp::STY => cpu.y.get(),
            WriteOp::SAX => cpu.a.get() & cpu.x.get(),
        }
    }
}

impl BranchOp {
    fn execute(&self, cpu: &CPU) -> bool {
        match self {
            BranchOp::BCC => !cpu.get_flag(Flags::C),
            BranchOp::BCS => cpu.get_flag(Flags::C),
            BranchOp::BEQ => cpu.get_flag(Flags::Z),
            BranchOp::BMI => cpu.get_flag(Flags::N),
            BranchOp::BNE => !cpu.get_flag(Flags::Z),
            BranchOp::BPL => !cpu.get_flag(Flags::N),
            BranchOp::BVC => !cpu.get_flag(Flags::V),
            BranchOp::BVS => cpu.get_flag(Flags::V),
        }
    }
}

impl ImpliedOp {
    fn execute(&self, cpu: &CPU) {
        match self {
            ImpliedOp::CLC => {
                cpu.set_flag(Flags::C, false);
            }
            ImpliedOp::CLD => {
                cpu.set_flag(Flags::D, false);
            }
            ImpliedOp::CLI => {
                cpu.set_flag(Flags::I, false);
            }
            ImpliedOp::CLV => {
                cpu.set_flag(Flags::V, false);
            }
            ImpliedOp::DEX => {
                let result = cpu.x.get().wrapping_sub(1);
                cpu.set_zn(result);
                cpu.x.set(result);
            }
            ImpliedOp::DEY => {
                let result = cpu.y.get().wrapping_sub(1);
                cpu.set_zn(result);
                cpu.y.set(result);
            }
            ImpliedOp::INX => {
                let result = cpu.x.get().wrapping_add(1);
                cpu.set_zn(result);
                cpu.x.set(result);
            }
            ImpliedOp::INY => {
                let result = cpu.y.get().wrapping_add(1);
                cpu.set_zn(result);
                cpu.y.set(result);
            }
            ImpliedOp::NOP => (),
            ImpliedOp::SEC => {
                cpu.set_flag(Flags::C, true);
            }
            ImpliedOp::SED => {
                cpu.set_flag(Flags::D, true);
            }
            ImpliedOp::SEI => {
                cpu.set_flag(Flags::I, true);
            }
            ImpliedOp::TAX => {
                let result = cpu.a.get();
                cpu.set_zn(result);
                cpu.x.set(result);
            }
            ImpliedOp::TAY => {
                let result = cpu.a.get();
                cpu.set_zn(result);
                cpu.y.set(result);
            }
            ImpliedOp::TSX => {
                let result = cpu.s.get();
                cpu.set_zn(result);
                cpu.x.set(result);
            }
            ImpliedOp::TXA => {
                let result = cpu.x.get();
                cpu.set_zn(result);
                cpu.a.set(result);
            }
            ImpliedOp::TXS => {
                let result = cpu.x.get();
                cpu.s.set(result);
            }
            ImpliedOp::TYA => {
                let result = cpu.y.get();
                cpu.set_zn(result);
                cpu.a.set(result);
            }
        }
    }
}

impl SHOp {
    fn execute(&self, cpu: &CPU, h: u8) -> u8 {
        match self {
            SHOp::SHY => {
                cpu.y.get() & h
            },
            SHOp::SHX => {
                cpu.x.get() & h
            },
        }
    }
}
