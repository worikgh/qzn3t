///App code taken from https://ratatui.rs/tutorials/counter-app/
use app::App;
use std::collections::HashSet;

use std::fs::OpenOptions;
use std::io;
use std::io::Write;
use std::io::StdinLock;
use std::io::Lines;
use lv2::get_lv2_controller;
use lv2::Lv2Type;

use crate::lv2::ModHostController;
mod app;
mod run_executable;
mod lv2;
fn main() -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .open("/tmp/output.txt")
        .expect("Failed to open file");

    file.write_all(b"Debug messages from main.rs\n")?;


    // Store processed plugins so only get procerssed once
    let lines:Lines<StdinLock> = io::stdin().lines();
    let mod_host_controller:ModHostController = get_lv2_controller(lines)?;

    // Do something with the mod_host_controller
    let mut types:HashSet<Lv2Type> = HashSet::new();
    for lv2 in mod_host_controller.simulators.iter(){
	for t in lv2.types.iter() {
	    let lv2_type:Lv2Type = t.clone();
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

    // let mut mod_host_resp = "".to_string();
    // // Enforce twenty frames a second from this loop
    // let target_fps = 20;
    // let target_frame_time = time::Duration::from_secs(1) / target_fps;    
    
    // loop {
    //     // Note the time at the top of th eloop, and sleep at the
    //     // bottom to keep the looping speed constant
    //     let start_time = time::Instant::now();

    // 	let latest = mod_host_controller.get_data_nb()?;
    // 	if latest.len() == 0 && mod_host_resp.len() != 0 {
    // 	    // There is some data from mod-host, but it has stopped sending any.
    // 	    if mod_host_resp == "bye" {
    // 		// Mod-host has quit
    // 		break;
    // 	    }
    // 	    // Output response.  To stdout for now.  Soon send to UI
    // 	    print!("{mod_host_resp}");
    // 	    mod_host_resp = "".to_string();
    // 	}else {
    // 	    if latest.len() != 0 {
    // 		// Got some more response
    // 		mod_host_resp += latest.as_str();
    // 	    }
    // 	}
    //     // enforce duration
    //     let elapsed_time = start_time.elapsed();
    //     if elapsed_time < target_frame_time {
    //         thread::sleep(target_frame_time - elapsed_time);
    //     }
    // }	
    // Close the file
    drop(file);
    mod_host_controller.mod_host_th.join().expect("Joining mod-host thread");	

    Ok(())
}
