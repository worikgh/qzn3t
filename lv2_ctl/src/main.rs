///App code taken from https://ratatui.rs/tutorials/counter-app/
use app::App;
use std::collections::HashSet;

use lv2::get_lv2_controller;
use lv2::Lv2Type;
use std::fs::OpenOptions;
use std::io;
use std::io::Lines;
use std::io::StdinLock;
use std::io::Write;

use crate::lv2::ModHostController;
mod app;
mod lv2;
mod run_executable;
fn main() -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .open("/tmp/output.txt")
        .expect("Failed to open file");

    file.write_all(b"Debug messages from main.rs\n")?;

    // Store processed plugins so only get procerssed once
    let lines: Lines<StdinLock> = io::stdin().lines();
    let mod_host_controller: ModHostController = get_lv2_controller(lines)?;

    // Do something with the mod_host_controller
    let mut types: HashSet<Lv2Type> = HashSet::new();
    for lv2 in mod_host_controller.simulators.iter() {
        for t in lv2.types.iter() {
            let lv2_type: Lv2Type = t.clone();
            types.insert(lv2_type);
        }
    }
    let mut types = types.into_iter().collect::<Vec<_>>();
    types.sort();
    for t in types.iter() {
        println!("Type: {t:?}");
    }

    // Start user interface
    App::run(&mod_host_controller).expect("Running app");
    drop(file);
    mod_host_controller
        .mod_host_th
        .join()
        .expect("Joining mod-host thread");

    Ok(())
}
