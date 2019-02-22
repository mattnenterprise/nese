use cpu;

const TRIANGLE_SEQUENCE_TABLE: [u16; 32] = [
    15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0,
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15
];

const LENGTH_COUNTER_TABLE: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14,
    12, 16, 24, 18, 48, 20, 96, 22, 192, 24, 72, 26, 16, 28, 32, 30];

const NOISE_PERIOD_TABLE: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068
];

const DMC_PERIOD_TABLE: [u16; 16] = [
    214, 190, 170, 160, 143, 127, 113, 107, 95, 80, 71, 64, 53, 42, 36, 27
];

const DUTY_CYCLE_TABLE: [u8; 32] = [
    0,1,0,0,0,0,0,0,
    0,1,1,0,0,0,0,0,
    0,1,1,1,1,0,0,0,
    1,0,0,1,1,1,1,1
];

fn create_mixer_pulse_table() -> [f32; 31] {
    let mut table = [0.0; 31];
    for i in 0..31 {
        table[i] = 95.52 / (8128.0 / (i as f32) + 100.0);
    }
    return table;
}

fn create_mixer_tnd_table() -> [f32; 203] {
    let mut table = [0.0; 203];
    for i in 0..203 {
        table[i] = 163.67 / (24329.0 / (i as f32) + 100.0);
    }
    return table;
}

pub struct APU {
    pulse1: Pulse,
    pulse2: Pulse,
    triangle: Triangle,
    noise: Noise,
    dmc: DMC,
    frame_counter: u64,
    frame_counter_mode: u8,
    inhibit_irq: bool,
    mixer_pulse_table: [f32; 31],
    mixer_tnd_table: [f32; 203],
}

impl APU {
    pub fn new() -> APU {
        return APU{
            pulse1: Pulse::new(PulseChannelType::One),
            pulse2: Pulse::new(PulseChannelType::Two),
            triangle: Triangle::new(),
            noise: Noise::new(),
            dmc: DMC::new(),
            frame_counter: 0,
            frame_counter_mode: 0,
            inhibit_irq: false,
            mixer_pulse_table: create_mixer_pulse_table(),
            mixer_tnd_table: create_mixer_tnd_table(),
        }
    }

    pub fn write_register(&mut self, address: u16, value: u8) {
        match address {
            0x4000 => {
                self.pulse1.write_controls(value);
            }
            0x4001 => {
                self.pulse1.write_sweep(value);
            },
            0x4002 => {
                self.pulse1.write_timer_period_low(value);
            },
            0x4003 => {
                self.pulse1.write_length_counter_and_timer_period_high(value);
            },
            0x4004 => {
                self.pulse2.write_controls(value);
            }
            0x4005 => {
                self.pulse2.write_sweep(value);
            },
            0x4006 => {
                self.pulse2.write_timer_period_low(value);
            },
            0x4007 => {
                self.pulse2.write_length_counter_and_timer_period_high(value);
            },
            0x4008 => {
                self.triangle.write_linear_counter_and_control(value);
            },
            0x4009 => {
                // Ignore this register
            }
            0x400A => {
                self.triangle.write_timer_period_low(value);
            },
            0x400B => {
                self.triangle.write_length_counter_and_timer_period_high(value);
            },
            0x400C => {
                self.noise.write_controls(value);
            },
            0x400D => {
                // Ignore this register
            }
            0x400E => {
                self.noise.write_mode_flag_and_timer_period(value);
            },
            0x400F => {
                self.noise.write_length_counter(value);
            },
            0x4010 => {
                self.dmc.write_controls(value);
            }
            0x4011 => {
                self.dmc.write_output(value);
            }
            0x4012 => {
                self.dmc.write_sample_address(value);
            },
            0x4013 => {
                self.dmc.write_sample_length(value);
            },
            0x4015 => {
                self.pulse1.enabled = value & 1 == 1;
                self.pulse2.enabled = (value >> 1) & 1 == 1;
                self.triangle.enabled = (value >> 2) & 1 == 1;
                self.noise.enabled = (value >> 3) & 1 == 1;
                self.dmc.enabled = (value >> 4) & 1 == 1;
                if self.dmc.enabled && self.dmc.sample_length_counter == 0 {
                    self.dmc.reset();
                }
            },
            0x4017 => {
                self.frame_counter_mode = (value >> 7) & 1;
                self.inhibit_irq = value & 0x40 == 1;
            }
            _ => {
                panic!("Address {:#X} not implemented", address);
            }
        }
    }

