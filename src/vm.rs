use rand::Rng;
use std::time::{Duration, Instant};
use std::thread;

use sdl2::EventPump;
use sdl2::keyboard::Scancode;
use sdl2::event::Event;

pub struct Env {
    memory: [u8; 4096],
    pub display_changed: bool,
    pub display: [u64; 32], // 64x32 (updated @60hz) (idea: fade effect)
    program_counter: u16,
    index_register: u16, // 16-bit, ref as "I"
    
    stack: [u16; 16], // holds the PCs for all subroutines nested above the current one in descending order.
    stack_next_pos: u8,
    
    delay_timer: u8, // delay timer @60hz
    sound_timer: u8, // beeps while not 0
    last_timer_tick: Instant,

    pub variable_registers: [u8; 16], // v0-f (vf may be flag register)
    skip_flag: bool, // flag to indicate if this instruction is to be skipped
    current_instr: (u8, u8, u8, u8), // 4 nibbles

    pub fading_pixels: [u64; 32],
}

mod nibble {
    pub fn unpack(a: u8, b: u8) -> (u8, u8, u8, u8) {
        ((a & 0b11110000) >> 4, a & 0b00001111, 
            (b & 0b11110000) >> 4, b & 0b00001111)
    }
    
    pub fn pack(a: u8, b: u8, c: u8, d: u8) -> u16 {
        ((a as u16) << 12) | ((b as u16) << 8) | ((c as u16) << 4) | (d as u16)
    }    
}

/*
First 512 bytes (0-1ff) were meant to be for the interpreter.
Programs are located at addr 0x200.
Fonts are 4x5 and are popularly located at addr 0x50-0x9F

(scancodes are used, below is QWERTY)
1 2 3 4 is the mapping of 1 2 3 C
Q W E R                   4 5 6 D
A S D F                   7 8 9 E
Z X C V                   A 0 B F
*/

impl Env {
    const FONT_START_LOCATION: usize = 0x50;
    const PROGRAM_START_LOCATION: usize = 0x200;

    pub fn new() -> Env {
        let mut env = Env {
            memory: [0; 4096],
            display: [0; 32],
            display_changed: false,
            program_counter: 0,
            index_register: 0,
            stack: [0; 16],
            delay_timer: 0,
            sound_timer: 0,
            last_timer_tick: Instant::now(),
            skip_flag: false,
            stack_next_pos: 0,
            variable_registers: [0; 16],
            current_instr: (0, 0, 0, 0), // tuple of nibbles
            fading_pixels: [0; 32],
        };

        // 5 cols, 16 rows
        const FONTS: [u8; 5 * 16] = [
            0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
            0x20, 0x60, 0x20, 0x20, 0x70, // 1
            0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
            0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
            0x90, 0x90, 0xF0, 0x10, 0x10, // 4
            0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
            0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
            0xF0, 0x10, 0x20, 0x40, 0x40, // 7
            0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
            0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
            0xF0, 0x90, 0xF0, 0x90, 0x90, // A
            0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
            0xF0, 0x80, 0x80, 0x80, 0xF0, // C
            0xE0, 0x90, 0x90, 0x90, 0xE0, // D
            0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
            0xF0, 0x80, 0xF0, 0x80, 0x80  // F
        ];

        for i in 0..FONTS.len() {
            env.memory[Env::FONT_START_LOCATION + i] = FONTS[i];
        }

        env
    }

    pub fn is_beeping(&self) -> bool {
        self.sound_timer > 0
    }

    pub fn load_into_memory(&mut self, rom: &Vec<u8>) {
        self.program_counter = Env::PROGRAM_START_LOCATION as u16;
        for i in 0..rom.len() {
            self.memory[Env::PROGRAM_START_LOCATION + i] = rom[i];
        }
    }

    fn display_clear(&mut self) {
        self.display = [0; 32];
    }

    fn subroutine_return(&mut self) {
        if self.stack_next_pos > 0 {
            self.stack_next_pos -= 1;
            self.program_counter = self.stack[self.stack_next_pos as usize];
        } else {
            panic!("Cannot RETURN with no outer subroutine to return to"); // This could just exit gracefully...
        }
    }

