extern crate pulldown_cmark;

mod renderer;
use renderer::jira;

use pulldown_cmark::{Options, Parser};

use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::{env, fs};

fn main() -> io::Result<()> {
    // take either stdin or a file
    let input_target = env::args().nth(1);
    let mut input_reader: Box<dyn BufRead> = match input_target {
        None => Box::new(BufReader::new(io::stdin())),
        Some(filename) => Box::new(BufReader::new(
            fs::File::open(filename).expect("could not read input file"),
        )),
    };

    // stringify input
    let mut input_string = String::new();
    input_reader
        .read_to_string(&mut input_string)
        .expect("Could not read input");

    // output to either stdout or a file
    let output_target = env::args().nth(2);
    let output_writer: Box<dyn Write> = match output_target {
        None => Box::new(BufWriter::new(io::stdout())),
        Some(filename) => Box::new(BufWriter::new(
            fs::File::create(filename).expect("could not create output file"),
        )),
    };

    let options = Options::all();
    let parser = Parser::new_ext(&input_string, options);

    jira::write_jira(output_writer, parser)
}
