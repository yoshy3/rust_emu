// NES APU (Audio Processing Unit) - NTSC
//
// Clean rewrite based on nesdev wiki specifications:
//   https://www.nesdev.org/wiki/APU

const LENGTH_TABLE: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 12, 16, 24, 18, 48, 20, 96, 22,
    192, 24, 72, 26, 16, 28, 32, 30,
];

const DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 1, 0, 0, 0, 0, 0, 0], // 12.5%
    [0, 1, 1, 0, 0, 0, 0, 0], // 25%
    [0, 1, 1, 1, 1, 0, 0, 0], // 50%
    [1, 0, 0, 1, 1, 1, 1, 1], // 25% negated
];

const TRIANGLE_SEQUENCE: [u8; 32] = [
    15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
    13, 14, 15,
];

const NOISE_PERIOD_TABLE: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
];

const DMC_PERIOD_TABLE: [u16; 16] = [
    428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106, 84, 72, 54,
];

// ─── Envelope ────────────────────────────────────────────────────────

struct Envelope {
    start: bool,
    loop_flag: bool, // Also serves as length counter halt for pulse/noise
    constant: bool,
    param: u8, // V: constant volume value / divider reload value
    divider: u8,
    decay: u8,
}

impl Envelope {
    fn new() -> Self {
        Self {
            start: false,
            loop_flag: false,
            constant: false,
            param: 0,
            divider: 0,
            decay: 0,
        }
    }

    fn clock(&mut self) {
        if self.start {
            self.start = false;
            self.decay = 15;
            self.divider = self.param;
        } else if self.divider > 0 {
            self.divider -= 1;
        } else {
            self.divider = self.param;
            if self.decay > 0 {
                self.decay -= 1;
            } else if self.loop_flag {
                self.decay = 15;
            }
        }
    }

    fn volume(&self) -> u8 {
        if self.constant {
            self.param
        } else {
            self.decay
        }
    }
}

// ─── Sweep Unit ──────────────────────────────────────────────────────

struct Sweep {
    enabled: bool,
    period: u8,
    negate: bool,
    shift: u8,
    reload: bool,
    divider: u8,
    ones_complement: bool, // true for Pulse 1, false for Pulse 2
}

impl Sweep {
    fn new(ones_complement: bool) -> Self {
        Self {
            enabled: false,
            period: 0,
            negate: false,
            shift: 0,
            reload: false,
            divider: 0,
            ones_complement,
        }
    }

    fn target_period(&self, current: u16) -> u16 {
        let delta = current >> self.shift;
        if self.negate {
            let result = current.wrapping_sub(delta);
            if self.ones_complement {
                result.wrapping_sub(1) // Pulse 1: one's complement
            } else {
                result // Pulse 2: two's complement
            }
        } else {
            current.wrapping_add(delta)
        }
    }

    fn is_muting(&self, current: u16) -> bool {
        current < 8 || (!self.negate && self.target_period(current) > 0x7FF)
    }

    fn clock(&mut self, timer_period: &mut u16) {
        if self.divider == 0 && self.enabled && self.shift > 0 && !self.is_muting(*timer_period) {
            *timer_period = self.target_period(*timer_period);
        }
        if self.divider == 0 || self.reload {
            self.divider = self.period;
            self.reload = false;
        } else {
            self.divider -= 1;
        }
    }
}

// ─── Pulse Channel ───────────────────────────────────────────────────

struct Pulse {
    enabled: bool,
    duty: u8,
    duty_pos: u8,
    timer: u16,
    timer_period: u16,
    length_counter: u8,
    envelope: Envelope,
    sweep: Sweep,
}

impl Pulse {
    fn new(ones_complement: bool) -> Self {
        Self {
            enabled: false,
            duty: 0,
            duty_pos: 0,
            timer: 0,
            timer_period: 0,
            length_counter: 0,
            envelope: Envelope::new(),
            sweep: Sweep::new(ones_complement),
        }
    }

    fn clock_timer(&mut self) {
        if self.timer > 0 {
            self.timer -= 1;
        } else {
            self.timer = self.timer_period;
            self.duty_pos = (self.duty_pos + 1) & 7;
        }
    }

    fn output(&self) -> u8 {
        if !self.enabled
            || self.length_counter == 0
            || self.sweep.is_muting(self.timer_period)
            || DUTY_TABLE[self.duty as usize][self.duty_pos as usize] == 0
        {
            0
        } else {
            self.envelope.volume()
        }
    }
}

// ─── Triangle Channel ────────────────────────────────────────────────

