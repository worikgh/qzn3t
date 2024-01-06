// peak_volume
#![feature(buf_read_has_data_left)]
//! Output the greatest amplitude in the passed file.  It is the
//! difference between the maximum and the minimum amplitude
//! Use: rustup override set nightly
use std::env;
use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;

fn main() {
    let mut args = env::args();

    // The file to process
    let in_fn: String = args.nth(1).expect("Pass a filename");
    let mut reader = match fs::File::open(in_fn.as_str()) {
        Ok(f) => BufReader::new(f),
        Err(err) => panic!("{err}: Cannot open {in_fn}"),
    };
    let mut buffer = [0_u8; 4];
    let mut max: f32 = f32::MIN;
    let mut min: f32 = f32::MAX;
    loop {
        match reader.read_exact(&mut buffer) {
            Ok(b) => b,
            Err(err) => panic!("{err}: Cannot get 4 bytes"),
        };
        let bits = u32::from_le_bytes(buffer);
        let f = f32::from_bits(bits);
        if f > max {
            max = f;
        } else if f < min {
            min = f;
        }
        if !reader.has_data_left().unwrap() {
            break;
        }
    }
    println!("{}", max - min);
}
