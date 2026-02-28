const LENGTH_TABLE: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 12, 16, 24, 18, 48, 20, 96, 22,
    192, 24, 72, 26, 16, 28, 32, 30,
];

const TRIANGLE_TABLE: [u8; 32] = [
    15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
    13, 14, 15,
];

const DMC_PERIOD_TABLE: [u16; 16] = [
    428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106, 84, 72, 54,
];

pub struct Apu {
    // Pulse 1
    pulse1_enabled: bool,
    pulse1_length_counter: u8,
    pulse1_timer: u16,
    pulse1_timer_period: u16,
    pulse1_duty: u8,
    pulse1_duty_pos: u8,
    pulse1_envelope_loop: bool,
    pulse1_constant_volume: bool,
    pulse1_volume: u8,
    pulse1_env_start: bool,
    pulse1_env_divider: u8,
    pulse1_env_decay: u8,
    pulse1_sweep_enabled: bool,
    pulse1_sweep_period: u8,
    pulse1_sweep_negate: bool,
    pulse1_sweep_shift: u8,
    pulse1_sweep_reload: bool,
    pulse1_sweep_divider: u8,

    // Pulse 2
    pulse2_enabled: bool,
    pulse2_length_counter: u8,
    pulse2_timer: u16,
    pulse2_timer_period: u16,
    pulse2_duty: u8,
    pulse2_duty_pos: u8,
    pulse2_envelope_loop: bool,
    pulse2_constant_volume: bool,
    pulse2_volume: u8,
    pulse2_env_start: bool,
    pulse2_env_divider: u8,
    pulse2_env_decay: u8,
    pulse2_sweep_enabled: bool,
    pulse2_sweep_period: u8,
    pulse2_sweep_negate: bool,
    pulse2_sweep_shift: u8,
    pulse2_sweep_reload: bool,
    pulse2_sweep_divider: u8,

    // Triangle
    triangle_enabled: bool,
    triangle_length_counter: u8,
    triangle_timer: u16,
    triangle_timer_period: u16,
    triangle_linear_counter: u8,
    triangle_linear_counter_reload: u8,
    triangle_linear_control: bool,
    triangle_linear_reload: bool,
    triangle_step: u8,

    // Noise
    noise_enabled: bool,
    noise_length_counter: u8,
    noise_timer: u16,
    noise_timer_period: u16,
    noise_shift_register: u16,
    noise_mode: bool,
    noise_envelope_loop: bool,
    noise_constant_volume: bool,
    noise_volume: u8,
    noise_env_start: bool,
    noise_env_divider: u8,
    noise_env_decay: u8,

    // DMC
    dmc_enabled: bool,
    dmc_irq_enable: bool,
    dmc_loop_flag: bool,
    dmc_timer: u16,
    dmc_timer_period: u16,
    dmc_output_level: u8,
    dmc_sample_address: u16,
    dmc_sample_length: u16,
    dmc_current_address: u16,
    dmc_current_length: u16,
    dmc_sample_buffer: Option<u8>,
    dmc_shift_register: u8,
    dmc_bits_remaining: u8,
    dmc_silent: bool,
    dmc_irq_pending: bool,

    // Frame Counter
    frame_counter_mode: u8, // 0: 4-step, 1: 5-step
    frame_counter_cycle: u32,
    irq_inhibit: bool,
    irq_pending: bool,
}

