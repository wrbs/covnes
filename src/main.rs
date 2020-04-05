#![feature(generators, generator_trait)]

mod cartridges;
mod cpu;
mod roms;
mod system;

#[cfg(test)]
mod tests;

fn main() {
    println!("Hello, world!");
}
