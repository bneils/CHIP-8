mod vm;

extern crate sdl2;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::event::Event;
use sdl2::audio::{AudioCallback, AudioSpecDesired};

use std::fs::File;
use std::io::Read;
use std::thread;
use std::time::{Instant, Duration};
use vm::Env;

// The audio code below is stolen from the SDL rust example repo for square waves.
//https://github.com/Rust-SDL2/rust-sdl2/blob/master/examples/audio-squarewave.rs
struct SquareWave {
    phase_inc: f32,
    phase: f32,
    volume: f32,
}

impl AudioCallback for SquareWave {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        // Generate a square wave
        for x in out.iter_mut() {
            *x = if self.phase <= 0.5 {
                self.volume
            } else {
                -self.volume
            };
            self.phase = (self.phase + self.phase_inc) % 1.0;
        }
    }
}

fn main() {
    const WIDTH: u32 = 1024; // nearest multiple of 2
    const HEIGHT: u32 = WIDTH / 2;

    let sdl_context = sdl2::init()
        .expect("Couldn't initialize SDL2");

    let video_subsystem = sdl_context
        .video()
        .expect("Couldn't initialize the video subsystem");

    let audio_subsystem = sdl_context
        .audio()
        .expect("Couldn't initialize the audio subsystem");


    let desired_spec = AudioSpecDesired {
        freq: Some(44_100),
        channels: Some(1), // mono
        samples: None,     // default sample size
    };

    let device = audio_subsystem.open_playback(None, &desired_spec, |spec| {
        // Show obtained AudioSpec
        //println!("{:?}", spec);

        // initialize the audio callback
        SquareWave {
            phase_inc: 440.0 / spec.freq as f32,
            phase: 0.0,
            volume: 0.25,
        }
    }).unwrap();

    let window = video_subsystem
        .window("chip8", WIDTH, HEIGHT)
        //.resizable()
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

    let mut file = match File::open("/home/ben/Downloads/cavern.ch8") {
        Ok(file) => file,
        Err(err) => panic!("couldn't open file because {}", err),
    };

    let mut buf: Vec<u8> = Vec::new();
    file.read_to_end(&mut buf).unwrap();
    env.load_into_memory(&buf);

    let hz = 540;

    let mut iterations_this_nap = 0;
    const NUM_NAPS: u32 = 30;
    let mut nap_start = Instant::now();

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
        
        iterations_this_nap += 1;
        if iterations_this_nap >= hz / NUM_NAPS {
            iterations_this_nap = 0;

            if env.is_beeping() {   // it's practical to check it here.
                device.resume();
            } else {
                device.pause();
            }
            
            let dur = (1_000_000_000 / NUM_NAPS) as i32 - nap_start.elapsed().as_nanos() as i32;
            if dur > 0 {
                thread::sleep(Duration::new(0, dur as u32));
            }
            
            nap_start = Instant::now();
        }
    }
}