impl Apu {
    pub fn new() -> Self {
        Self {
            pulse1_enabled: false,
            pulse1_length_counter: 0,
            pulse1_timer: 0,
            pulse1_timer_period: 0,
            pulse1_duty: 0,
            pulse1_duty_pos: 0,
            pulse1_envelope_loop: false,
            pulse1_constant_volume: false,
            pulse1_volume: 0,
            pulse1_env_start: false,
            pulse1_env_divider: 0,
            pulse1_env_decay: 0,
            pulse1_sweep_enabled: false,
            pulse1_sweep_period: 0,
            pulse1_sweep_negate: false,
            pulse1_sweep_shift: 0,
            pulse1_sweep_reload: false,
            pulse1_sweep_divider: 0,

            pulse2_enabled: false,
            pulse2_length_counter: 0,
            pulse2_timer: 0,
            pulse2_timer_period: 0,
            pulse2_duty: 0,
            pulse2_duty_pos: 0,
            pulse2_envelope_loop: false,
            pulse2_constant_volume: false,
            pulse2_volume: 0,
            pulse2_env_start: false,
            pulse2_env_divider: 0,
            pulse2_env_decay: 0,
            pulse2_sweep_enabled: false,
            pulse2_sweep_period: 0,
            pulse2_sweep_negate: false,
            pulse2_sweep_shift: 0,
            pulse2_sweep_reload: false,
            pulse2_sweep_divider: 0,

            triangle_enabled: false,
            triangle_length_counter: 0,
            triangle_timer: 0,
            triangle_timer_period: 0,
            triangle_linear_counter: 0,
            triangle_linear_counter_reload: 0,
            triangle_linear_control: false,
            triangle_linear_reload: false,
            triangle_step: 0,

            noise_enabled: false,
            noise_length_counter: 0,
            noise_timer: 0,
            noise_timer_period: 0,
            noise_shift_register: 1, // Must be non-zero
            noise_mode: false,
            noise_envelope_loop: false,
            noise_constant_volume: false,
            noise_volume: 0,
            noise_env_start: false,
            noise_env_divider: 0,
            noise_env_decay: 0,

            dmc_enabled: false,
            dmc_irq_enable: false,
            dmc_loop_flag: false,
            dmc_timer: 0,
            dmc_timer_period: DMC_PERIOD_TABLE[0],
            dmc_output_level: 0,
            dmc_sample_address: 0,
            dmc_sample_length: 0,
            dmc_current_address: 0,
            dmc_current_length: 0,
            dmc_sample_buffer: None,
            dmc_shift_register: 0,
            dmc_bits_remaining: 8,
            dmc_silent: true,
            dmc_irq_pending: false,

            frame_counter_mode: 0,
            frame_counter_cycle: 0,
            irq_inhibit: true,
            irq_pending: false,
        }
    }

