#![feature(generators, generator_trait)]

#[macro_use]
extern crate bitflags;

pub mod io;
pub mod cpu;
pub mod mappers;
pub mod ppu;
pub mod romfiles;
pub mod system;
pub mod pallette;

pub use system::Nes;