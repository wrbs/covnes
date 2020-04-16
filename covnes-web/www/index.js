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

const stepButton = document.getElementById('step');
const canvas = document.getElementById('screen');
const ctx = canvas.getContext('2d');

function step() {
    emu.tick_cycle();
    const pointer = emu.get_video();
    const cells = new Uint8Array(memory.buffer, pointer, 256 * 240 * 3);
    const imageData = ctx.createImageData(256, 240);
    for(let row = 0; row < 240; row++) {
        for(let col = 0; col < 256; col++) {
            let index = (row * 256 + col) * 3;
            let id_index = (row * 256 + col) * 4;
            const r = cells[index];
            const g = cells[index + 1];
            const b = cells[index + 2];

            imageData.data[id_index] = r;
            imageData.data[id_index + 1] = g;
            imageData.data[id_index + 2] = b;
            imageData.data[id_index + 3] = 255;
        }
    }

    ctx.putImageData(imageData, 0, 0);

    console.log("tick");
    window.requestAnimationFrame(step);
}

stepButton.onclick = function() {
    step()
};
