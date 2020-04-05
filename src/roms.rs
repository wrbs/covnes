use failure::{bail, Error};
use io::Read;
use std::io;

pub enum Mirroring {
    Horizontal,
    Vertical,
    FourScreen,
}

pub struct RomFile {
    pub mirroring: Mirroring,
    pub prg_rom: Vec<u8>,
    pub chr_rom: Vec<u8>,
}

const MAGIC_BYTES: [u8; 4] = [0x4E, 0x45, 0x53, 0x1A];

impl RomFile {
    pub fn from_read<R: Read>(f: &mut R) -> Result<RomFile, Error> {
        let mut header = [0; 16];
        let bytes_read = f.read(&mut header)?;

        if bytes_read < 16 {
            bail!("Could not read header");
        }

        if &header[0..4] != &MAGIC_BYTES {
            bail!("File is not in the iNES format");
        }

        assert!(header[4] == 1 || header[4] == 2);

        let prg_rom_size = (header[4] as usize) * 16384;
        let chr_rom_size = (header[5] as usize) * 8192;

        if header[6] & 0x4 == 0x4 {
            bail!("What's a trainer?")
        }

        let mirroring = if header[6] & 0x8 == 0x8 {
            Mirroring::FourScreen
        } else {
            if header[6] & 0x1 == 0x1 {
                Mirroring::Vertical
            } else {
                Mirroring::Horizontal
            }
        };

        if header[6] >> 4 != 0 && header[7] >> 4 != 0 {
            bail!("Only doing mapper 0 for now")
        }

        let mut prg_rom = vec![0; prg_rom_size];
        let read = f.read(&mut prg_rom[..])?;
        if read != prg_rom_size {
            bail!("Could not read all of the prg_rom");
        };

        if chr_rom_size == 0 {
            bail!("What does it mean when chr_rom_size is 0?");
        }

        let mut chr_rom = vec![0; chr_rom_size];
        let read = f.read(&mut chr_rom[..])?;
        if read != chr_rom_size {
            bail!("Could not read all of the chr_rom");
        }

        Ok(RomFile {
            mirroring,
            prg_rom,
            chr_rom,
        })
    }
}
