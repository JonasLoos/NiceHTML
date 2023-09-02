import * as wasm from './nicehtml_transpiler/pkg/nicehtml_transpiler.js';
// Start loading the WASM module
const wasmPromise = wasm.default();

// Fetch the scripts
const scripts = document.querySelectorAll('script[type="text/nicehtml"]');
const scriptPromises = Array.from(scripts).map((script) => {
    if (script.src) {
        // Fetch the script content
        return fetch(script.src + '?timestamp=' + Date.now())
            .then(response => response.text())
            .catch(error => console.error('Error loading script:', error));
    } else {
        // Use the inline script content
        return Promise.resolve(script.textContent);
    }
});

// Wait for the WASM module and scripts to load
Promise.all([wasmPromise, ...scriptPromises])
    .then(([wasmModule, ...scriptContents]) => {
        console.log('Start rendering scripts');
        scriptContents.forEach(content => wasm.transpile(content));
    });