struct Triangle {
    enabled: bool,
    timer: u16,
    timer_period: u16,
    length_counter: u8,
    linear_counter: u8,
    linear_reload_value: u8,
    control_flag: bool, // Also length counter halt
    linear_reload_flag: bool,
    step: u8,
}

impl Triangle {
    fn new() -> Self {
        Self {
            enabled: false,
            timer: 0,
            timer_period: 0,
            length_counter: 0,
            linear_counter: 0,
            linear_reload_value: 0,
            control_flag: false,
            linear_reload_flag: false,
            step: 0,
        }
    }

    fn clock_timer(&mut self) {
        if self.timer > 0 {
            self.timer -= 1;
        } else {
            self.timer = self.timer_period;
            if self.length_counter > 0 && self.linear_counter > 0 {
                self.step = (self.step + 1) & 0x1F;
            }
        }
    }

    fn clock_linear_counter(&mut self) {
        if self.linear_reload_flag {
            self.linear_counter = self.linear_reload_value;
        } else if self.linear_counter > 0 {
            self.linear_counter -= 1;
        }
        if !self.control_flag {
            self.linear_reload_flag = false;
        }
    }

    fn output(&self) -> f32 {
        if !self.enabled || self.length_counter == 0 || self.linear_counter == 0 {
            0.0
        } else if self.timer_period < 2 {
            // Ultrasonic: DAC averages to ~7.5, effectively silence after HPF
            7.5
        } else {
            TRIANGLE_SEQUENCE[self.step as usize] as f32
        }
    }
}

// ─── Noise Channel ───────────────────────────────────────────────────

struct Noise {
    enabled: bool,
    timer: u16,
    timer_period: u16,
    length_counter: u8,
    shift_register: u16,
    mode: bool,
    envelope: Envelope,
}

impl Noise {
    fn new() -> Self {
        Self {
            enabled: false,
            timer: 0,
            timer_period: NOISE_PERIOD_TABLE[0],
            length_counter: 0,
            shift_register: 1, // Must be non-zero
            mode: false,
            envelope: Envelope::new(),
        }
    }

    fn clock_timer(&mut self) {
        if self.timer > 0 {
            self.timer -= 1;
        } else {
            self.timer = self.timer_period;
            let bit0 = self.shift_register & 1;
            let other = if self.mode {
                (self.shift_register >> 6) & 1
            } else {
                (self.shift_register >> 1) & 1
            };
            self.shift_register >>= 1;
            self.shift_register |= (bit0 ^ other) << 14;
        }
    }

    fn output(&self) -> u8 {
        if !self.enabled || self.length_counter == 0 || (self.shift_register & 1) != 0 {
            0
        } else {
            self.envelope.volume()
        }
    }
}

// ─── DMC Channel ─────────────────────────────────────────────────────

struct Dmc {
    enabled: bool,
    irq_enable: bool,
    loop_flag: bool,
    timer: u16,
    timer_period: u16,
    output_level: u8,
    sample_address: u16,
    sample_length: u16,
    current_address: u16,
    current_length: u16,
    sample_buffer: Option<u8>,
    shift_register: u8,
    bits_remaining: u8,
    silent: bool,
    irq_pending: bool,
}

impl Dmc {
    fn new() -> Self {
        Self {
            enabled: false,
            irq_enable: false,
            loop_flag: false,
            timer: 0,
            timer_period: DMC_PERIOD_TABLE[0],
            output_level: 0,
            sample_address: 0xC000,
            sample_length: 1,
            current_address: 0,
            current_length: 0,
            sample_buffer: None,
            shift_register: 0,
            bits_remaining: 8,
            silent: true,
            irq_pending: false,
        }
    }

    fn clock(&mut self) {
        if !self.silent {
            if (self.shift_register & 1) != 0 {
                if self.output_level <= 125 {
                    self.output_level += 2;
                }
            } else if self.output_level >= 2 {
                self.output_level -= 2;
            }
        }
        self.shift_register >>= 1;
        self.bits_remaining -= 1;
        if self.bits_remaining == 0 {
            self.bits_remaining = 8;
            if let Some(buf) = self.sample_buffer.take() {
                self.silent = false;
                self.shift_register = buf;
            } else {
                self.silent = true;
            }
        }
    }
}

// ─── APU ─────────────────────────────────────────────────────────────

pub struct Apu {
    pulse1: Pulse,
    pulse2: Pulse,
    triangle: Triangle,
    noise: Noise,
    dmc: Dmc,

