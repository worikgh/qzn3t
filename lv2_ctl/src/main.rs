///App code taken from https://ratatui.rs/tutorials/counter-app/
use app::App;
use mod_host_controller::ModHostController;
use std::fs::File;
use std::io::{BufRead, BufReader};

mod app;
mod colours;
mod lv2;
mod lv2_simulator;
mod lv2_stateful_list;
mod mod_host_controller;
mod port;
mod port_table;
mod run_executable;
mod test_data;
fn main() -> std::io::Result<()> {
   let file = if let Ok(f) = File::open("../../lv2.dat") {
      f
   } else if let Ok(f) = File::open("lv2.dat") {
      f
   } else {
      panic!("Cannot find data")
   };
   let reader = BufReader::new(file);
   // let lines: Lines<StdinLock> = io::stdin().lines();
   // let mut mod_host_controller: ModHostController =
   //    get_lv2_controller(lines.map(|r| r.map_err(Into::into)))?;
   let mut mod_host_controller: ModHostController =
      ModHostController::get_lv2_controller(
         reader.lines().map(|r| r.map_err(Into::into)),
      )?;
   // Start user interface.  Loop until user quits
   App::run(&mut mod_host_controller).expect("Running app");

   mod_host_controller
      .mod_host_th
      .join()
      .expect("Joining mod-host thread");

   Ok(())
}
