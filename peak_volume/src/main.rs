// peak_volume
//! Output the greatest amplitude in the passed file.  It is the
//! difference between the maximum and the minimum amplitude
use std::env;
use std::fs;
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
            Ok(_) => {
                let bits = u32::from_le_bytes(buffer);
                let f = f32::from_bits(bits);
                if f > max {
                    max = f;
                }
                if f < min {
                    min = f;
                }
            }
            Err(err) => {
                // Check if error is due to EOF
                if err.kind() == std::io::ErrorKind::UnexpectedEof {
                    break;
                }
                panic!("{err}: Cannot read 4 bytes from {in_fn}");
            }
        };
    }
    println!("{}", max - min);
}