    // Frame counter
    frame_mode: u8, // 0: 4-step, 1: 5-step
    frame_cycle: u32,
    irq_inhibit: bool,
    frame_irq: bool,

    // Even/odd CPU cycle tracking (pulse/noise tick on every other CPU cycle)
    odd_cycle: bool,

    // Output accumulator for oversampling
    accumulated_output: f32,
    accumulated_cycles: u32,

    // Debug: channel solo (0=all, 1=pulse1, 2=pulse2, 3=triangle, 4=noise, 5=dmc)
    pub solo_channel: u8,
}

#[allow(clippy::new_without_default)]
impl Apu {
    pub fn new() -> Self {
        Self {
            pulse1: Pulse::new(true),  // Pulse 1: one's complement negate
            pulse2: Pulse::new(false), // Pulse 2: two's complement negate
            triangle: Triangle::new(),
            noise: Noise::new(),
            dmc: Dmc::new(),
            frame_mode: 0,
            frame_cycle: 0,
            irq_inhibit: true,
            frame_irq: false,
            odd_cycle: false,
            accumulated_output: 0.0,
            accumulated_cycles: 0,
            solo_channel: 0,
        }
    }

    pub fn write_register(&mut self, addr: u16, data: u8) {
        match addr {
            // ── Pulse 1 ──
            0x4000 => {
                self.pulse1.duty = (data >> 6) & 3;
                self.pulse1.envelope.loop_flag = (data & 0x20) != 0;
                self.pulse1.envelope.constant = (data & 0x10) != 0;
                self.pulse1.envelope.param = data & 0x0F;
            }
            0x4001 => {
                self.pulse1.sweep.enabled = (data & 0x80) != 0;
                self.pulse1.sweep.period = (data >> 4) & 7;
                self.pulse1.sweep.negate = (data & 0x08) != 0;
                self.pulse1.sweep.shift = data & 0x07;
                self.pulse1.sweep.reload = true;
            }
            0x4002 => {
                self.pulse1.timer_period = (self.pulse1.timer_period & 0x0700) | data as u16;
            }
            0x4003 => {
                self.pulse1.timer_period =
                    (self.pulse1.timer_period & 0x00FF) | ((data as u16 & 7) << 8);
                if self.pulse1.enabled {
                    self.pulse1.length_counter = LENGTH_TABLE[(data >> 3) as usize];
                }
                // Omit duty_pos reset (real HW does it, but it causes audible clicks)
                self.pulse1.envelope.start = true;
            }

            // ── Pulse 2 ──
            0x4004 => {
                self.pulse2.duty = (data >> 6) & 3;
                self.pulse2.envelope.loop_flag = (data & 0x20) != 0;
                self.pulse2.envelope.constant = (data & 0x10) != 0;
                self.pulse2.envelope.param = data & 0x0F;
            }
            0x4005 => {
                self.pulse2.sweep.enabled = (data & 0x80) != 0;
                self.pulse2.sweep.period = (data >> 4) & 7;
                self.pulse2.sweep.negate = (data & 0x08) != 0;
                self.pulse2.sweep.shift = data & 0x07;
                self.pulse2.sweep.reload = true;
            }
            0x4006 => {
                self.pulse2.timer_period = (self.pulse2.timer_period & 0x0700) | data as u16;
            }
            0x4007 => {
                self.pulse2.timer_period =
                    (self.pulse2.timer_period & 0x00FF) | ((data as u16 & 7) << 8);
                if self.pulse2.enabled {
                    self.pulse2.length_counter = LENGTH_TABLE[(data >> 3) as usize];
                }
                self.pulse2.envelope.start = true;
            }

            // ── Triangle ──
            0x4008 => {
                self.triangle.control_flag = (data & 0x80) != 0;
                self.triangle.linear_reload_value = data & 0x7F;
            }
            0x400A => {
                self.triangle.timer_period = (self.triangle.timer_period & 0x0700) | data as u16;
            }
            0x400B => {
                self.triangle.timer_period =
                    (self.triangle.timer_period & 0x00FF) | ((data as u16 & 7) << 8);
                if self.triangle.enabled {
                    self.triangle.length_counter = LENGTH_TABLE[(data >> 3) as usize];
                }
                self.triangle.linear_reload_flag = true;
            }

            // ── Noise ──
            0x400C => {
                self.noise.envelope.loop_flag = (data & 0x20) != 0;
                self.noise.envelope.constant = (data & 0x10) != 0;
                self.noise.envelope.param = data & 0x0F;
            }
            0x400E => {
                self.noise.mode = (data & 0x80) != 0;
                self.noise.timer_period = NOISE_PERIOD_TABLE[(data & 0x0F) as usize];
            }
            0x400F => {
                if self.noise.enabled {
                    self.noise.length_counter = LENGTH_TABLE[(data >> 3) as usize];
                }
                self.noise.envelope.start = true;
            }

            // ── DMC ──
            0x4010 => {
                self.dmc.irq_enable = (data & 0x80) != 0;
                self.dmc.loop_flag = (data & 0x40) != 0;
                self.dmc.timer_period = DMC_PERIOD_TABLE[(data & 0x0F) as usize];
                if !self.dmc.irq_enable {
                    self.dmc.irq_pending = false;
                }
            }
            0x4011 => {
                self.dmc.output_level = data & 0x7F;
            }
            0x4012 => {
                self.dmc.sample_address = 0xC000 | ((data as u16) << 6);
            }
            0x4013 => {
                self.dmc.sample_length = ((data as u16) << 4) | 1;
            }

            // ── Status / Control ──
            0x4015 => {
                self.pulse1.enabled = (data & 0x01) != 0;
                if !self.pulse1.enabled {
                    self.pulse1.length_counter = 0;
                }
                self.pulse2.enabled = (data & 0x02) != 0;
                if !self.pulse2.enabled {
                    self.pulse2.length_counter = 0;
                }
                self.triangle.enabled = (data & 0x04) != 0;
                if !self.triangle.enabled {
                    self.triangle.length_counter = 0;
                }
                self.noise.enabled = (data & 0x08) != 0;
                if !self.noise.enabled {
                    self.noise.length_counter = 0;
                }
                self.dmc.enabled = (data & 0x10) != 0;
                if !self.dmc.enabled {
                    self.dmc.current_length = 0;
                } else if self.dmc.current_length == 0 {
                    self.dmc.current_address = self.dmc.sample_address;
                    self.dmc.current_length = self.dmc.sample_length;
                }
                self.dmc.irq_pending = false;
            }

            // ── Frame Counter ──
            0x4017 => {
                self.frame_mode = (data >> 7) & 1;
                self.irq_inhibit = (data & 0x40) != 0;
                if self.irq_inhibit {
                    self.frame_irq = false;
                }
                self.frame_cycle = 0;
                // 5-step mode: clock immediately
                if self.frame_mode == 1 {
                    self.clock_quarter_frame();
                    self.clock_half_frame();
                }
            }
            _ => {}
        }
    }

