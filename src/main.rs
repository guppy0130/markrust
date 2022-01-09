extern crate pulldown_cmark;
use pulldown_cmark::{Options, Parser as MarkdownParser};

/// The renderer is responsible for converting events from pulldown-cmark into markup
mod renderer;
use renderer::jira;

use clap::{ArgGroup, Parser};

use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::process::Command;
use std::{env, fs};

#[derive(Parser)]
#[clap(author, version, about)]
// invalid cases: `-e in out`
// valid cases: `-e out` `` `in out`
// exclude output because we'll manually shift input to output later (index 0 is input)
#[clap(group(ArgGroup::new("editor_exclusion").required(false).args(&["output", "editor"])))]
struct Cli {
    /// Prepend TOC markup
    #[clap(short, long)]
    toc: bool,
    /// FILE input, or empty for stdin
    input: Option<String>,
    /// FILE output, or empty for stdout
    output: Option<String>,
    /// Launch $EDITOR as input
    #[clap(short, long)]
    editor: bool,
    /// Add N to header level (can be negative)
    #[clap(default_value_t = 0, short, long)]
    modify_headers: i8,
}
/// Binary entrypoint
///
/// # Returns
///
/// * `Result` - from writing to stdout or file
fn main() -> io::Result<()> {
    let args = Cli::parse();

    let mut input_file: Option<String> = args.input;
    let mut output_file: Option<String> = args.output;

    if args.editor {
        // if --editor is passed, launch $EDITOR with a temporary file you can
        // provide `-e OUTPUT`, but this means reinterpreting INPUT as OUTPUT if
        // `-e` is passed.
        let mut tmpfile = env::temp_dir();
        tmpfile.push("markrust.md");

        fs::File::create(&tmpfile).expect("Could not write temporary file. Falling back to stdin.");

        // launch the editor
        let editor = env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
        Command::new(editor)
            .arg(&tmpfile)
            .status()
            .expect("Failed to launch $EDITOR. Do you have flags?");

        // treat the `input` as `output`
        output_file = input_file;
        input_file = Some(String::from(tmpfile.to_str().unwrap()));
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
    let parser = MarkdownParser::new_ext(&input_string, options);

    if args.toc {
        // prepend TOC markup first
        jira::write_toc(&mut output_writer)?;
    }

    let modify_headers = args.modify_headers;
    jira::write_jira(&mut output_writer, parser, modify_headers)?;

    // flush before drop
    output_writer.flush()
}
