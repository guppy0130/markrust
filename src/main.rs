extern crate pulldown_cmark;
use pulldown_cmark::{Options, Parser};

mod renderer;
use renderer::jira;

#[macro_use]
extern crate clap;
use clap::{App, Arg};

use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::process::Command;
use std::{env, fs};

fn main() -> io::Result<()> {
    let args = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .arg(
            Arg::with_name("toc")
                .help("Prepend TOC markup")
                .long("toc")
                .short("t")
                .multiple(false)
                .takes_value(false)
                .required(false),
        )
        .arg(
            Arg::with_name("modify_headers")
                .help("add N to each header level. Can be negative")
                .long("modify-headers")
                .short("m")
                .multiple(false)
                .takes_value(true)
                .require_equals(true) // so negative numbers aren't flags
                .required(false),
        )
        .arg(
            Arg::with_name("input")
                .help("FILE input, or empty for stdin")
                .long("input")
                .short("i")
                .index(1)
                .required(false),
        )
        .arg(
            Arg::with_name("output")
                .help("FILE output, or empty for stdout")
                .long("output")
                .short("o")
                .index(2)
                .required(false),
        )
        .arg(
            Arg::with_name("editor")
                .help("Launch $EDITOR as input")
                .long("editor")
                .short("e")
                .required(false)
                .multiple(false)
                .takes_value(false)
                .conflicts_with("output"), // we just want to ensure 1 file passed
        )
        .get_matches();

    // if --editor is passed, launch $EDITOR with a temporary file
    // you can provide `-e OUTPUT`, but this means reinterpreting INPUT as OUTPUT if `-e` is passed.
    let mut tmpfile = env::temp_dir();
    tmpfile.push("markrust.md");

    let mut input_file: Option<&str> = args.value_of("input");
    let mut output_file: Option<&str> = args.value_of("output");

    if args.is_present("editor") {
        fs::File::create(&tmpfile).expect("Could not write temporary file. Falling back to stdin.");

        // launch the editor
        let editor = env::var("EDITOR").unwrap_or("vim".to_string());
        Command::new(editor)
            .arg(&tmpfile)
            .status()
            .expect("Failed to launch $EDITOR. Do you have flags?");

        // treat the `input` as `output`
        output_file = input_file;
        input_file = tmpfile.to_str();
    }

    // take either stdin or a file
    let mut input_reader: Box<dyn BufRead> = match input_file {
        Some(filename) => Box::new(BufReader::new(
            fs::File::open(filename).expect("Could not read input file"),
        )),
        None => Box::new(BufReader::new(io::stdin())),
    };

    // stringify input for parser
    let mut input_string = String::new();
    input_reader
        .read_to_string(&mut input_string)
        .expect("Could not read input");

    // output to either stdout or a file
    let mut output_writer: Box<dyn Write> = match output_file {
        Some(filename) => Box::new(BufWriter::new(
            fs::File::create(filename).expect("could not create output file"),
        )),
        None => Box::new(BufWriter::new(io::stdout())),
    };

    let options = Options::all();
    let parser = Parser::new_ext(&input_string, options);

    if args.is_present("toc") {
        // prepend TOC markup first
        jira::write_toc(&mut output_writer)?;
    }

    let modify_headers = value_t!(args.value_of("modify_headers"), i8).unwrap_or(0);
    jira::write_jira(&mut output_writer, parser, modify_headers)?;

    // flush before drop
    output_writer.flush()
}
