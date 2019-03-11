extern crate clap;
// #[macro_use]
// extern crate log;
// extern crate env_logger;
#[macro_use] extern crate failure;
// extern crate ignore;
extern crate walkdir;
extern crate indicatif;
extern crate pathdiff;

use clap::{Arg, App, SubCommand};

use std::error;

mod app;
mod copy;

use std::sync::mpsc::channel;
use std::time::Duration;
use std::thread;

fn main() -> Result<(), Box<error::Error>> {
    let matches = App::new("qop")
        .version("0.0.1")
        .author("Nikita Bilous <nikita@bilous.me>")
        .about("File operations manager")
        .subcommand(SubCommand::with_name("cp")
            .aliases(&["copy"])
            .about("copy files")
            .arg(Arg::with_name("source")
                 .index(1)
                 .required(true)
                 .help("path")
                 .multiple(true)
            )
            .arg(Arg::with_name("dest")
                 // .index(2)
                 .required(true)
                 .help("dest")
                 .multiple(false)
            )
        )
        .get_matches();

    // let (cs, cr) = channel::<Option<usize>>();
    // thread::spawn(move || {
    //     while let Ok(Some(i)) = cr.recv() {
    //         println!("{}", i);
    //     }
    //     println!("done");
    // });
    // for i in 1..10 {
    //     cs.send(Some(i)).unwrap();
    // }
    // cs.send(None).unwrap();
    // // drop(cs);
    // thread::sleep(Duration::from_millis(1000));
    // println!("bye");
    // return Ok(());

    let mut app = app::App::new(&matches);
    app.run(&matches)?;
    Ok(())
}
