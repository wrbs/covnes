import * as covnes from "covnes-web";
import { memory } from "covnes-web/covnes_web_bg";

covnes.init();
let emu = covnes.EmulatorState.new();

const romfile = document.getElementById('romfile');
romfile.addEventListener("change", romFileChange);

function romFileChange() {
    const files = this.files;
    const file = files[0];
    const reader = new FileReader();
    reader.onload = (evt) => {
        let ab = evt.target.result;
        const view = new Int8Array(ab);
        // try {
            emu.load_rom(view);
        // } catch(err) {
        //     document.getElementById('romfile_load_error').innerHTML = err.message;
        // }
    };
    reader.readAsArrayBuffer(file);
}

const scale = 2;
const canvas = document.getElementById('screen');
canvas.setAttribute('width', String(scale * 256));
canvas.setAttribute('height', String(scale * 240));
const ctx = canvas.getContext('2d');

const KEYCODES = {
    // j = A
    KeyJ: 0x1,
    // k = B
    KeyK: 0x2,
    // u = SELECT
    KeyU: 0x4,
    // i = START
    KeyI: 0x8,
    // w = UO
    KeyW: 0x10,
    // s = DOWN
    KeyS: 0x20,
    // a = LEFT
    KeyA: 0x40,
    // d = RIGHT
    KeyD: 0x80
};

let buttons = 0;
document.onkeydown = function(ev) {
    const mask = KEYCODES[ev.code];
    if(mask) {
        buttons |= mask;
    }
};

document.onkeyup = function(ev) {
    const mask = KEYCODES[ev.code];
    if(mask) {
        buttons &= ~mask;
    }
};

const playPauseButton = document.getElementById('play_pause');
playPauseButton.innerHtml = "Play";
let isPaused = true;

playPauseButton.onclick = function() {
    if(isPaused) {
        isPaused = false;
        playPauseButton.innerHTML = "Pause";
    } else {
        isPaused = true;
        playPauseButton.innerHTML = "Play";
    }
};

function step() {
    if(!isPaused) {
        emu.tick_cycle(buttons);
        const pointer = emu.get_video();
        const cells = new Uint8Array(memory.buffer, pointer, 256 * 240 * 3);
        const imageData = ctx.createImageData(256 * scale, 240 * scale);
        for(let row = 0; row < 240; row++) {
            for(let col = 0; col < 256; col++) {
                let index = (row * 256 + col) * 3;
                const r = cells[index];
                const g = cells[index + 1];
                const b = cells[index + 2];

                for(let i = 0; i < scale; i++) {
                    for(let j = 0; j < scale; j++) {
                        let id_index = ((row * scale + i) * 256 * scale + (col * scale + j)) * 4;
                        imageData.data[id_index] = r;
                        imageData.data[id_index + 1] = g;
                        imageData.data[id_index + 2] = b;
                        imageData.data[id_index + 3] = 255;
                    }
                }
            }
        }

        ctx.putImageData(imageData, 0, 0);
    }

    window.requestAnimationFrame(step);
}

step();
