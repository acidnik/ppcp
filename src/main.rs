extern crate clap;
#[macro_use] extern crate failure;
extern crate walkdir;
extern crate indicatif;
extern crate pathdiff;

use clap::{Arg, App};

use std::error;

mod app;
mod copy;

fn main() -> Result<(), Box<error::Error>> {
    let matches = App::new("ppcp")
        .version("0.0.1")
        .author("Nikita Bilous <nikita@bilous.me>")
        .about("Copy files in console with progress bar")
        .arg(Arg::with_name("source")
             .index(1)
             .required(true)
             .help("source path")
             .multiple(true)
        )
        .arg(Arg::with_name("dest")
             .required(true)
             .help("destination path")
             .multiple(false)
        )
        .get_matches();

    let mut app = app::App::new();
    app.run(&matches)?;
    Ok(())
}