    pub fn step<T: cpu::Memory>(&mut self, cpu_cycle: u64, cpu: &mut cpu::CPU<T>) {
        self.step_timer(cpu_cycle, cpu);
        if cpu_cycle % 2 == 0 {
            self.step_frame_counter();
        }
    }

    fn step_timer<T: cpu::Memory>(&mut self, cpu_cycle: u64, cpu: &mut cpu::CPU<T>) {
        if cpu_cycle % 2 == 0 {
            self.pulse1.step_timer();
            self.pulse2.step_timer();
            self.noise.step_timer();
            self.dmc.step_timer(cpu);
        }
        self.triangle.step_timer();
    }

    fn step_frame_counter(&mut self) {
        self.frame_counter += 1;
        if self.frame_counter_mode == 0 {
            // 4 step mode
            match self.frame_counter {
                3729 => {
                    self.step_envelope();
                    self.triangle.step_linear_counter();
                },
                7457 => {
                    self.step_envelope();
                    self.triangle.step_linear_counter();
                    self.step_length_counter();
                    self.step_sweep();
                },
                11186 => {
                    self.step_envelope();
                    self.triangle.step_linear_counter();
                },
                14915 => {
                    self.step_envelope();
                    self.triangle.step_linear_counter();
                    self.step_length_counter();
                    self.step_sweep();
                    self.frame_counter = 0;
                },
                _ => {}
            }
        } else if self.frame_counter_mode == 1{
            // 5 step mode
            match self.frame_counter {
                3729 => {
                    self.step_envelope();
                    self.triangle.step_linear_counter();
                },
                7457 => {
                    self.step_envelope();
                    self.triangle.step_linear_counter();
                    self.step_length_counter();
                    self.step_sweep();
                },
                11186 => {
                    self.step_envelope();
                    self.triangle.step_linear_counter();
                },
                14915 => {},
                18641 => {
                    self.step_envelope();
                    self.triangle.step_linear_counter();
                    self.step_length_counter();
                    self.step_sweep();
                    self.frame_counter = 0;
                },
                _ => {}
            }
        }

    }

    fn step_envelope(&mut self) {
        self.pulse1.step_envelope();
        self.pulse2.step_envelope();
        self.noise.step_envelope();
    }

    fn step_length_counter(&mut self) {
        self.pulse1.step_length_counter();
        self.pulse2.step_length_counter();
        self.triangle.step_length_counter();
        self.noise.step_length_counter();
    }

    fn step_sweep(&mut self) {
        self.pulse1.step_sweep();
        self.pulse2.step_sweep();
    }

    pub fn output(&mut self) -> f32 {
        let pulse1_out = self.pulse1.output();
        let pulse2_out = self.pulse2.output();
        let pulse_out = self.mixer_pulse_table[(pulse1_out + pulse2_out) as usize];
        let triangle_out = self.triangle.output();
        let noise_out = self.noise.output();
        let dmc_out = self.dmc.get_output();
        let tnd_out = self.mixer_tnd_table[((3*triangle_out) + (2*noise_out) + dmc_out) as usize];
        return pulse_out + tnd_out;
    }
}

enum PulseChannelType {
    One,
    Two,
}

struct Pulse {
    channel_type: PulseChannelType,
    use_constant_volume: bool,
    duty_cycle: u8,
    constant_volume: u8,
    enabled: bool,
    envelope_counter: u8,
    envelope_decay_level_counter: u8,
    envelope_loop: bool,
    envelope_period: u8,
    envelope_start_flag: bool,
    length_counter_halt: bool,
    length_counter: u8,
    sweep_counter: u8,
    sweep_enabled: bool,
    sweep_period: u8,
    sweep_negate: bool,
    sweep_shift_count: u8,
    sweep_reload_flag: bool,
    timer_period: u16,
    timer: u16,
    sequence_index: u8,
}