    pub fn write_register(&mut self, addr: u16, data: u8) {
        match addr {
            0x4000 => {
                // Pulse 1: Duty, length halt, constant volume, volume/envelope
                self.pulse1_duty = (data & 0xC0) >> 6;
                self.pulse1_envelope_loop = (data & 0x20) != 0;
                self.pulse1_constant_volume = (data & 0x10) != 0;
                self.pulse1_volume = data & 0x0F;
            }
            0x4001 => {
                // Pulse 1: Sweep
                self.pulse1_sweep_enabled = (data & 0x80) != 0;
                self.pulse1_sweep_period = (data & 0x70) >> 4;
                self.pulse1_sweep_negate = (data & 0x08) != 0;
                self.pulse1_sweep_shift = data & 0x07;
                self.pulse1_sweep_reload = true;
            }
            0x4002 => {
                // Pulse 1: Timer low
                self.pulse1_timer_period = (self.pulse1_timer_period & 0x0700) | (data as u16);
            }
            0x4003 => {
                // Pulse 1: Length counter load, timer high
                self.pulse1_timer_period =
                    (self.pulse1_timer_period & 0x00FF) | (((data & 0x07) as u16) << 8);
                if self.pulse1_enabled {
                    self.pulse1_length_counter = LENGTH_TABLE[(data >> 3) as usize];
                }
                self.pulse1_duty_pos = 0; // Restart sequence
                self.pulse1_env_start = true; // Reset envelope
            }
            0x4004 => {
                // Pulse 2: Duty, length halt, constant volume, volume/envelope
                self.pulse2_duty = (data & 0xC0) >> 6;
                self.pulse2_envelope_loop = (data & 0x20) != 0;
                self.pulse2_constant_volume = (data & 0x10) != 0;
                self.pulse2_volume = data & 0x0F;
            }
            0x4005 => {
                // Pulse 2: Sweep
                self.pulse2_sweep_enabled = (data & 0x80) != 0;
                self.pulse2_sweep_period = (data & 0x70) >> 4;
                self.pulse2_sweep_negate = (data & 0x08) != 0;
                self.pulse2_sweep_shift = data & 0x07;
                self.pulse2_sweep_reload = true;
            }
            0x4006 => {
                // Pulse 2: Timer low
                self.pulse2_timer_period = (self.pulse2_timer_period & 0x0700) | (data as u16);
            }
            0x4007 => {
                // Pulse 2: Length counter load, timer high
                self.pulse2_timer_period =
                    (self.pulse2_timer_period & 0x00FF) | (((data & 0x07) as u16) << 8);
                if self.pulse2_enabled {
                    self.pulse2_length_counter = LENGTH_TABLE[(data >> 3) as usize];
                }
                self.pulse2_duty_pos = 0; // Restart sequence
                self.pulse2_env_start = true; // Reset envelope
            }
            0x4008 => {
                // Triangle: Linear counter
                self.triangle_linear_control = (data & 0x80) != 0;
                self.triangle_linear_counter_reload = data & 0x7F;
            }
            0x400A => {
                // Triangle: Timer low
                self.triangle_timer_period = (self.triangle_timer_period & 0x0700) | (data as u16);
            }
            0x400B => {
                // Triangle: Length counter load, timer high
                self.triangle_timer_period =
                    (self.triangle_timer_period & 0x00FF) | (((data & 0x07) as u16) << 8);
                if self.triangle_enabled {
                    self.triangle_length_counter = LENGTH_TABLE[(data >> 3) as usize];
                }
                self.triangle_linear_reload = true;
            }
            0x400C => {
                // Noise: Volume/Envelope
                self.noise_envelope_loop = (data & 0x20) != 0;
                self.noise_constant_volume = (data & 0x10) != 0;
                self.noise_volume = data & 0x0F;
            }
            0x400E => {
                // Noise: Period, Mode
                self.noise_mode = (data & 0x80) != 0;
                let noise_table = [
                    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2032, 4064,
                ];
                self.noise_timer_period = noise_table[(data & 0x0F) as usize];
            }
            0x400F => {
                // Noise: Length counter load
                if self.noise_enabled {
                    self.noise_length_counter = LENGTH_TABLE[(data >> 3) as usize];
                }
                self.noise_env_start = true; // Reset envelope
            }
            0x4010 => {
                self.dmc_irq_enable = (data & 0x80) != 0;
                self.dmc_loop_flag = (data & 0x40) != 0;
                self.dmc_timer_period = DMC_PERIOD_TABLE[(data & 0x0F) as usize];
                if !self.dmc_irq_enable {
                    self.dmc_irq_pending = false;
                }
            }
            0x4011 => {
                self.dmc_output_level = data & 0x7F;
            }
            0x4012 => {
                self.dmc_sample_address = 0xC000 | ((data as u16) << 6);
            }
            0x4013 => {
                self.dmc_sample_length = ((data as u16) << 4) | 1;
            }
            0x4015 => {
                // Status / Control
                self.pulse1_enabled = (data & 0x01) != 0;
                if !self.pulse1_enabled {
                    self.pulse1_length_counter = 0;
                }
                self.pulse2_enabled = (data & 0x02) != 0;
                if !self.pulse2_enabled {
                    self.pulse2_length_counter = 0;
                }
                self.triangle_enabled = (data & 0x04) != 0;
                if !self.triangle_enabled {
                    self.triangle_length_counter = 0;
                }
                self.noise_enabled = (data & 0x08) != 0;
                if !self.noise_enabled {
                    self.noise_length_counter = 0;
                }

                self.dmc_enabled = (data & 0x10) != 0;
                if !self.dmc_enabled {
                    self.dmc_current_length = 0;
                } else if self.dmc_current_length == 0 {
                    self.dmc_current_address = self.dmc_sample_address;
                    self.dmc_current_length = self.dmc_sample_length;
                }
                self.dmc_irq_pending = false;
            }
            0x4017 => {
                // Frame Counter
                self.frame_counter_mode = (data & 0x80) >> 7;
                self.irq_inhibit = (data & 0x40) != 0;
                if self.irq_inhibit {
                    self.irq_pending = false;
                }

                self.frame_counter_cycle = 0;
                // If 5-step mode, clock immediately
                if self.frame_counter_mode == 1 {
                    self.clock_envelopes();
                    self.clock_length_counters();
                }
            }
            _ => {}
        }
    }

    pub fn read_status(&mut self) -> u8 {
        let mut status = 0;
        if self.pulse1_length_counter > 0 {
            status |= 0x01;
        }
        if self.pulse2_length_counter > 0 {
            status |= 0x02;
        }
        if self.triangle_length_counter > 0 {
            status |= 0x04;
        }
        if self.noise_length_counter > 0 {
            status |= 0x08;
        }
        if self.dmc_current_length > 0 {
            status |= 0x10;
        }

        if self.irq_pending {
            status |= 0x40;
        }
        if self.dmc_irq_pending {
            status |= 0x80;
        }

        // Reading status clears frame IRQ flag, but NOT dmc irq flag?
        // Actually, reading $4015 clears the frame interrupt flag.
        self.irq_pending = false;
        status
    }

