// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0
use qzn3t_engine::Engine;
use std::path::PathBuf;

fn main() {
    let mut engine = Engine::new();
    let path: PathBuf = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/recorded.raw"));
    engine.start_recording(3, &path).unwrap();
    println!("<enter> to stop:");
    let mut buf = String::new();
    std::io::stdin().read_line(&mut buf).unwrap();
    println!("{buf}");
}
