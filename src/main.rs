extern crate comrak;

mod renderer;
use renderer::jira;

use comrak::nodes::AstNode;
use comrak::{parse_document, Arena, ComrakOptions};

use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::{env, fs};

fn main() {
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
    let mut output_writer: Box<dyn Write> = match output_target {
        None => Box::new(BufWriter::new(io::stdout())),
        Some(filename) => Box::new(BufWriter::new(
            fs::File::create(filename).expect("could not create output file"),
        )),
    };

    // setup markdown parser settings
    let comrak_options = ComrakOptions {
        ext_table: true,
        ..ComrakOptions::default()
    };

    // parse markdown to AST
    let arena = Arena::new();

    let root = parse_document(&arena, &input_string, &comrak_options);

    /// Converts `node` to jira formatting
    ///
    /// # Arguments
    ///
    /// * `node` - node to evaluate
    /// * `writer` - something that implements Write, to write to
    /// * `bullet_stack` - keeps track of bullets depth and type
    /// * `table_header` - keeps track if working on header row or not
    /// * `f` - function to apply against node. In this case, the jira formatter. Should return a
    /// string, which we use to close the jira markup, if necessary.
    fn iter_nodes<'a, F>(
        node: &'a AstNode<'a>,
        writer: &mut dyn Write,
        bullet_stack: &mut Vec<u8>,
        mut table_header: &mut bool,
        f: &F,
    ) where
        F: Fn(
            &'a AstNode<'a>,
            &mut Vec<u8>,
            &mut bool,
            &mut dyn Write,
        ) -> (Option<String>, bool, u8, bool),
    {
        // render current element
        let (closing_string, pop_bullet_stack, newline, mut table_header) =
            f(node, bullet_stack, &mut table_header, writer);
        // render children
        for c in node.children() {
            iter_nodes(c, writer, bullet_stack, &mut table_header, f);
        }
        // close current element
        if pop_bullet_stack {
            bullet_stack.pop();
            bullet_stack.truncate(bullet_stack.len());
        }

        if closing_string.is_some() {
            writer
                .write_all(closing_string.unwrap().as_bytes())
                .expect("failed to write to file");
        }
        writer
            .write_all("\n".repeat(newline.into()).as_bytes())
            .expect("failed to write to file");
    }

    // run the parser against the root
    let mut bullet_stack: Vec<u8> = Vec::new();
    let mut table_header: bool = false;
    iter_nodes(
        root,
        output_writer.by_ref(),
        &mut bullet_stack,
        &mut table_header,
        &jira::render,
    );
}
