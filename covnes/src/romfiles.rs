use failure::{bail, Error};
use io::Read;
use std::fs::File;
use std::io;
use std::path::Path;

#[derive(Debug)]
pub enum Mirroring {
    Horizontal,
    Vertical,
    FourScreen,
}

#[derive(Debug)]
pub struct RomFile {
    pub prg_rom: Vec<u8>,
    pub chr_rom: Option<Vec<u8>>,
    pub provide_prg_ram: bool,
    pub mirroring: Mirroring,
    pub mapper: usize,
}

const MAGIC_BYTES: [u8; 4] = [0x4E, 0x45, 0x53, 0x1A];

impl RomFile {
    pub fn from_filename<P: AsRef<Path>>(path: P) -> Result<RomFile, Error> {
        let mut f = File::open(path)?;
        Self::from_read(&mut f)
    }

    pub fn from_read<R: Read>(f: &mut R) -> Result<RomFile, Error> {
        let mut header = [0; 16];
        let bytes_read = f.read(&mut header)?;

        if bytes_read < 16 {
            bail!("Could not read header");
        }

        if &header[0..4] != &MAGIC_BYTES {
            bail!("File is not in the iNES format");
        }

        let prg_rom_size = (header[4] as usize) * 16384;
        let chr_rom_size = (header[5] as usize) * 8192;

        let provide_prg_ram = header[6] & 2 == 2;
        let provide_trainer = header[6] & 4 == 4;

        if provide_trainer {
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

        let mapper_low = header[6] >> 4;
        let mapper = (header[7] & 0xF0) | mapper_low;

        // TODO other flags, NES 2.0, detect DiskDude!, etc.

        let mut prg_rom = vec![0; prg_rom_size];
        let read = f.read(&mut prg_rom[..])?;
        if read != prg_rom_size {
            bail!("Could not read all of the prg_rom");
        };

        let chr_rom = if chr_rom_size == 0 {
            None
        } else {
            let mut chr_rom = vec![0; chr_rom_size];
            let read = f.read(&mut chr_rom[..])?;
            if read != chr_rom_size {
                bail!("Could not read all of the chr_rom");
            }

            Some(chr_rom)
        };

        Ok(RomFile {
            mirroring,
            prg_rom,
            chr_rom,
            provide_prg_ram,
            mapper: mapper as usize,
        })
    }
}