impl Pulse {
    fn new(channel_type: PulseChannelType) -> Pulse {
        Pulse{
            channel_type: channel_type,
            use_constant_volume: false,
            duty_cycle: 0,
            constant_volume: 0,
            enabled: false,
            envelope_counter: 0,
            envelope_decay_level_counter: 0,
            envelope_loop: false,
            envelope_period: 0,
            envelope_start_flag: false,
            length_counter_halt: false,
            length_counter: 0,
            sweep_counter: 0,
            sweep_enabled: false,
            sweep_period: 0,
            sweep_negate: false,
            sweep_shift_count: 0,
            sweep_reload_flag: false,
            timer_period: 0,
            timer: 0,
            sequence_index: 0,
        }
    }

    fn step_timer(&mut self) {
        if self.timer == 0 {
            self.timer =  self.timer_period;
            self.sequence_index = (self.sequence_index + 1) % 8;
        } else {
            self.timer -= 1;
        }
    }

    fn step_length_counter(&mut self) {
        if !self.length_counter_halt && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    fn step_envelope(&mut self) {
        if !self.envelope_start_flag {
            if self.envelope_counter <= 0 {
                self.envelope_counter = self.envelope_period;
                if self.envelope_decay_level_counter > 0 {
                    self.envelope_decay_level_counter -= 1;
                } else if self.envelope_loop {
                    self.envelope_decay_level_counter = 15;
                }
            } else {
                self.envelope_counter = 0;
            }
        } else {
            self.envelope_start_flag = false;
            self.envelope_decay_level_counter = 15;
            self.envelope_counter = self.envelope_period;
        }
    }

    fn step_sweep(&mut self) {
        if self.sweep_counter == 0 && self.sweep_enabled && self.sweep_shift_count > 0 {
            let change_amount = self.timer_period >> self.sweep_shift_count;
            if self.sweep_negate {
                self.timer_period -= change_amount;
                match self.channel_type {
                    PulseChannelType::One => {
                        self.timer_period -= 1;
                    }
                    _ => {},
                }
            } else {
                 self.timer_period += change_amount;
            }
        }
        if self.sweep_counter == 0 || self.sweep_reload_flag {
            self.sweep_counter = self.sweep_period;
            self.sweep_reload_flag = false;
        } else {
            self.sweep_counter -= 1;
        }
    }

    // $4000 / $4004
    fn write_controls(&mut self, value: u8) {
        self.duty_cycle = value >> 6;
        self.length_counter_halt = (value >> 5) & 1 == 1;
        self.envelope_loop = (value >> 5) & 1 == 1;
        self.use_constant_volume = (value >> 4) & 1 == 1;
        self.constant_volume = value & 0x0F;
        self.envelope_period = value & 0x0F;
    }

    // $4001 / $4005
    fn write_sweep(&mut self, value: u8) {
        self.sweep_enabled = ((value >> 7) & 1) == 1;
        self.sweep_period = value >> 4 & 0x7;
        self.sweep_negate = (value >> 3) & 1 == 1;
        self.sweep_shift_count = value & 7;
        self.sweep_reload_flag = true;
    }

    // $4002 / $4006
    fn write_timer_period_low(&mut self, low: u8) {
        self.timer_period = (self.timer_period & 0xFF00) | (low as u16);
    }

    // $4003 / $4007
    fn write_length_counter_and_timer_period_high(&mut self, value: u8) {
        self.timer_period = (self.timer_period & 0x00FF) | (((value & 0x7) as u16) << 8);
        self.length_counter = LENGTH_COUNTER_TABLE[((value & 0xF8) >> 3) as usize];
        self.sequence_index = 0;
        self.envelope_start_flag = true;
    }

    fn output(&mut self) -> u32 {
        if self.timer < 8 {
            return 0;
        }
        if self.length_counter == 0 {
            return 0;
        }
        if !self.enabled {
            return 0;
        }
        if DUTY_CYCLE_TABLE[((self.duty_cycle*8) + self.sequence_index) as usize] == 0 {
            return 0;
        }

        if self.use_constant_volume {
            return self.constant_volume as u32;
        }
        return self.envelope_decay_level_counter as u32;
    }
}

struct Triangle {
    enabled: bool,
    control_flag: bool,
    length_counter_halt: bool,
    length_counter: u8,
    linear_counter: u8,
    linear_counter_period: u8,
    linear_counter_reload_flag: bool,
    sequence_index: u8,
    timer_period: u16,
    timer: u16
}

impl Triangle {
    fn new() -> Triangle {
        Triangle{
            enabled: false,
            control_flag: false,
            length_counter_halt: false,
            length_counter: 0,
            linear_counter: 0,
            linear_counter_period: 0,
            linear_counter_reload_flag: false,
            sequence_index: 0,
            timer_period: 0,
            timer: 0,
        }
    }

