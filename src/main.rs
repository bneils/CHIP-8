mod vm;

extern crate sdl2;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::event::Event;

use std::fs::File;
use std::io::Read;
use std::thread;
use std::time::Duration;
use vm::Env;

fn main() {
    const WIDTH: u32 = 1024; // nearest multiple of 2
    const HEIGHT: u32 = WIDTH / 2;

    let sdl_context = sdl2::init()
        .expect("Couldn't initialize SDL2");

    let video_subsystem = sdl_context
        .video()
        .expect("Couldn't initialize the video subsystem");

    let window = video_subsystem
        .window("chip8", WIDTH, HEIGHT)
        .resizable()
        .build()
        .unwrap();

    let mut event_pump = sdl_context
        .event_pump()
        .expect("Couldn't initialize the event pump. Perhaps one already exists?");

    let mut canvas = window
        .into_canvas()
        .build()
        .expect("Couldn't initialize the canvas");

    let mut env = Env::new();

    let mut file = match File::open("/home/ben/Downloads/ibm.ch8") {
        Ok(file) => file,
        Err(err) => panic!("couldn't open file because {}", err),
    };

    let mut buf: Vec<u8> = Vec::new();
    file.read_to_end(&mut buf).unwrap();
    env.load_into_memory(&buf);

    let hz = 540;

    'main: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'main,
                _ => {},
            }
        }

        env.read_instr(&mut event_pump);
        if env.display_changed {
            for y in 0..32 {
                for x in 0..64 {
                    let bit = (env.display[y] >> (64 - (x + 1))) as u8 & 1;
                    canvas.set_draw_color(Color::RGB(0, 255 * bit, 0));
                    let w = WIDTH / 64;
                    canvas.fill_rect(Rect::new(
                        x as i32 * w as i32, 
                        y as i32 * w as i32, 
                        w, w)).unwrap();
                }
            }
            canvas.present();
        }
        
        // Vf is changed if any pixels were set from 1 to 0.
        //if env.variable_registers[0xf] != 0 {  
        //}

        thread::sleep(Duration::new(0, 1_000_000_000u32 / hz));
    }
}
