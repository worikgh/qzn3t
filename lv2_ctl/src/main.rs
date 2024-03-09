///App code taken from https://ratatui.rs/tutorials/counter-app/
use app::App;

use lv2::get_lv2_controller;
use std::io;
use std::io::Lines;
use std::io::StdinLock;

use crate::lv2::ModHostController;
mod app;
mod lv2;
mod run_executable;
fn main() -> std::io::Result<()> {
    let lines: Lines<StdinLock> = io::stdin().lines();
    let mod_host_controller: ModHostController = get_lv2_controller(lines)?;

    // Start user interface.  Loop until user quits
    App::run(&mod_host_controller).expect("Running app");

    mod_host_controller
        .mod_host_th
        .join()
        .expect("Joining mod-host thread");

    Ok(())
}