    pub fn read_status(&mut self) -> u8 {
        let mut status = 0u8;
        if self.pulse1.length_counter > 0 {
            status |= 0x01;
        }
        if self.pulse2.length_counter > 0 {
            status |= 0x02;
        }
        if self.triangle.length_counter > 0 {
            status |= 0x04;
        }
        if self.noise.length_counter > 0 {
            status |= 0x08;
        }
        if self.dmc.current_length > 0 {
            status |= 0x10;
        }
        if self.frame_irq {
            status |= 0x40;
        }
        if self.dmc.irq_pending {
            status |= 0x80;
        }
        // Reading $4015 clears the frame IRQ flag
        self.frame_irq = false;
        status
    }

    /// Compute the mixed output for this instant (0.0 – ~1.0).
    pub fn output(&self) -> f32 {
        let solo = self.solo_channel;

        let p1 = if solo == 0 || solo == 1 {
            self.pulse1.output() as f32
        } else {
            0.0
        };
        let p2 = if solo == 0 || solo == 2 {
            self.pulse2.output() as f32
        } else {
            0.0
        };
        let t = if solo == 0 || solo == 3 {
            self.triangle.output()
        } else {
            0.0
        };
        let n = if solo == 0 || solo == 4 {
            self.noise.output() as f32
        } else {
            0.0
        };
        let d = if solo == 0 || solo == 5 {
            self.dmc.output_level as f32
        } else {
            0.0
        };

        // NES non-linear mixer (approximation formula from nesdev wiki)
        let pulse_out = if p1 + p2 > 0.0 {
            95.88 / (8128.0 / (p1 + p2) + 100.0)
        } else {
            0.0
        };

        let tnd_out = if t + n + d > 0.0 {
            159.79 / (1.0 / (t / 8227.0 + n / 12241.0 + d / 22638.0) + 100.0)
        } else {
            0.0
        };

        pulse_out + tnd_out
    }