    fn step_timer(&mut self) {
        if self.timer == 0 {
            self.timer =  self.timer_period;
            if self.length_counter > 0 && self.linear_counter > 0 {
                self.sequence_index = (self.sequence_index + 1) % 32;
            }
        } else {
            self.timer -= 1;
        }
    }

    fn step_length_counter(&mut self) {
        if !self.length_counter_halt && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    fn step_linear_counter(&mut self) {
        if self.linear_counter_reload_flag {
            self.linear_counter = self.linear_counter_period;
        } else if self.linear_counter > 0 {
            self.linear_counter -= 1;
        }
        if !self.control_flag {
            self.linear_counter_reload_flag = false;
        }
    } 

    // $4008
    fn write_linear_counter_and_control(&mut self, value: u8) {
        self.control_flag = (value >> 7) & 1 == 1;
        self.length_counter_halt = (value >> 7) & 1 == 1;
        self.linear_counter_period = value & 0x7F;
    }

    // $400A
    fn write_timer_period_low(&mut self, low: u8) {
        self.timer_period = (self.timer_period & 0xFF00) | (low as u16);
    }

    // $400B
    fn write_length_counter_and_timer_period_high(&mut self, value: u8) {
        self.timer_period = (self.timer_period & 0x00FF) | (((value & 0x7) as u16) << 8);
        self.length_counter = LENGTH_COUNTER_TABLE[((value & 0xF8) >> 3) as usize];
        self.linear_counter_reload_flag = true;
    }

    fn output(&mut self) -> u16 {
        if !self.enabled {
            return 0;
        }
        if self.length_counter == 0 {
            return 0;
        }
        if self.linear_counter == 0 {
            return 0;
        }
        return TRIANGLE_SEQUENCE_TABLE[self.sequence_index as usize] as u16;
    }
}

struct Noise {
    enabled: bool,
    mode_flag: bool,
    length_counter_halt: bool,
    length_counter: u8,
    envelope_counter: u8,
    envelope_decay_level_counter: u8,
    envelope_loop: bool,
    envelope_period: u8,
    envelope_start_flag: bool,
    timer: u16,
    timer_period: u16,
    shift_register: u16,
    use_constant_volume: bool,
    constant_volume: u8,
}

impl Noise {
    fn new() -> Noise {
        Noise {
            enabled: false,
            mode_flag: false,
            length_counter_halt: false,
            length_counter: 0,
            envelope_counter: 0,
            envelope_decay_level_counter: 0,
            envelope_loop: false,
            envelope_period: 0,
            envelope_start_flag: false,
            timer: 0,
            timer_period: 0,
            shift_register: 1,
            use_constant_volume: false,
            constant_volume: 0,
        }
    }

    fn step_timer(&mut self) {
        if self.timer == 0 {
            self.timer =  self.timer_period;
            let mut feedback = (self.shift_register & 1) ^ ((self.shift_register >> 1) & 1);
            if self.mode_flag {
                feedback = (self.shift_register & 1) ^ ((self.shift_register >> 6) & 1);
            }
            self.shift_register >>= 1;
            self.shift_register = (feedback << 14) | self.shift_register;
        } else {
            self.timer -= 1;
        }
    }

