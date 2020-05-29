# covnes

Second 2020 Quarantine Project (first was [this midi
player](https://github.com/wrbs/software-synth)): a low-level NES emulator in Rust.

If you're actually looking for a NES emulator go use [mesen](https://www.mesen.ca/) which is much
more accurate, nice to use and full-featured. This project was about the act of creating more than
the creation at the end.

LICENSE: MIT for code. The test ROMs don't have an explicit license anywhere but are widely
distributed in other emulators - see [here](http://wiki.nesdev.com/w/index.php/Emulator_tests) for
author information.

## Features

### Emulator core

- A really accurate CPU:
    - dummy reads & dummy writes at correct cycles
    - nearly all opcodes (including undocumented/illegal)
    - passes every CPU test
- A dot-by-dot PPU that is pretty alright
    - does lookups of the ppu address space mostly on the right cycle
    - (I think) emulates the sprite overflow bug
    - doesn't do NTSC emulation - just uses an RGB palette
    - still some timing issues with exactly which cycle spirte 0 hit occurs on - I've gotten most
      of the way through the standard tests but couldn't quite get alignment right on some of them.
- No APU (so no sound)
- Very little optimisation but I've managed to get away with it on my computer up to now. YMMV.
  However it's completely unplayable in cargo dev profile.
- Mappers 0 (`NROM`), 1 (`SxROM`) and 2 (`UxROM`) which means it covers SMB1 but not SMB2 or SMB3
  (among many other games, especially earlier on in the NES's lifetime)

It started off very fast (easily able to keep up with the NTSC framerate) but started slowing down
a lot as I implemented more complicated things and sprites in the PPU. The CPU is probably as fast
as the approach (see below) is capable of so any optimisation effort should go towards the PPU
first.

It would be a lot of work but I would imagine that running the PPU and CPU out of order (but making
sure to synchronise before PPU/cartridge mapper register accesses) could help a lot on the
instruction and data cache usage front.

The other thing that could do with optimisation is that drawing for both the SDL and web interface
is much slower than it has to be (takes up about as long as computing the next frame does) - we
could perhaps offload some of it the GPU to lessen the load of the CPU. This also would give me a
chance to do fun things shaders to try and emulate an actual CRT TV better.

I don't actually have plans to do any of these things but I'm writing it here as a note for if I
ever want to return to the project some day.

### Interface

SDL interface:

- absolute bare minimum to play games with fixed keybindings
- some support for playing `.fm2` files from FCEUX for testing against TASes. All the SMB ones on
  the website work now and are quite fun to watch.

WASM:

- just a proof of concept to test whether browser WASM support would be fast enough to play the
  games. It is. The main thing that's slowing it down right now is actually due to not optimising
  the actual drawing of each pixel on the frame.

Both interfaces use VSync (`requesttAnimationFrame()` on web) so very much depend on the fact that
the monitor used is 60Hz to run at about the right frame rate. Abstracting away the actual refresh
rate from the emulation frame-rate could help a lot but I haven't had any need to do it so haven't
done it.

## Implementaiton notes

I'm going to focus on the CPU here. If you are trying to read the code there please remember that I
didn't go straight into writing this and debugging it in the format it is now - it was an iterative
process which I will describe below.

The current approach is that the CPU has all of the state that it needs in a single struct -
including what stage of executing the current instruction it is in.

If you are familiar with the 6502 or try to make an emulator you will know that a lot of
instructions behave identically with regards to timing/addressing behaviour which was used to
implement most of the opcodes.

The general approach is the CPU is as an explicit state machine (see
[here](https://byuu.net/design/cooperative-threading) for a very cool alternative approach that I
didn't dare try to do in Rust). Something else calls tick on the cpu with a callback for doing
Reads/Writes from memory. Some other emulators have the CPU control emulation and directly tick
other devices like the PPU but I preferred the closer correspondence to the actual hardware of
having the CPU be just another module.

A Rust GADT-style enum stores the current CPU 'state' (not including registers). This allows for a
nicer hybrid than the typical integer state approach you see in C/C++ state machines - we can for
example store in the state the actual operation to execute once all of the address mode fetches
have been done and so massively cut down on the number of states. We can also store past fetches
that the CPU would have somewhere in its pipeline in the state enum too so can avoid adding global
variables.

I didn't go straight to this (although it would be feasible now that I know more about the 6502 and
where all the good documentation is ([hint](http://nesdev.com/6502_cpu.txt)).

I was inspired by [this](https://kyle.space/posts/i-made-a-nes-emulator/) but didn't actually want
to use generators for everything after getting it working the first time so:

- I started by using rust 
  [generators](https://doc.rust-lang.org/beta/unstable-book/language-features/generators.html)
  to bootstrap the CPU. I knew my eventual method was going to be to use states so I had the same
  opcode decoder that currently exists decoding the instruction/addressing mode and another bit of
  code branching on that. However, there was no explicit state yet and every time I needed a cycle
  I used yield.
- The problem with the generators is there's no access to the actual generator stat and control
  flow/borrowing is a bit more awkward than I wanted so the eventual goal was to remove the
  generators mechanically into an explicit state and match statement.
    - This is what the compiler does, but the benefit of the approach is we can do some manual
      optimisation with the allocation in states (I probably do a worse job on the small things and
      better in the bigger picture than LLVM would with generators) and at the end we actually have
      the state enum explicitly and ready to be serialised for things like savestates.
    - The other reason is that generators are like closures in that they borrow everything for
      their whole lifetime. I don't want this, I only want the borrow to exist for as long as we're
      stepping the CPU so that we can actually mutate things outside of using `Cell`.
- I got the CPU passing nestest flawlessly using generators. I then added the state variable and
  step by step started adding the branching structure and states you see now (but still having
  everything inside a generator). This allowed me to always check that I hadn't broken anything and
  nestest still matched the log after every change and to iteratively refactor away yields into
  explicit state changes. 
- As I went I noticed that even some different addressing modes did the same operations as each
  other after the first few so was able to massively cut down on the number of states. I also
  noticed that Read ops/Write ops/Read-write ops shared a lot of the initial states so could
  combine that too.
- When I was done, I removed the final yield that the generator still had and had a nice and
  working state machine.

I would recommend this approach - not only to emulator authors but also more generally as a method
of making complicated things. Do the easy thing first with good tests and then refactor iteratively
keeeping tests passing.

PPU is not as interesting as there's less branching - it's basically a big match statement on the
current scanline and dot with sprite evaluation for the next scanline happening in parallel (among
other bits and pieces or timing hacks).