    fn goto(&mut self) {
        let (_, b, c, d) = self.current_instr;
        self.program_counter = nibble::pack(0, b, c, d) - 2;
    }

    fn call_subroutine(&mut self) {
        if (self.stack_next_pos as usize) < self.stack.len() {
            let (_, b, c, d) = self.current_instr;
            self.stack[self.stack_next_pos as usize] = self.program_counter;
            self.stack_next_pos += 1;
            self.program_counter = nibble::pack(0, b, c, d);
            println!("{} CALL {}", self.program_counter, nibble::pack(0, b, c, d));
        } else {
            panic!("Maximum levels of recursion exceeded ({})", self.stack.len());
        }
    }

    fn skip_if_register_equals_value(&mut self) {
        let (_, x, a, b) = self.current_instr;
        self.skip_flag = 
            self.variable_registers[x as usize] == 
                nibble::pack(0, 0, a, b) as u8;
    }

    fn skip_if_register_not_equals_value(&mut self) {
        let (_, x, a, b) = self.current_instr;
        self.skip_flag = 
            self.variable_registers[x as usize] != 
            nibble::pack(0, 0, a, b) as u8;
    }

    fn skip_if_registers_equal(&mut self) {
        let (_, x, y, _) = self.current_instr;
        self.skip_flag =
            self.variable_registers[x as usize] ==
            self.variable_registers[y as usize];
    }

    fn register_set_value(&mut self) {
        let (_, x, a, b) = self.current_instr;
        self.variable_registers[x as usize] = nibble::pack(0, 0, a, b) as u8;
    }

    fn register_add_value(&mut self) {
        let (_, x, a, b) = self.current_instr;
        self.variable_registers[x as usize] += nibble::pack(0, 0, a, b) as u8;
    }

    fn register_set_register(&mut self) {
        let (_, x, y, _) = self.current_instr;
        self.variable_registers[x as usize] = self.variable_registers[y as usize];
    }

    fn register_or_register(&mut self) {
        let (_, x, y, _) = self.current_instr;
        self.variable_registers[x as usize] |= self.variable_registers[y as usize];
    }

    fn register_and_register(&mut self) {
        let (_, x, y, _) = self.current_instr;
        self.variable_registers[x as usize] &= self.variable_registers[y as usize];
    }

    fn register_xor_register(&mut self) {
        let (_, x, y, _) = self.current_instr;
        self.variable_registers[x as usize] ^= self.variable_registers[y as usize];
    }

    fn register_add_register(&mut self) {
        let (_, x, y, _) = self.current_instr;
        let sum = self.variable_registers[x as usize].overflowing_add(self.variable_registers[y as usize]);
        self.variable_registers[x as usize] = sum.0;
        self.variable_registers[15] = sum.1 as u8;
    }

    fn register_sub_register(&mut self) {
        let (_, x, y, _) = self.current_instr;
        let diff = self.variable_registers[x as usize].overflowing_sub(self.variable_registers[y as usize]);
        self.variable_registers[x as usize] = diff.0;
        self.variable_registers[15] = (!diff.1) as u8;
    }

    fn register_right_shift(&mut self) {
        let (_, x, _, _) = self.current_instr;
        self.variable_registers[15] = self.variable_registers[x as usize] & 1;
        self.variable_registers[x as usize] >>= 1;
    }

    fn register_set_register_sub_register(&mut self) {
        let (_, x, y, _) = self.current_instr;
        let diff = self.variable_registers[y as usize].overflowing_sub(self.variable_registers[x as usize]);
        self.variable_registers[x as usize] = diff.0;
        self.variable_registers[15] = (!diff.1) as u8;
    }

    fn register_left_shift(&mut self) {
        let x = self.current_instr.1;
        self.variable_registers[15] = (self.variable_registers[x as usize] >> 7) & 1;
        self.variable_registers[x as usize] <<= 1;
    }

    fn skip_if_registers_not_equal(&mut self) {
        let (_, x, y, _) = self.current_instr;
        self.skip_flag = self.variable_registers[x as usize] != self.variable_registers[y as usize];
    }

    fn set_index_register(&mut self) {
        let (_, a, b, c) = self.current_instr;
        self.index_register = nibble::pack(0, a, b, c);
    }

