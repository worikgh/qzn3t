// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0
#[allow(unused_imports)]
use qzn3t_engine::Engine;
use std::env;
#[allow(unused_imports)]
use std::path::PathBuf;

#[allow(unused_variables, unused_mut)]
fn main() {
    let mut engine = Engine::new().unwrap();
    engine
        .add_client(
            &["system:capture_1", "system:capture_2"],
            &["system:playback_1", "system:playback_2"],
        )
        .unwrap();
    let path: PathBuf = PathBuf::from(format!(
        "{}/recorded.raw",
        env::var("CARGO_MANIFEST_DIR").unwrap()
    ));
    engine.unpause();
    println!("<enter> to stop:");
    let mut buf = String::new();
    std::io::stdin().read_line(&mut buf).unwrap();
    println!("{buf}");
}
