pub struct Ppu {
    pub vram: [u8; 2048],
    pub oam: [u8; 256],
    pub palette: [u8; 32],
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            vram: [0; 2048],
            oam: [0; 256],
            palette: [0; 32],
        }
    }

    pub fn tick(&mut self) {
        // PPU cycle
    }

    pub fn draw(&self, frame: &mut [u8]) {
        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            // Fill with random noise or pattern for verification
            let x = (i % 256) as u8;
            let y = (i / 256) as u8;
            
            let r = x.wrapping_add(y);
            let g = x;
            let b = y;

            pixel.copy_from_slice(&[r, g, b, 0xFF]);
        }
    }
}