    fn goto_register_zero_plus_value(&mut self) {
        let (_, a, b, c) = self.current_instr;
        self.program_counter = 
            self.variable_registers[0] as u16 + nibble::pack(0, a, b, c);
    }

    fn set_register_rand_and_value(&mut self) {
        let (_, x, a, b) = self.current_instr;
        let n: u8 = rand::thread_rng().gen();
        self.variable_registers[x as usize] = n & nibble::pack(0, 0, a, b) as u8;
    }

    fn draw_sprite(&mut self) {
        let (_, x, y, h) = self.current_instr;
        let (x, y) = (self.variable_registers[x as usize], self.variable_registers[y as usize]);
        // this footprint tells if a certain column anywhere in the rows was flipped from 1 to 0.
        let mut on_to_off_footprint: u64 = 0;
        for i in 0..h {
            let row = (
                // First, this accesses 8 pixels starting at I, and then shifts it the distance
                // from the 8th pixel the 64th pixel. You then shift it right x times.
                (self.memory[self.index_register as usize + i as usize] as u64) << (64 - 8)
            ) >> x;
            let y = (y + i) as usize;
            let fading_row = self.display[y] & row;
            self.fading_pixels[y] = fading_row;
            on_to_off_footprint |= fading_row;
            self.display[y] ^= row;
            self.display_changed = true;
        }

        // if a pixel was switched to OFF, anywhere
        self.variable_registers[15] = (on_to_off_footprint != 0) as u8;
    }

