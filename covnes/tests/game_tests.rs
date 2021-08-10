use covnes::nes::cpu::CpuHostAccess;
use covnes::nes::io::DummyIO;
use covnes::nes::mappers;
use covnes::nes::Nes;
use covnes::romfiles::RomFile;
use failure::Error;
use regex::Regex;
use std::fs::File;
use std::io::{BufRead, BufReader};

// These test me against LaiNES. LaiNES is not perfect by any means but it provides an easy way to
// troubleshoot obvious issues. I couldn't get the timing to align with mesen, it's too accurate for
// what I need right now

const BUF_SIZE: usize = 1024;

#[test]
#[ignore]
fn dk_log_cmp() -> Result<(), Error> {
    log_cmp("dk")
}

#[test]
#[ignore]
fn ice_log_cmp() -> Result<(), Error> {
    log_cmp("ice")
}

// It's like 2 gigs, no time for that
// #[test]
// fn chace_log_cmp() -> Result<(), Error> {
//     log_cmp("Chase")
// }

fn log_cmp(game: &str) -> Result<(), Error> {
    // Load up the rom
    let mut f = File::open(format!("../roms/games/{}.nes", game))?;
    let log = File::open(format!("../roms/games/{}.log", game))?;
    let mut log = BufReader::new(log);

    let rom = RomFile::from_read(&mut f)?;
    let cart = mappers::from_rom(rom)?;
    let io = DummyIO;
    let mut nes = Nes::new(io);

    nes.insert_cartridge(cart);

    // Annoyingly, nestest doesn't do the right thing with the PPU after reset
    nes.ppu.dot.set(0);

    // It FFs the ram
    nes.cpu_ram.set([0xFF; 2048]);

    let re_ppu = Regex::new(r"P +(\d+) +(\d+): CTRL:([A-F0-9]{2}) STATUS:([A-F0-9]{2}) v:([A-F0-9]{4}) t:([A-F0-9]{4}) bsl:([A-F0-9]{4}) bsh:([A-F0-9]{4}) bgl:([A-F0-9]{2})").unwrap();
    let re_cpu = Regex::new(r"C ([A-F0-9]{4}) A:([A-F0-9]{2}) X:([A-F0-9]{2}) Y:([A-F0-9]{2}) P:([A-F0-9]{2}) S:([A-F0-9]{2}) tos:([A-F0-9]{2})").unwrap();

    let mut hackno = 0;

    let mut linebuf = Vec::with_capacity(BUF_SIZE);
    for _ in 0..BUF_SIZE {
        linebuf.push(String::new());
    }

    let mut i = 0;

    loop {
        let buf = &mut linebuf[i];

        buf.clear();
        log.read_line(buf)?;
        if buf == "" {
            break;
        }

        if buf.chars().next() == Some('D') {
        } else if buf.chars().next() == Some('P') {
            let cap = re_ppu.captures(&buf).unwrap();

            let expected_dot = u16::from_str_radix(&cap[1], 10).unwrap();
            let expected_sl = u16::from_str_radix(&cap[2], 10).unwrap();
            let expected_ctrl = u8::from_str_radix(&cap[3], 16).unwrap();
            let expected_status = u8::from_str_radix(&cap[4], 16).unwrap();
            let expected_v = u16::from_str_radix(&cap[5], 16).unwrap();
            let expected_t = u16::from_str_radix(&cap[6], 16).unwrap();
            let expected_bsl = u16::from_str_radix(&cap[7], 16).unwrap();
            let expected_bsh = u16::from_str_radix(&cap[8], 16).unwrap();
            let expected_bgl = u8::from_str_radix(&cap[9], 16).unwrap();

            let actual_dot = nes.ppu.dot.get();
            let actual_sl = nes.ppu.scanline.get();
            let actual_ctrl = nes.ppu.ppuctrl.get().bits();
            let actual_status = nes.ppu.ppustatus.get().bits();
            let actual_v = nes.ppu.addr_v.get();
            let actual_t = nes.ppu.addr_t.get();
            let actual_bsl = nes.ppu.bg_low_shift.get();
            let actual_bsh = nes.ppu.bg_high_shift.get();
            let actual_bgl = nes.ppu.fetched_bg_pattern_low.get();

            nes.tick();

            let mut fail = false;
            if expected_dot != actual_dot {
                println!("Bad dot E:{} A:{}", expected_dot, actual_dot);
                fail = true;
            }

            if expected_sl != actual_sl {
                println!("Bad scanline E:{} A:{}", expected_sl, actual_sl);
                fail = true;
            }

            if expected_ctrl != actual_ctrl {
                println!("Bad PPUCTRL E:{:02X} A:{:02X}", expected_ctrl, actual_ctrl);
                fail = true;
            }

            if expected_status != actual_status {
                println!(
                    "Bad PPUSTATUS E:{:02X} A:{:02X}",
                    expected_status, actual_status
                );
                fail = true;
            }

            if expected_v != actual_v {
                println!("Bad v E:{:04X} A:{:04X}", expected_v, actual_v);
                fail = true;
            }

            if expected_t != actual_t {
                println!("Bad t E:{:04X} A:{:04X}", expected_t, actual_t);
                fail = true;
            }

            if expected_bsl != actual_bsl {
                println!("Bad bsl E:{:04X} A:{:04X}", expected_bsl, actual_bsl);
                fail = true;
            }

            if expected_bsh != actual_bsh {
                println!("Bad bsh E:{:04X} A:{:04X}", expected_bsh, actual_bsh);
                fail = true;
            }

            if expected_bgl != actual_bgl {
                println!("Bad bgl E:{:02X} A:{:02X}", expected_bgl, actual_bgl);
                fail = true;
            }

            if fail {
                if hackno == 0 {
                    hackno += 1;
                    nes.ppu.reset();
                    nes.vram.set([0xFF; 2048]);
                    nes.ppu.tick(&nes);
                } else {
                    print_buf(&linebuf, i);
                    panic!("Bad ppu");
                }
            }
        } else {
            if !nes.cpu.is_at_instruction() {
                panic!(
                    "Cpu not at an instruction, it's at {:?}",
                    nes.cpu.state.get()
                )
            }
            let cap = re_cpu.captures(&buf).unwrap();

            let expected_pc = u16::from_str_radix(&cap[1], 16).unwrap();
            let expected_a = u8::from_str_radix(&cap[2], 16).unwrap();
            let expected_x = u8::from_str_radix(&cap[3], 16).unwrap();
            let expected_y = u8::from_str_radix(&cap[4], 16).unwrap();
            let expected_p = u8::from_str_radix(&cap[5], 16).unwrap();
            let expected_s = u8::from_str_radix(&cap[6], 16).unwrap();
            let expected_tos = u8::from_str_radix(&cap[7], 16).unwrap();

            let actual_pc = nes.cpu.pc.get();
            let actual_a = nes.cpu.a.get();
            let actual_x = nes.cpu.x.get();
            let actual_y = nes.cpu.y.get();
            let actual_p = nes.cpu.get_p() | 0x20;
            let actual_s = nes.cpu.s.get();
            let actual_tos = nes.read(0x100 | actual_s as u16);

            let mut fail = false;

            if expected_pc != actual_pc {
                println!("Bad pc E:{:04X} A:{:04X}", expected_pc, actual_pc);
                fail = true;
            }

            if expected_a != actual_a {
                println!("Bad a E:{:02X} A:{:02X}", expected_a, actual_a);
                fail = true;
            }

            if expected_x != actual_x {
                println!("Bad x E:{:02X} A:{:02X}", expected_x, actual_x);
                fail = true;
            }

            if expected_y != actual_y {
                println!("Bad y E:{:02X} A:{:02X}", expected_y, actual_y);
                fail = true;
            }

            if expected_p != actual_p {
                println!("Bad p E:{:02X} A:{:02X}", expected_p, actual_p);
                fail = true;
            }

            if expected_s != actual_s {
                println!("Bad s E:{:02X} A:{:02X}", expected_s, actual_s);
                fail = true;
            }

            if expected_tos != actual_tos {
                println!("Bad tos E:{:02X} A:{:02X}", expected_tos, actual_tos);
                fail = true;
            }

            if fail {
                print_buf(&linebuf, i);
                panic!("Bad CPU");
            }
        }

        i += 1;
        i %= BUF_SIZE;
    }

    Ok(())
}

fn print_buf(buf: &Vec<String>, i: usize) {
    let mut j = i;
    loop {
        j = (j + 1) % BUF_SIZE;
        print!("{}", buf[j]);
        if j == i {
            break;
        }
    }
}