    pub fn output(&self) -> f32 {
        // Sweep mute: period < 8 always mutes; target > $7FF only mutes when NOT negating
        // (negate mode subtracts, so overflow into bit 11 cannot occur)
        let p1_mute = self.pulse1_timer_period < 8
            || (!self.pulse1_sweep_negate && self.pulse1_target_period() > 0x7FF);
        let p1 = if self.pulse1_length_counter > 0 && !p1_mute {
            let duty_table = [
                [0, 1, 0, 0, 0, 0, 0, 0],
                [0, 1, 1, 0, 0, 0, 0, 0],
                [0, 1, 1, 1, 1, 0, 0, 0],
                [1, 0, 0, 1, 1, 1, 1, 1],
            ];
            if duty_table[self.pulse1_duty as usize][self.pulse1_duty_pos as usize] == 1 {
                if self.pulse1_constant_volume {
                    self.pulse1_volume
                } else {
                    self.pulse1_env_decay
                }
            } else {
                0
            }
        } else {
            0
        };

        let p2_mute = self.pulse2_timer_period < 8
            || (!self.pulse2_sweep_negate && self.pulse2_target_period() > 0x7FF);
        let p2 = if self.pulse2_length_counter > 0 && !p2_mute {
            let duty_table = [
                [0, 1, 0, 0, 0, 0, 0, 0],
                [0, 1, 1, 0, 0, 0, 0, 0],
                [0, 1, 1, 1, 1, 0, 0, 0],
                [1, 0, 0, 1, 1, 1, 1, 1],
            ];
            if duty_table[self.pulse2_duty as usize][self.pulse2_duty_pos as usize] == 1 {
                if self.pulse2_constant_volume {
                    self.pulse2_volume
                } else {
                    self.pulse2_env_decay
                }
            } else {
                0
            }
        } else {
            0
        };

        let pulse_out = if p1 + p2 > 0 {
            95.88 / (8128.0 / (p1 as f32 + p2 as f32) + 100.0)
        } else {
            0.0
        };

        // Triangle
        let t = if self.triangle_length_counter > 0 && self.triangle_linear_counter > 0 {
            TRIANGLE_TABLE[self.triangle_step as usize] as f32
        } else {
            0.0
        };

        // Noise
        let n = if self.noise_length_counter > 0 && (self.noise_shift_register & 1) == 0 {
            if self.noise_constant_volume {
                self.noise_volume as f32
            } else {
                self.noise_env_decay as f32
            }
        } else {
            0.0
        };

        let d = self.dmc_output_level as f32;

        let tnd_out = if t + n + d > 0.0 {
            159.79 / (1.0 / (t / 8227.0 + n / 12241.0 + d / 22638.0) + 100.0)
        } else {
            0.0
        };

        pulse_out + tnd_out
    }

