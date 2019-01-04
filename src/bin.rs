#![warn(rust_2018_idioms, clippy::all, clippy::pedantic)]

use clap::{Arg, App, SubCommand};
use std::io::Write;

fn main() {
    let matches = App::new("survival_tiles_tool")
        .version("1.0")
        .author("Walter Pearce <jaynus@gmail.com>")
        .about("Does awesome things")
        .subcommand(SubCommand::with_name("convert")
            .about("Convert Tiled files to amethyst serialized RON file")
            .arg(Arg::with_name("input")
                .short("i")
                .long("input")
                .value_name("FILE")
                .help("Input Tiled .tmx file")
                .takes_value(true)
                .required(true))
            .arg(Arg::with_name("output")
                .short("o")
                .long("output")
                .value_name("FILE")
                .help("Output RON file")
                .takes_value(true))
        ).get_matches();

    if let Some(matches) = matches.subcommand_matches("convert") {

        let path = std::path::Path::new(matches.value_of("input").unwrap());
        let file = std::fs::File::open(&path).unwrap();
        let serialized = survival_tiles::amethyst::spritesheet_from_tiled(file, &path).unwrap();

        let pretty = ron::ser::PrettyConfig {
            depth_limit: 99,
            separate_tuple_members: true,
            enumerate_arrays: true,
            ..ron::ser::PrettyConfig::default()
        };
        let ron_tiles = ron::ser::to_string_pretty(&serialized, pretty).expect("Serialization failed");


        if matches.is_present("output") {
            // Write to file
            std::fs::File::create(&std::path::Path::new(matches.value_of("output").unwrap())).expect("Unable to open output file")
                .write_all(ron_tiles.as_bytes()).expect("Unable to write to output file");
            
        } else {
            println!("{}", ron_tiles);
        }
    }

}