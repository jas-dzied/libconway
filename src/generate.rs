#![allow(dead_code)]
use std::fs;

use rand::Rng;

pub trait Generator {
    fn generate(self, config: &crate::life::Config) -> Vec<u32>;
}

pub struct RawData {
    positions: Vec<(u32, u32)>,
    x_offset: u32,
    y_offset: u32,
}

impl Generator for RawData {
    fn generate(self, config: &crate::life::Config) -> Vec<u32> {
        let mut data = vec![0; (config.width * config.height) as usize];
        for (x, y) in self.positions {
            data[((y + self.y_offset) * config.width + (x + self.x_offset)) as usize] = 1;
        }
        data
    }
}

pub struct Random(pub f64);

impl Generator for Random {
    fn generate(self, config: &crate::life::Config) -> Vec<u32> {
        let mut rng = rand::thread_rng();
        (0..(config.width * config.height))
            .map(|_| rng.gen_bool(self.0) as u32)
            .collect::<Vec<_>>()
    }
}

pub struct Plaintext {
    pub source: &'static str,
    pub x_offset: u32,
    pub y_offset: u32,
}

impl Generator for Plaintext {
    fn generate(self, config: &crate::life::Config) -> Vec<u32> {
        let mut data = vec![0; (config.width * config.height) as usize];
        let text = fs::read_to_string(self.source).unwrap();
        let lines = text.lines().filter(|x| !x.starts_with("!"));

        for (y, line) in lines.enumerate() {
            for (x, chr) in line.chars().enumerate() {
                let index = ((y as u32 + self.y_offset) * config.width + (x as u32 + self.x_offset))
                    as usize;
                data[index] = match chr {
                    '.' => 0,
                    'O' => 1,
                    _ => panic!("Bad char"),
                }
            }
        }

        data
    }
}

pub fn glider_gun() -> RawData {
    RawData {
        positions: vec![
            (24, 0),
            (22, 1),
            (24, 1),
            (12, 2),
            (13, 2),
            (20, 2),
            (21, 2),
            (34, 2),
            (35, 2),
            (11, 3),
            (15, 3),
            (20, 3),
            (21, 3),
            (34, 3),
            (35, 3),
            (0, 4),
            (1, 4),
            (10, 4),
            (16, 4),
            (20, 4),
            (21, 4),
            (0, 5),
            (1, 5),
            (10, 5),
            (14, 5),
            (16, 5),
            (17, 5),
            (22, 5),
            (24, 5),
            (10, 6),
            (16, 6),
            (24, 6),
            (11, 7),
            (15, 7),
            (12, 8),
            (13, 8),
        ],
        x_offset: 10,
        y_offset: 10,
    }
}