    pub fn tick(&mut self, cycles: u16) {
        for _ in 0..cycles {
            // Pulse and Noise timers decrement every 2 CPU cycles
            // For simplicity, we can use a cycle counter or just keep track in Apu.
            // Let's add a pulse_timer_phase or similar.

            // Triangle timer decrements every CPU cycle
            if self.triangle_timer > 0 {
                self.triangle_timer -= 1;
            } else {
                self.triangle_timer = self.triangle_timer_period;
                if self.triangle_length_counter > 0 && self.triangle_linear_counter > 0 {
                    self.triangle_step = (self.triangle_step + 1) & 0x1F;
                }
            }

            // Pulse 1, Pulse 2, Noise (every 2 cycles)
            if self.frame_counter_cycle % 2 == 0 {
                if self.pulse1_timer > 0 {
                    self.pulse1_timer -= 1;
                } else {
                    self.pulse1_timer = self.pulse1_timer_period;
                    self.pulse1_duty_pos = (self.pulse1_duty_pos + 1) & 0x07;
                }

                if self.pulse2_timer > 0 {
                    self.pulse2_timer -= 1;
                } else {
                    self.pulse2_timer = self.pulse2_timer_period;
                    self.pulse2_duty_pos = (self.pulse2_duty_pos + 1) & 0x07;
                }

                if self.noise_timer > 0 {
                    self.noise_timer -= 1;
                } else {
                    self.noise_timer = self.noise_timer_period;

                    let bit0 = self.noise_shift_register & 1;
                    let bit_n = if self.noise_mode {
                        (self.noise_shift_register >> 6) & 1
                    } else {
                        (self.noise_shift_register >> 1) & 1
                    };
                    let feedback = bit0 ^ bit_n;
                    self.noise_shift_register >>= 1;
                    self.noise_shift_register |= feedback << 14;
                }
            }

            // DMC
            if self.dmc_timer > 0 {
                self.dmc_timer -= 1;
            } else {
                self.dmc_timer = self.dmc_timer_period;
                self.clock_dmc();
            }

            self.frame_counter_cycle += 1;
            // ... (rest of frame counter logic)
            if self.frame_counter_mode == 0 {
                // 4-step mode
                match self.frame_counter_cycle {
                    7457 => self.clock_envelopes(),
                    14913 => {
                        self.clock_envelopes();
                        self.clock_length_counters();
                        self.clock_sweeps();
                    }
                    22371 => self.clock_envelopes(),
                    29828 => {
                        if !self.irq_inhibit {
                            self.irq_pending = true;
                        }
                    }
                    29829 => {
                        self.clock_envelopes();
                        self.clock_length_counters();
                        self.clock_sweeps();
                        self.frame_counter_cycle = 0;
                    }
                    _ => {}
                }
            } else {
                // 5-step mode
                match self.frame_counter_cycle {
                    7457 => self.clock_envelopes(),
                    14913 => {
                        self.clock_envelopes();
                        self.clock_length_counters();
                        self.clock_sweeps();
                    }
                    22371 => self.clock_envelopes(),
                    37281 => {
                        self.clock_envelopes();
                        self.clock_length_counters();
                        self.clock_sweeps();
                        self.frame_counter_cycle = 0;
                    }
                    _ => {}
                }
            }
        }
    }

    fn clock_envelopes(&mut self) {
        // Envelopes (Pulse 1, Pulse 2, Noise)
        // Pulse 1
        if self.pulse1_env_start {
            self.pulse1_env_start = false;
            self.pulse1_env_decay = 15;
            self.pulse1_env_divider = self.pulse1_volume;
        } else {
            if self.pulse1_env_divider > 0 {
                self.pulse1_env_divider -= 1;
            } else {
                self.pulse1_env_divider = self.pulse1_volume;
                if self.pulse1_env_decay > 0 {
                    self.pulse1_env_decay -= 1;
                } else if self.pulse1_envelope_loop {
                    self.pulse1_env_decay = 15;
                }
            }
        }

        // Pulse 2
        if self.pulse2_env_start {
            self.pulse2_env_start = false;
            self.pulse2_env_decay = 15;
            self.pulse2_env_divider = self.pulse2_volume;
        } else {
            if self.pulse2_env_divider > 0 {
                self.pulse2_env_divider -= 1;
            } else {
                self.pulse2_env_divider = self.pulse2_volume;
                if self.pulse2_env_decay > 0 {
                    self.pulse2_env_decay -= 1;
                } else if self.pulse2_envelope_loop {
                    self.pulse2_env_decay = 15;
                }
            }
        }

        // Noise
        if self.noise_env_start {
            self.noise_env_start = false;
            self.noise_env_decay = 15;
            self.noise_env_divider = self.noise_volume;
        } else {
            if self.noise_env_divider > 0 {
                self.noise_env_divider -= 1;
            } else {
                self.noise_env_divider = self.noise_volume;
                if self.noise_env_decay > 0 {
                    self.noise_env_decay -= 1;
                } else if self.noise_envelope_loop {
                    self.noise_env_decay = 15;
                }
            }
        }

        // Linear counter (Triangle)
        if self.triangle_linear_reload {
            self.triangle_linear_counter = self.triangle_linear_counter_reload;
        } else if self.triangle_linear_counter > 0 {
            self.triangle_linear_counter -= 1;
        }
        if !self.triangle_linear_control {
            self.triangle_linear_reload = false;
        }
    }

