extern crate clap;
#[macro_use]
extern crate failure;
extern crate indicatif;
extern crate path_abs;
extern crate pathdiff;
extern crate walkdir;
use clap::{arg, command, value_parser, ArgAction, Command};
use std::path::PathBuf;

use std::error;

mod app;
mod avgspeed;
mod copy;

fn main() -> Result<(), Box<error::Error>> {
    let matches = command!()
        .arg(arg!(["ppcp"]))
        .version("0.0.1")
        .author("Nikita Bilous <nikita@bilous.me>")
        .about("Copy files in console with progress bar")
        .arg(
            arg!(
                -s --source <PATH> "source path"
            )
            .required(true)
            .value_parser(value_parser!(PathBuf)),
        )
        .arg(
            arg!(
                -d --dest <PATH> "source path"
            )
            .required(true)
            .value_parser(value_parser!(PathBuf)),
        )
        .get_matches();

    let mut app = app::App::new();
    app.run(&matches)?;
    Ok(())
}
