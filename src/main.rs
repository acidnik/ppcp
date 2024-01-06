use clap::{arg, command, value_parser, ArgAction};
use std::error;
use std::path::PathBuf;
mod app;
mod avgspeed;
mod copy;

fn main() -> Result<(), Box<dyn error::Error>> {
    let matches = command!()
        .version("0.0.1")
        .author("Nikita Bilous <nikita@bilous.me>")
        .about("Copy files in console with progress bar")
        .arg(
            arg!(
                -d --dest <PATH> "source path"
            )
            .required(true)
            .value_parser(value_parser!(PathBuf))
            .last(true),
        )
        .arg(
            arg!(
                -s --source <PATH> "source path"
            )
            .required(true)
            .value_parser(value_parser!(PathBuf))
            .index(1)
            .action(ArgAction::Append),
        )
        .get_matches();

    let mut app = app::App::new();
    app.run(&matches)?;
    Ok(())
}
