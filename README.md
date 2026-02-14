# Rust Cross-Platform NES Emulator

A Nintendo Entertainment System (NES) emulator written in Rust, targeting generic desktop platforms (Windows/Mac/Linux) and WebAssembly (WASM).

## Features
- **Cross-Platform**: Runs natively on desktop and in modern web browsers.
- **Rendering**: Uses `pixels` for hardware-accelerated 2D pixel buffer rendering.
- **Web Support**: Built with `wasm-bindgen`.

## Prerequisites
- [Rust](https://www.rust-lang.org/tools/install)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer.html) (for Web build)
- Python 3 (optional, for local web server)

## Running Locally

### Desktop
Run the emulator natively:
```bash
cargo run
```
Use `Esc` to exit.

### Web (WASM)
1. Build the project for the web:
   ```bash
   wasm-pack build --target web --no-typescript
   ```
2. Start a local server:
   ```bash
   python3 -m http.server 8000
   ```
3. Open `http://localhost:8000` in your browser.

## Project Structure
- `src/main.rs`: Desktop entry point.
- `src/lib.rs`: Shared library & Web entry point.
- `src/cpu.rs`: 6502 CPU Implementation.
- `src/ppu.rs`: PPU (Picture Processing Unit) logic.
- `src/bus.rs`: Memory Bus.

## License
MIT
