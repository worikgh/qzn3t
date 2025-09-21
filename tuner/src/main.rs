// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0
use clap::Parser;

use tuner::TunerArgs;
use tuner::inner_main;
fn main() {
    let args = TunerArgs::parse();
    inner_main(&args);
}