    fn clock_sweeps(&mut self) {
        // Pulse 1 Sweep
        let p1_target = self.pulse1_target_period();
        let p1_mute = self.pulse1_timer_period < 8 || p1_target > 0x7FF;
        if self.pulse1_sweep_divider == 0
            && self.pulse1_sweep_enabled
            && !p1_mute
            && self.pulse1_sweep_shift > 0
        {
            self.pulse1_timer_period = p1_target;
        }
        if self.pulse1_sweep_divider == 0 || self.pulse1_sweep_reload {
            self.pulse1_sweep_divider = self.pulse1_sweep_period;
            self.pulse1_sweep_reload = false;
        } else {
            self.pulse1_sweep_divider -= 1;
        }

        // Pulse 2 Sweep
        let p2_target = self.pulse2_target_period();
        let p2_mute = self.pulse2_timer_period < 8 || p2_target > 0x7FF;
        if self.pulse2_sweep_divider == 0
            && self.pulse2_sweep_enabled
            && !p2_mute
            && self.pulse2_sweep_shift > 0
        {
            self.pulse2_timer_period = p2_target;
        }
        if self.pulse2_sweep_divider == 0 || self.pulse2_sweep_reload {
            self.pulse2_sweep_divider = self.pulse2_sweep_period;
            self.pulse2_sweep_reload = false;
        } else {
            self.pulse2_sweep_divider -= 1;
        }
    }

    fn pulse1_target_period(&self) -> u16 {
        let delta = self.pulse1_timer_period >> self.pulse1_sweep_shift;
        if self.pulse1_sweep_negate {
            self.pulse1_timer_period.wrapping_sub(delta).wrapping_sub(1)
        } else {
            self.pulse1_timer_period.wrapping_add(delta)
        }
    }

    fn pulse2_target_period(&self) -> u16 {
        let delta = self.pulse2_timer_period >> self.pulse2_sweep_shift;
        if self.pulse2_sweep_negate {
            self.pulse2_timer_period.wrapping_sub(delta)
        } else {
            self.pulse2_timer_period.wrapping_add(delta)
        }
    }

    fn clock_length_counters(&mut self) {
        // Length counter halt flag = envelope_loop for pulse/noise, linear_control for triangle
        if self.pulse1_length_counter > 0 && !self.pulse1_envelope_loop {
            self.pulse1_length_counter -= 1;
        }
        if self.pulse2_length_counter > 0 && !self.pulse2_envelope_loop {
            self.pulse2_length_counter -= 1;
        }
        if self.triangle_length_counter > 0 && !self.triangle_linear_control {
            self.triangle_length_counter -= 1;
        }
        if self.noise_length_counter > 0 && !self.noise_envelope_loop {
            self.noise_length_counter -= 1;
        }
    }

    fn clock_dmc(&mut self) {
        if !self.dmc_silent {
            if (self.dmc_shift_register & 0x01) != 0 {
                if self.dmc_output_level <= 125 {
                    self.dmc_output_level += 2;
                }
            } else {
                if self.dmc_output_level >= 2 {
                    self.dmc_output_level -= 2;
                }
            }
        }
        self.dmc_shift_register >>= 1;
        self.dmc_bits_remaining -= 1;
        if self.dmc_bits_remaining == 0 {
            self.dmc_bits_remaining = 8;
            if let Some(buf) = self.dmc_sample_buffer {
                self.dmc_silent = false;
                self.dmc_shift_register = buf;
                self.dmc_sample_buffer = None;
            } else {
                self.dmc_silent = true;
            }
        }
    }

    pub fn dmc_needs_fetch(&self) -> bool {
        self.dmc_sample_buffer.is_none() && self.dmc_current_length > 0
    }

    pub fn dmc_fetch_address(&self) -> u16 {
        self.dmc_current_address
    }

    pub fn dmc_provide_sample(&mut self, data: u8) {
        self.dmc_sample_buffer = Some(data);
        self.dmc_current_address = self.dmc_current_address.wrapping_add(1);
        if self.dmc_current_address == 0 {
            self.dmc_current_address = 0x8000;
        }
        self.dmc_current_length -= 1;
        if self.dmc_current_length == 0 {
            if self.dmc_loop_flag {
                self.dmc_current_address = self.dmc_sample_address;
                self.dmc_current_length = self.dmc_sample_length;
            } else if self.dmc_irq_enable {
                self.dmc_irq_pending = true;
            }
        }
    }

    pub fn is_irq_pending(&self) -> bool {
        self.irq_pending || self.dmc_irq_pending
    }
}