    fn step_length_counter(&mut self) {
        if !self.length_counter_halt && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    fn step_envelope(&mut self) {
        if !self.envelope_start_flag {
            if self.envelope_counter <= 0 {
                self.envelope_counter = self.envelope_period;
                if self.envelope_decay_level_counter > 0 {
                    self.envelope_decay_level_counter -= 1;
                } else if self.envelope_loop {
                    self.envelope_decay_level_counter = 15;
                }
            } else {
                self.envelope_counter = 0;
            }
        } else {
            self.envelope_start_flag = false;
            self.envelope_decay_level_counter = 15;
            self.envelope_counter = self.envelope_period;
        }
    }

    // $400C
    fn write_controls(&mut self, value: u8) {
        self.length_counter_halt = (value >> 5) & 1 == 1;
        self.envelope_loop = (value >> 5) & 1 == 1;
        self.use_constant_volume = (value >> 4) & 1 == 1;
        self.constant_volume = value & 0x0F;
        self.envelope_period = value & 0x0F;
    }

    // $400E
    fn write_mode_flag_and_timer_period(&mut self, value: u8) {
        self.mode_flag = (value >> 7) & 1 == 1;
        self.timer_period = NOISE_PERIOD_TABLE[(value & 0x0F) as usize];
    }

    // $400F
    fn write_length_counter(&mut self, value: u8) {
        self.length_counter = LENGTH_COUNTER_TABLE[((value & 0xF8) >> 3) as usize];
        self.envelope_start_flag = true;
    }

    fn output(&mut self) -> u16 {
        if !self.enabled {
            return 0;
        }
        if self.shift_register & 1 == 1 {
            return 0;
        }
        if self.length_counter == 0 {
            return 0;
        }
        if self.use_constant_volume {
            return self.constant_volume as u16;
        }
        return self.envelope_decay_level_counter as u16;
    }
}

struct DMC {
    enabled: bool,
    timer: u16,
    timer_period: u16,
    sample_address: u16,
    sample_length: u16,
    sample_address_counter: u16,
    sample_length_counter: u16,
    output: u8,
    irq_enabled: bool,
    loop_enabled: bool,
    shift_register: u8,
    bits_remaining_counter: u8,
    silence_flag: bool,
    sample_buffer: Option<u8>,
}

impl DMC {
    fn new() -> DMC {
        DMC{
            enabled: false,
            timer: 0,
            timer_period: 0,
            sample_address: 0,
            sample_length: 0,
            sample_address_counter: 0,
            sample_length_counter: 0,
            output: 0,
            irq_enabled: false,
            loop_enabled: false,
            shift_register: 0,
            bits_remaining_counter: 0,
            silence_flag: false,
            sample_buffer: Some(0),
        }
    }

    fn reset(&mut self) {
        self.sample_address_counter = self.sample_address;
        self.sample_length_counter = self.sample_length;
    }

    fn step_timer<T: cpu::Memory>(&mut self, cpu: &mut cpu::CPU<T>) {
        // TODO finish this and make sure it works.
        if !self.enabled {
            return
        }
        if self.sample_buffer == None && self.sample_length_counter > 0 {
            cpu.stall += 4;
            self.sample_buffer = Some(cpu.read(self.sample_address_counter));
            if self.sample_address_counter == 0xFFFF {
                self.sample_address_counter = 0x8000;
            } else {
                self.sample_address_counter = 0x0000;
            }
            self.sample_length_counter -= 1;
            if self.sample_address_counter == 0 && self.loop_enabled {
                self.reset();
            }
            // TODO handle otherwise, if the bytes remaining counter becomes zero and the IRQ enabled flag is set, the interrupt flag is set.
        }
        if self.timer == 0 {
            self.timer = self.timer_period;
            if !self.silence_flag {
                if self.shift_register&1 == 1 {
                    if self.output <= 125 {
                        self.output += 2;
                    }
                } else {
                    if self.output >= 2 {
                        self.output -= 2;
                    }
                }
            }
            self.shift_register >>= 1;
            self.bits_remaining_counter -= 1;
            if self.bits_remaining_counter <= 0 {
                self.bits_remaining_counter = 8;
                match self.sample_buffer{
                    Some(v) => {
                        self.silence_flag = false;
                        self.shift_register = v;
                        self.sample_buffer = None;
                    },
                    None => {
                        self.silence_flag = true;
                    }
                }
            }
        } else {
            self.timer -= 1; 
        }
    }

    // $4010
    fn write_controls(&mut self, value: u8) {
        self.irq_enabled = (value >> 7) & 1 == 1;
        self.loop_enabled = (value >> 6) & 1 == 1;
        self.timer_period = DMC_PERIOD_TABLE[(value & 0x0F) as usize];
    }

    // $4011
    fn write_output(&mut self, d: u8) {
        self.output = d & 0x7F;
    }

    // $4012
    fn write_sample_address(&mut self, a: u8) {
        self.sample_address = 0xC000 + (a as u16 * 64);
    }

    // $4013
    fn write_sample_length(&mut self, l: u8) {
        self.sample_length = (l as u16 * 16) + 1;
    }

    fn get_output(&mut self) -> u16 {
        return self.output as u16;
    }
}