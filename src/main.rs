use clap::{arg, command, value_parser, ArgAction};
use std::error;
use std::path::PathBuf;
mod app;
mod avgspeed;
mod copy;
use clap::Arg;
fn main() -> Result<(), Box<dyn error::Error>> {
    let matches = command!()
        .version("0.0.1")
        .author("Nikita Bilous <nikita@bilous.me>")
        .about("Copy files in console with progress bar")
        .arg(
            Arg::new("source")
                .required(true)
                .num_args(1..)
                .action(ArgAction::Append)
                .index(1)
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("destination")
                .required(true)
                .index(2)
                // .last(true)
                .value_parser(value_parser!(PathBuf)),
        )
        .get_matches();

    let mut app = app::App::new();
    app.run(&matches)?;
    Ok(())
}
