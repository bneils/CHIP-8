use rand::Rng;
use std::time::{Instant};

use sdl2::EventPump;
use sdl2::keyboard::Scancode;
use sdl2::event::Event;

pub struct Env {
    pub display_changed: bool,
    pub fading_pixels: [u64; 32],
    pub display: [u64; 32], // 64x32 (updated @60hz) (idea: fade effect)
    
    program_counter: u16,
    current_instr: (u8, u8, u8, u8), // 4 nibbles
    
    stack: [u16; 16], // holds the PCs for all subroutines nested above the current one in descending order.
    stack_next_pos: u8,
    
    delay_timer: u8, // delay timer @60hz
    sound_timer: u8, // beeps while not 0
    last_timer_tick: Instant,

    memory: [u8; 4096],
    index_register: u16, // 16-bit, ref as "I"
    pub variable_registers: [u8; 16], // v0-f (vf may be flag register)
}

mod nibble {
    #[inline]
    pub fn unpack(a: u8, b: u8) -> (u8, u8, u8, u8) {
        ((a & 0b11110000) >> 4, a & 0b00001111, 
            (b & 0b11110000) >> 4, b & 0b00001111)
    }
    
    #[inline]
    pub fn pack(a: u8, b: u8, c: u8, d: u8) -> u16 {
        ((a as u16) << 12) | ((b as u16) << 8) | ((c as u16) << 4) | (d as u16)
    }    
}

/*
First 512 bytes (0-1ff) were meant to be for the interpreter.
Programs are located at addr 0x200.
Fonts are 4x5 and are popularly located at addr 0x50-0x9F
*/

impl Env {
    const FONT_START_LOCATION: usize = 0x50;
    const PROGRAM_START_LOCATION: usize = 0x200;