    fn get_hex_press(&self, event_pump: &mut EventPump) -> u8 {
        loop {
            for event in event_pump.poll_iter() {
                match event {
                    Event::Quit { .. } => { panic!() },
                    Event::KeyDown { scancode, .. } => {
                        if let Some(code) = scancode {
                            // I dunno if there's a better way of doing this...
                            match code {
                                Scancode::Num0 => return 0,
                                Scancode::Num1 => return 1,
                                Scancode::Num2 => return 2,
                                Scancode::Num3 => return 3,
                                Scancode::Num4 => return 4,
                                Scancode::Num5 => return 5,
                                Scancode::Num6 => return 6,
                                Scancode::Num7 => return 7,
                                Scancode::Num8 => return 8,
                                Scancode::Num9 => return 9,
                                Scancode::A => return 10,
                                Scancode::B => return 11,
                                Scancode::C => return 12,
                                Scancode::D => return 13,
                                Scancode::E => return 14,
                                Scancode::F => return 15,
                                _ => {},
                            };
                        }
                    },
                    _ => {},
                }
            }
            thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));  //60hz is all that's necessary
        }
    }

    fn skip_if_key_pressed_equals_register(&mut self, pump: &mut EventPump) {
        self.skip_flag = 
            self.variable_registers[self.current_instr.1 as usize] == self.get_hex_press(pump);
    }

    fn skip_if_key_pressed_not_equals_register(&mut self, pump: &mut EventPump) {
        self.skip_flag =
            self.variable_registers[self.current_instr.1 as usize] != self.get_hex_press(pump);
    }

    fn set_register_to_delay_timer(&mut self) {
        self.variable_registers[self.current_instr.1 as usize] = self.delay_timer;
    }

    fn set_register_to_blocking_key(&mut self, pump: &mut EventPump) {
        self.variable_registers[self.current_instr.1 as usize] = self.get_hex_press(pump);
    }

    fn set_delay_timer_to_register(&mut self) {
        self.delay_timer = self.variable_registers[self.current_instr.1 as usize];
    }

    fn set_sound_timer_to_register(&mut self) {
        self.sound_timer = self.variable_registers[self.current_instr.1 as usize];
    }

    fn add_register_to_index_register(&mut self) {
        self.index_register += self.variable_registers[self.current_instr.1 as usize] as u16;
    }

    fn set_index_register_to_sprite_location_of_register(&mut self) {
        // Each sprite is 5 bytes wide, so to find its addr, you count in increments of 5
        self.index_register = Env::FONT_START_LOCATION as u16 + 
            self.variable_registers[self.current_instr.1 as usize] as u16 * 5;
    }

    fn bcd_of_register_in_index_register(&mut self) {
        let v = self.variable_registers[self.current_instr.1 as usize];
        self.memory[self.index_register as usize] = v / 100;
        self.memory[self.index_register as usize + 1] = v % 100 / 10;
        self.memory[self.index_register as usize + 2] = v % 10;
    }

    fn store_registers_up_to_in_memory(&mut self) {
        for i in 0..=self.current_instr.1 {
            self.memory[self.index_register as usize + i as usize] = 
                self.variable_registers[i as usize];
        }
    }

    fn loads_registers_up_to_in_memory(&mut self) {
        for i in 0..=self.current_instr.1 {
            self.variable_registers[i as usize] =
                self.memory[self.index_register as usize + i as usize];
        }
    }

    pub fn read_instr(&mut self, pump: &mut EventPump) {
        if self.skip_flag {
            self.skip_flag = false;
            self.program_counter += 2; // skip the current
        }

        // We can update the s/d timers here b/c the clock hz is >60
        let elapsed = self.last_timer_tick.elapsed().as_nanos();
        if elapsed >= 1_000_000_000 / 60 {
            if self.delay_timer > 0 {
                self.delay_timer -= 1;
            }
            if self.sound_timer > 0 {
                self.sound_timer -= 1;
            }
            self.last_timer_tick = Instant::now();
        }

        self.current_instr = nibble::unpack(
            self.memory[self.program_counter as usize], 
            self.memory[self.program_counter as usize + 1]
        );

        let (n1, n2, n3, n4) = self.current_instr;

        let mut unrecognized_flag = false;

        //https://en.wikipedia.org/wiki/CHIP-8
        match n1 {
            0 if n2 == 0 && n3 == 0xE => match n4 {
                0 => self.display_clear(),
                0xE => self.subroutine_return(),
                _ => unrecognized_flag = true,
            },
            1 => self.goto(),
            2 => self.call_subroutine(),
            3 => self.skip_if_register_equals_value(),
            4 => self.skip_if_register_not_equals_value(),
            5 if n4 == 0 => self.skip_if_registers_equal(),
            6 => self.register_set_value(),
            7 => self.register_add_value(),
            8 => match n4 {
                0 => self.register_set_register(),
                1 => self.register_or_register(),
                2 => self.register_and_register(),
                3 => self.register_xor_register(),
                4 => self.register_add_register(),
                5 => self.register_sub_register(),
                6 => self.register_right_shift(),
                7 => self.register_set_register_sub_register(),
                0xE => self.register_left_shift(),
                _ => unrecognized_flag = true,
            },
            9 if n4 == 0 => self.skip_if_registers_not_equal(),
            0xA => self.set_index_register(),
            0xB => self.goto_register_zero_plus_value(),
            0xC => self.set_register_rand_and_value(),
            0xD => self.draw_sprite(),
            0xE => match n3 {
                9 if n4 == 0xE => self.skip_if_key_pressed_equals_register(pump),
                0xA if n4 == 1 => self.skip_if_key_pressed_not_equals_register(pump),
                _ => unrecognized_flag = true,
            },
            0xF => match n3 {
                0 => match n4 {
                    7 => self.set_register_to_delay_timer(),
                    0xA => self.set_register_to_blocking_key(pump),
                    _ => unrecognized_flag = true,
                },
                1 => match n4 {
                    5 => self.set_delay_timer_to_register(),
                    8 => self.set_sound_timer_to_register(),
                    0xE => self.add_register_to_index_register(),
                    _ => unrecognized_flag = true,
                },
                2 if n4 == 9 => self.set_index_register_to_sprite_location_of_register(),
                3 if n4 == 3 => self.bcd_of_register_in_index_register(),
                5 if n4 == 5 => self.store_registers_up_to_in_memory(),
                6 if n4 == 5 => self.loads_registers_up_to_in_memory(),
                _ => unrecognized_flag = true,
            },
            _ => unrecognized_flag = true,
        };
        if unrecognized_flag {
            panic!("Unrecognized opcode ({:?}) at {}", self.current_instr, self.program_counter);
        }

        self.program_counter += 2; // each instr is 2 bytes
    }
}