    /// Advance the APU by the given number of CPU cycles.
    pub fn tick(&mut self, cycles: u16) {
        for _ in 0..cycles {
            // Triangle timer: clocks every CPU cycle
            self.triangle.clock_timer();

            // Pulse & Noise timers: clock every other CPU cycle (APU rate = CPU/2)
            self.odd_cycle = !self.odd_cycle;
            if self.odd_cycle {
                self.pulse1.clock_timer();
                self.pulse2.clock_timer();
                self.noise.clock_timer();
            }

            // DMC timer: clocks every CPU cycle
            if self.dmc.timer > 0 {
                self.dmc.timer -= 1;
            } else {
                self.dmc.timer = self.dmc.timer_period;
                self.dmc.clock();
            }

            // Frame counter sequencer
            self.frame_cycle += 1;
            if self.frame_mode == 0 {
                // 4-step mode
                match self.frame_cycle {
                    7457 => self.clock_quarter_frame(),
                    14913 => {
                        self.clock_quarter_frame();
                        self.clock_half_frame();
                    }
                    22371 => self.clock_quarter_frame(),
                    29828 => {
                        if !self.irq_inhibit {
                            self.frame_irq = true;
                        }
                    }
                    29829 => {
                        self.clock_quarter_frame();
                        self.clock_half_frame();
                        if !self.irq_inhibit {
                            self.frame_irq = true;
                        }
                        self.frame_cycle = 0;
                    }
                    _ => {}
                }
            } else {
                // 5-step mode (no IRQ)
                match self.frame_cycle {
                    7457 => self.clock_quarter_frame(),
                    14913 => {
                        self.clock_quarter_frame();
                        self.clock_half_frame();
                    }
                    22371 => self.clock_quarter_frame(),
                    37281 => {
                        self.clock_quarter_frame();
                        self.clock_half_frame();
                        self.frame_cycle = 0;
                    }
                    _ => {}
                }
            }

            // Accumulate raw output for oversampling (no smoothing filter)
            self.accumulated_output += self.output();
            self.accumulated_cycles += 1;
        }
    }

    // ── Quarter frame: envelopes + triangle linear counter ──

    fn clock_quarter_frame(&mut self) {
        self.pulse1.envelope.clock();
        self.pulse2.envelope.clock();
        self.noise.envelope.clock();
        self.triangle.clock_linear_counter();
    }

    // ── Half frame: length counters + sweep units ──

    fn clock_half_frame(&mut self) {
        // Length counters (halt flag = envelope loop_flag for pulse/noise,
        // control_flag for triangle)
        if self.pulse1.length_counter > 0 && !self.pulse1.envelope.loop_flag {
            self.pulse1.length_counter -= 1;
        }
        if self.pulse2.length_counter > 0 && !self.pulse2.envelope.loop_flag {
            self.pulse2.length_counter -= 1;
        }
        if self.triangle.length_counter > 0 && !self.triangle.control_flag {
            self.triangle.length_counter -= 1;
        }
        if self.noise.length_counter > 0 && !self.noise.envelope.loop_flag {
            self.noise.length_counter -= 1;
        }

        // Sweep units
        self.pulse1.sweep.clock(&mut self.pulse1.timer_period);
        self.pulse2.sweep.clock(&mut self.pulse2.timer_period);
    }

    // ── DMC interface (used by Bus for memory-mapped sample fetch) ──

    pub fn dmc_needs_fetch(&self) -> bool {
        self.dmc.sample_buffer.is_none() && self.dmc.current_length > 0
    }

    pub fn dmc_fetch_address(&self) -> u16 {
        self.dmc.current_address
    }

    pub fn dmc_provide_sample(&mut self, data: u8) {
        self.dmc.sample_buffer = Some(data);
        self.dmc.current_address = self.dmc.current_address.wrapping_add(1);
        if self.dmc.current_address == 0 {
            self.dmc.current_address = 0x8000;
        }
        self.dmc.current_length -= 1;
        if self.dmc.current_length == 0 {
            if self.dmc.loop_flag {
                self.dmc.current_address = self.dmc.sample_address;
                self.dmc.current_length = self.dmc.sample_length;
            } else if self.dmc.irq_enable {
                self.dmc.irq_pending = true;
            }
        }
    }

    pub fn is_irq_pending(&self) -> bool {
        self.frame_irq || self.dmc.irq_pending
    }

    // ── Output accumulator (for oversampling at audio output rate) ──

    pub fn reset_accumulator(&mut self) {
        self.accumulated_output = 0.0;
        self.accumulated_cycles = 0;
    }

    pub fn averaged_output(&self) -> f32 {
        if self.accumulated_cycles > 0 {
            self.accumulated_output / self.accumulated_cycles as f32
        } else {
            self.output()
        }
    }
}