    pub fn new() -> Env {
        let mut env = Env {
            memory: [0; 4096],
            display: [0; 32],
            display_changed: false,
            program_counter: 0x200,
            index_register: 0,
            stack: [0; 16],
            delay_timer: 0,
            sound_timer: 0,
            last_timer_tick: Instant::now(),
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

    // Copies the ROM into emulator memory.
    // Changes the program counter to prepare for execution.
    pub fn load_into_memory(&mut self, rom: &Vec<u8>) {
        self.program_counter = Env::PROGRAM_START_LOCATION as u16;
        for i in 0..rom.len() {
            self.memory[Env::PROGRAM_START_LOCATION + i] = rom[i];
        }
    }

    #[inline]
    fn display_clear(&mut self) {
        self.display = [0; 32];
    }

    #[inline]
    fn subroutine_return(&mut self) {
        if self.stack_next_pos > 0 {
            self.stack_next_pos -= 1;
            self.program_counter = self.stack[self.stack_next_pos as usize];
        } else {
            panic!("Cannot RETURN with no outer subroutine to return to"); // This could just exit gracefully...
        }
    }

    #[inline]
    fn goto(&mut self) {
        let (_, b, c, d) = self.current_instr;
        // You subtract 2 because the interpreter will step forward 2
        self.program_counter = nibble::pack(0, b, c, d) - 2;
    }

    #[inline]
    fn call_subroutine(&mut self) {
        if (self.stack_next_pos as usize) < self.stack.len() {
            let (_, b, c, d) = self.current_instr;
            self.stack[self.stack_next_pos as usize] = self.program_counter;
            self.stack_next_pos += 1;
            self.program_counter = nibble::pack(0, b, c, d);
        } else {
            panic!("Maximum levels of recursion exceeded ({})", self.stack.len());
        }
    }

    #[inline]
    fn skip_if_register_equals_value(&mut self) {
        let (_, x, a, b) = self.current_instr;
        if self.variable_registers[x as usize] == 
            nibble::pack(0, 0, a, b) as u8 {
            self.program_counter += 2;
        }
    }

    #[inline]
    fn skip_if_register_not_equals_value(&mut self) {
        let (_, x, a, b) = self.current_instr;
        if self.variable_registers[x as usize] != 
            nibble::pack(0, 0, a, b) as u8 {
            self.program_counter += 2;
        }
    }

    #[inline]
    fn skip_if_registers_equal(&mut self) {
        let (_, x, y, _) = self.current_instr;
        if self.variable_registers[x as usize] ==
            self.variable_registers[y as usize] {
            self.program_counter += 2;
        }
    }

    #[inline]
    fn register_set_value(&mut self) {
        let (_, x, a, b) = self.current_instr;
        self.variable_registers[x as usize] = nibble::pack(0, 0, a, b) as u8;
    }

    #[inline]
    fn register_add_value(&mut self) {
        let (_, x, a, b) = self.current_instr;
        self.variable_registers[x as usize] += nibble::pack(0, 0, a, b) as u8;
    }

    #[inline]
    fn register_set_register(&mut self) {
        let (_, x, y, _) = self.current_instr;
        self.variable_registers[x as usize] = self.variable_registers[y as usize];
    }

    #[inline]
    fn register_or_register(&mut self) {
        let (_, x, y, _) = self.current_instr;
        self.variable_registers[x as usize] |= self.variable_registers[y as usize];
    }

    #[inline]
    fn register_and_register(&mut self) {
        let (_, x, y, _) = self.current_instr;
        self.variable_registers[x as usize] &= self.variable_registers[y as usize];
    }

    #[inline]
    fn register_xor_register(&mut self) {
        let (_, x, y, _) = self.current_instr;
        self.variable_registers[x as usize] ^= self.variable_registers[y as usize];
    }

    #[inline]
    fn register_add_register(&mut self) {
        let (_, x, y, _) = self.current_instr;
        let sum = self.variable_registers[x as usize].overflowing_add(self.variable_registers[y as usize]);
        self.variable_registers[x as usize] = sum.0;
        self.variable_registers[15] = sum.1 as u8;
    }

    #[inline]
    fn register_sub_register(&mut self) {
        let (_, x, y, _) = self.current_instr;
        let diff = self.variable_registers[x as usize].overflowing_sub(self.variable_registers[y as usize]);
        self.variable_registers[x as usize] = diff.0;
        self.variable_registers[15] = (!diff.1) as u8;
    }

    #[inline]
    fn register_right_shift(&mut self) {
        let (_, x, _, _) = self.current_instr;
        self.variable_registers[15] = self.variable_registers[x as usize] & 1;
        self.variable_registers[x as usize] >>= 1;
    }

    #[inline]
    fn register_set_register_sub_register(&mut self) {
        let (_, x, y, _) = self.current_instr;
        let diff = self.variable_registers[y as usize].overflowing_sub(self.variable_registers[x as usize]);
        self.variable_registers[x as usize] = diff.0;
        self.variable_registers[15] = (!diff.1) as u8;
    }

    #[inline]
    fn register_left_shift(&mut self) {
        let x = self.current_instr.1;
        self.variable_registers[15] = (self.variable_registers[x as usize] >> 7) & 1;
        self.variable_registers[x as usize] <<= 1;
    }

    #[inline]
    fn skip_if_registers_not_equal(&mut self) {
        let (_, x, y, _) = self.current_instr;
        if self.variable_registers[x as usize] != 
            self.variable_registers[y as usize] {
            self.program_counter += 2;
        }
    }

    #[inline]
    fn set_index_register(&mut self) {
        let (_, a, b, c) = self.current_instr;
        self.index_register = nibble::pack(0, a, b, c);
    }

    #[inline]
    fn goto_register_zero_plus_value(&mut self) {
        let (_, a, b, c) = self.current_instr;
        self.program_counter = 
            self.variable_registers[0] as u16 + nibble::pack(0, a, b, c);
    }

    #[inline]
    fn set_register_rand_and_value(&mut self) {
        let (_, x, a, b) = self.current_instr;
        let n: u8 = rand::thread_rng().gen();
        self.variable_registers[x as usize] = n & nibble::pack(0, 0, a, b) as u8;
    }

    #[inline]
    fn draw_sprite(&mut self) {
        let (_, x, y, h) = self.current_instr;
        let (x, y) = (self.variable_registers[x as usize], self.variable_registers[y as usize]);
        // this footprint tells if a certain column anywhere in the rows was flipped from 1 to 0.
        let mut pixel_set_to_zero = false;
        for i in 0..h {
            let row = (
                // First, this accesses 8 pixels starting at I, and then shifts it the distance
                // from the 8th pixel the 64th pixel. You then shift it right x times.
                (self.memory[self.index_register as usize + i as usize] as u64) << (64 - 8)
            ) >> x;
            let y = (y + i) as usize;
            let fading_row = self.display[y] & row;
            self.fading_pixels[y] = fading_row;
            if fading_row != 0 {
                pixel_set_to_zero = true;
            }
            self.display[y] ^= row;
        }

        // if a pixel was switched to OFF, anywhere
        self.display_changed = true;
        self.variable_registers[15] = pixel_set_to_zero as u8;
    }

    #[inline]
    fn block_until_hex_key(&self, event_pump: &mut EventPump) -> u8 {
        loop {
            for event in event_pump.poll_iter() {
                match event {
                    Event::Quit { .. } => panic!(),
                    Event::KeyDown { scancode, .. } => match scancode {
                        Some(scancode) => match scancode {
                            Scancode::Num1 => return 1,
                            Scancode::Num2 => return 2,
                            Scancode::Num3 => return 3,
                            Scancode::Q => return 4,
                            Scancode::W => return 5,
                            Scancode::E => return 6,
                            Scancode::A => return 7,
                            Scancode::S => return 8,
                            Scancode::D => return 9,
                            Scancode::Z => return 0xA,
                            Scancode::X => return 0,
                            Scancode::C => return 0xB,
                            Scancode::Num4 => return 0xC,
                            Scancode::R => return 0xD,
                            Scancode::F => return 0xE,
                            Scancode::V => return 0xF,
                            _ => {},
                        },
                        None => {},
                    },
                    _ => {},
                }
            }
        }
    }

    #[inline]
    fn get_hex_key_state(&self, key: u8, event_pump: &EventPump) -> bool {
        /*
            Returns if it is pressed or not.

            (scancodes are used, below is QWERTY)
            1 2 3 4 is the mapping of 1 2 3 C
            Q W E R                   4 5 6 D
            A S D F                   7 8 9 E
            Z X C V                   A 0 B F
        */

        let code = match key {
            1 => Scancode::Num1,
            2 => Scancode::Num2,
            3 => Scancode::Num3,
            4 => Scancode::Q,
            5 => Scancode::W,
            6 => Scancode::E,
            7 => Scancode::A,
            8 => Scancode::S,
            9 => Scancode::D,
            0xA => Scancode::Z,
            0 => Scancode::X,
            0xB => Scancode::C,
            0xC => Scancode::Num4,
            0xD => Scancode::R,
            0xE => Scancode::F,
            0xF => Scancode::V,
            _ => return false,
        };
        
        event_pump.keyboard_state().is_scancode_pressed(code)
    }

    #[inline]
    fn skip_if_key_pressed_equals_register(&mut self, pump: &mut EventPump) {
        if self.get_hex_key_state(self.variable_registers[self.current_instr.1 as usize], pump) {
            self.program_counter += 2;
        }
    }

    #[inline]
    fn skip_if_key_pressed_not_equals_register(&mut self, pump: &mut EventPump) {
        if !self.get_hex_key_state(self.variable_registers[self.current_instr.1 as usize], pump) {
            self.program_counter += 2;
        }
    }

    #[inline]
    fn set_register_to_delay_timer(&mut self) {
        self.variable_registers[self.current_instr.1 as usize] = self.delay_timer;
    }

    #[inline]
    fn set_register_to_blocking_key(&mut self, pump: &mut EventPump) {
        self.variable_registers[self.current_instr.1 as usize] = self.block_until_hex_key(pump);
    }

    #[inline]
    fn set_delay_timer_to_register(&mut self) {
        self.delay_timer = self.variable_registers[self.current_instr.1 as usize];
    }

    #[inline]
    fn set_sound_timer_to_register(&mut self) {
        self.sound_timer = self.variable_registers[self.current_instr.1 as usize];
    }

    #[inline]
    fn add_register_to_index_register(&mut self) {
        self.index_register += self.variable_registers[self.current_instr.1 as usize] as u16;
    }

    #[inline]
    fn set_index_register_to_sprite_location_of_register(&mut self) {
        // Each sprite is 5 bytes wide, so to find its addr, you count in increments of 5
        self.index_register = Env::FONT_START_LOCATION as u16 + 
            self.variable_registers[self.current_instr.1 as usize] as u16 * 5;
    }

    #[inline]
    fn bcd_of_register_in_index_register(&mut self) {
        let v = self.variable_registers[self.current_instr.1 as usize];
        self.memory[self.index_register as usize] = v / 100;
        self.memory[self.index_register as usize + 1] = v % 100 / 10;
        self.memory[self.index_register as usize + 2] = v % 10;
    }

    #[inline]
    fn store_registers_up_to_in_memory(&mut self) {
        for i in 0..=self.current_instr.1 {
            self.memory[self.index_register as usize + i as usize] = 
                self.variable_registers[i as usize];
        }
    }

    #[inline]
    fn loads_registers_up_to_in_memory(&mut self) {
        for i in 0..=self.current_instr.1 {
            self.variable_registers[i as usize] =
                self.memory[self.index_register as usize + i as usize];
        }
    }

    pub fn read_instr(&mut self, pump: &mut EventPump) {
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
