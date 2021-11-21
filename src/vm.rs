
mod opcode {
    pub fn matcher(code: u16) {
        
    }
}

pub struct Env {
    memory: [u8; 4096],
    display: [u64; 32], // 64x32 (updated @60hz) (idea: fade effect)
    program_counter: usize,
    index_register: usize, // 16-bit, ref as "I"
    stack: [u16; 16],
    delay_timer: u8, // delay timer @60hz
    sound_timer: u8, // beeps while not 0
    variable_registers: [u8; 16], // v0-f (vf may be flag register)
    font: [u8; 5 * 16], // 5 cols, 16 rows

    current_instr: (u8, u8, u8, u8), // 4 nibbles
}

mod nibble {
    pub fn unpack(a: u8, b: u8) -> (u8, u8, u8, u8) {
        (a & 0b11110000, a & 0b00001111, 
            b & 0b11110000, b & 0b00001111)
    }
    
    pub fn pack(a: u8, b: u8, c: u8, d: u8) -> u16 {
        (a << 12) | (b << 8) | (c << 4) | (d << 0)
    }    
}

impl Env {
    pub fn new() -> Env {
        Env {
            memory: [0; 4096],
            display: [0; 32],
            program_counter: 0,
            index_register: 0,
            stack: [0; 16],
            delay_timer: 0,
            sound_timer: 0,
            variable_registers: [0; 16],
            current_instr: (0, 0, 0, 0),
            font: [
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
            ],
        }
    }

    pub fn load_into_memory(&mut self) {
        
    }

    fn display_clear(&mut self) {

    }

    fn subroutine_return(&mut self) {

    }

    fn goto(&mut self) {

    }

    fn call_subroutine(&mut self) {

    }

    fn skip_if_eq(&mut self) {

    }

    pub fn read_instr(&mut self) {
        let (n1, n2, n3, n4) = nibble::unpack(
            self.memory[self.program_counter], 
            self.memory[self.program_counter + 1]
        );
        self.current_instr = (n1, n2, n3, n4);

        const ERROR_MESSAGE: &str = "unrecognized instruction";

        //https://en.wikipedia.org/wiki/CHIP-8
        match n1 {
            0 if n2 == 0 && n3 == 0xE => match n4 {
                0 => self.display_clear(),
                0xE => self.subroutine_return(),
                _ => {},
            },
            1 => self.goto(),
            2 => self.call_subroutine(),
            3 => self.skip_if_register_equal_value(),
            4 => self.skip_if_register_not_equal_value(),
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
                _ => {},
            },
            9 if n4 == 0 => self.skip_if_registers_not_equal(),
            0xA => self.set_index_register(),
            0xB => self.goto_register_zero_plus_value(),
            0xC => self.set_register_rand_and_value(),
            0xD => self.draw_sprite(),
            0xE => match n3 {
                9 if n4 == 0xE => self.skip_if_key_pressed_equals_register(),
                0xA if n4 == 1 => self.skip_if_key_pressed_not_equals_register(),
                _ => {},
            },
            0xF => match n3 {
                0 => match n4 {
                    7 => self.,
                    0xA => ,
                    _ => {},
                },
                1 => match n4 {
                    5 => ,
                    8 => ,
                    0xE => ,
                    _ => {},
                },
                2 if n4 == 9 => ,
                3 if n4 == 3 => ,
                5 if n4 == 5 => ,
                6 if n4 == 5 => ,
            },
            _ => {},
        };
        self.program_counter += 2;
    }
}