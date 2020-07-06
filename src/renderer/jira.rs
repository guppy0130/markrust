extern crate comrak;

use comrak::nodes::*;
use std::io::Write;

/// render a node to Jira markup
///
/// # Arguments
///
/// * `node` - node to parse
/// * `bullet_stack` - a stack of bullets to render
/// * `table_header` - if table header enabled
/// * `writer` - something that implements Write
///
/// # Returns
///
/// * `closing_string` - Option<String> to close the current node after child parsing
/// * `pop_bullet_stack` - pop the latest bullet from the stack when done with a list
/// * `newline` - number of newlines to append after element finishes
/// * `table_header` - if table header enabled
pub fn render(
    node: &AstNode,
    bullet_stack: &mut Vec<u8>,
    table_header: &mut bool,
    writer: &mut dyn Write,
) -> (Option<String>, bool, u8, bool) {
    fn write(string: &str, writer: &mut dyn Write) {
        writer
            .write_all(string.as_bytes())
            .expect("failed to write to file");
    }

    match &node.data.borrow_mut().value {
        NodeValue::BlockQuote => {
            write("{quote}\n", writer);
            return (Some("{quote}".to_string()), false, 2, *table_header);
        }
        NodeValue::List(nodelist) => {
            match nodelist.list_type {
                ListType::Bullet => {
                    bullet_stack.push("*".as_bytes()[0]);
                }
                ListType::Ordered => {
                    bullet_stack.push("#".as_bytes()[0]);
                }
            }
            return (None::<String>, true, 0, *table_header);
        }
        NodeValue::Item(_) => {
            // TODO: figure out why nesting numbers inside bullets renders two lists
            let composed_str = String::from_utf8(bullet_stack.to_vec()).unwrap() + " ";
            write(&composed_str, writer);
        }
        NodeValue::CodeBlock(codeblock) => {
            if !codeblock.info.is_empty() {
                let codeblock_info = String::from_utf8(codeblock.info.to_vec()).unwrap();
                write(&format!("{{code:{}}}", codeblock_info), writer);
            } else {
                write("{{code}}", writer);
            }
            write("\n", writer);
            write(
                &String::from_utf8(codeblock.literal.to_vec()).unwrap(),
                writer,
            );
            return (Some("{code}".to_string()), false, 2, *table_header);
        }
        NodeValue::Paragraph => {
            return (None::<String>, false, 1, *table_header);
        }
        NodeValue::Heading(heading) => {
            write(&format!("h{}. ", heading.level.to_string()), writer);
            return (None::<String>, false, 2, *table_header);
        }
        NodeValue::ThematicBreak => {
            write("\n", writer);
            write("----", writer);
            return (None::<String>, false, 2, *table_header);
        }
        NodeValue::Table(_alignment) => {
            // alignment ignored by jira
            write("\n", writer);
            return (None::<String>, false, 1, false);
        }
        NodeValue::TableRow(is_header) => {
            if *is_header {
                *table_header = true;
                return (Some("||".to_string()), false, 1, *table_header);
            } else {
                *table_header = false;
                return (Some("|".to_string()), false, 1, *table_header);
            }
        }
        NodeValue::TableCell => {
            if *table_header {
                write("||", writer);
            } else {
                write("|", writer);
            }
        }
        NodeValue::Code(code_span) => {
            write("{{ monospaced }}", writer);
            write(&String::from_utf8(code_span.to_vec()).unwrap(), writer);
            write("{{ monospaced }}", writer);
        }
        NodeValue::Emph => {
            write("_", writer);
            return (Some("_".to_string()), false, 1, *table_header);
        }
        NodeValue::Strong => {
            write("*", writer);
            return (Some("*".to_string()), false, 1, *table_header);
        }
        NodeValue::Strikethrough => {
            write("-", writer);
            return (Some("-".to_string()), false, 1, *table_header);
        }
        NodeValue::Link(link) => {
            write("[", writer);
            if link.title.len() > 0 {
                write(
                    &format!("{}|", String::from_utf8(link.title.to_vec()).unwrap()),
                    writer,
                );
            }
            write(
                &format!("{}]", &String::from_utf8(link.url.to_vec()).unwrap()),
                writer,
            );
        }
        NodeValue::Text(text) => {
            // not sure if this is memory efficient...
            write(&String::from_utf8(text.to_vec()).unwrap(), writer);
        }
        _ => (),
    }
    return (None::<String>, false, 0, *table_header);
}

#[cfg(test)]
use comrak::nodes::Ast;
#[allow(unused_imports)] // nvim rls was bugging out for some reason
use std::cell::RefCell;

#[test]
/// does blockquote write out "{{ quote }}" and return ("{{ quote }}", true)?
fn test_blockquote() {
    let mut bullet_stack = Vec::new();
    let mut output = Vec::new();

    let blockquote_node = AstNode::new(RefCell::new(Ast::new(NodeValue::BlockQuote)));
    let (closing_string, _, newline, _) =
        render(&blockquote_node, &mut bullet_stack, &mut false, &mut output);
    assert_eq!(newline, 2);
    assert_eq!("{quote}\n", String::from_utf8(output).unwrap());
    assert_eq!("{quote}", closing_string.unwrap());
}

#[test]
/// unordered list
fn test_unordered_list() {
    let mut bullet_stack = Vec::new();
    let mut output = Vec::new();

    let node = AstNode::new(RefCell::new(Ast::new(NodeValue::List(NodeList {
        list_type: ListType::Bullet,
        marker_offset: 0,
        padding: 0,
        start: 1,
        delimiter: ListDelimType::Period,
        bullet_char: b'*',
        tight: false,
    }))));
    let (_, pop_bullet_stack, newline, _) =
        render(&node, &mut bullet_stack, &mut false, &mut output);
    assert!(pop_bullet_stack);
    assert!(newline == 0);
    assert_eq!("", String::from_utf8(output).unwrap());
}

#[test]
/// ordered list
fn test_ordered_list() {
    let mut bullet_stack = Vec::new();
    let mut output = Vec::new();

    let node = AstNode::new(RefCell::new(Ast::new(NodeValue::List(NodeList {
        list_type: ListType::Ordered,
        marker_offset: 0,
        padding: 0,
        start: 1,
        delimiter: ListDelimType::Period,
        bullet_char: b'#',
        tight: false,
    }))));
    let (_, pop_bullet_stack, newline, _) =
        render(&node, &mut bullet_stack, &mut false, &mut output);
    assert!(pop_bullet_stack);
    assert!(newline == 0);
    assert_eq!("", String::from_utf8(output).unwrap());
}

#[test]
/// codeblock
fn test_codeblock() {
    let mut bullet_stack = Vec::new();
    let mut output = Vec::new();

    let node = AstNode::new(RefCell::new(Ast::new(NodeValue::CodeBlock(
        NodeCodeBlock {
            fence_char: b'`',
            fence_length: 0,
            fence_offset: 0,
            fenced: true,
            info: String::from("rust").into_bytes(),
            literal: String::from("let mut bullet_stack = Vec::new()").into_bytes(),
        },
    ))));
    let (closing_string, _, newline, _) = render(&node, &mut bullet_stack, &mut false, &mut output);
    assert_eq!("{code}", closing_string.unwrap());
    assert!(newline == 2);
    assert_eq!(
        "{code:rust}\nlet mut bullet_stack = Vec::new()",
        String::from_utf8(output).unwrap()
    );
}

#[test]
/// heading
fn test_heading() {
    let mut bullet_stack = Vec::new();
    let mut output = Vec::new();

    let node = AstNode::new(RefCell::new(Ast::new(NodeValue::Heading(NodeHeading {
        level: 1,
        setext: false,
    }))));
    let (_, _, newline, _) = render(&node, &mut bullet_stack, &mut false, &mut output);
    assert_eq!(newline, 2);
    assert_eq!("h1. ", String::from_utf8(output).unwrap());
}

#[test]
/// thematic break
fn test_thematic_break() {
    let mut bullet_stack = Vec::new();
    let mut output = Vec::new();

    let node = AstNode::new(RefCell::new(Ast::new(NodeValue::ThematicBreak)));
    let (_, _, newline, _) = render(&node, &mut bullet_stack, &mut false, &mut output);
    assert_eq!(newline, 2);
    assert_eq!("\n----", String::from_utf8(output).unwrap());
}

#[test]
/// code span
fn test_code_span() {
    let mut bullet_stack = Vec::new();
    let mut output = Vec::new();

    let node = AstNode::new(RefCell::new(Ast::new(NodeValue::Code(
        String::from("monospaced content").into_bytes(),
    ))));
    render(&node, &mut bullet_stack, &mut false, &mut output);
    assert_eq!(
        "{{ monospaced }}monospaced content{{ monospaced }}",
        String::from_utf8(output).unwrap()
    );
}

#[test]
/// italics
fn test_emph() {
    let mut bullet_stack = Vec::new();
    let mut output = Vec::new();

    let node = AstNode::new(RefCell::new(Ast::new(NodeValue::Emph)));
    let (closing_string, _, _, _) = render(&node, &mut bullet_stack, &mut false, &mut output);
    assert_eq!("_", closing_string.unwrap());
    assert_eq!("_", String::from_utf8(output).unwrap());
}

#[test]
/// bold
fn test_strong() {
    let mut bullet_stack = Vec::new();
    let mut output = Vec::new();

    let node = AstNode::new(RefCell::new(Ast::new(NodeValue::Strong)));
    let (closing_string, _, _, _) = render(&node, &mut bullet_stack, &mut false, &mut output);
    assert_eq!("*", closing_string.unwrap());
    assert_eq!("*", String::from_utf8(output).unwrap());
}

#[test]
/// strikethrough
fn test_strikethrough() {
    let mut bullet_stack = Vec::new();
    let mut output = Vec::new();

    let node = AstNode::new(RefCell::new(Ast::new(NodeValue::Strikethrough)));
    let (closing_string, _, _, _) = render(&node, &mut bullet_stack, &mut false, &mut output);
    assert_eq!("-", closing_string.unwrap());
    assert_eq!("-", String::from_utf8(output).unwrap());
}

#[test]
/// link title
fn test_link_title() {
    let mut bullet_stack = Vec::new();
    let mut output = Vec::new();

    let node = AstNode::new(RefCell::new(Ast::new(NodeValue::Link(NodeLink {
        title: String::from("hello world").into_bytes(),
        url: String::from("https://example.com").into_bytes(),
    }))));
    render(&node, &mut bullet_stack, &mut false, &mut output);
    assert_eq!(
        "[hello world|https://example.com]",
        String::from_utf8(output).unwrap()
    );
}

#[test]
/// link no title
fn test_link_no_title() {
    let mut bullet_stack = Vec::new();
    let mut output = Vec::new();

    let node = AstNode::new(RefCell::new(Ast::new(NodeValue::Link(NodeLink {
        title: Vec::new(),
        url: String::from("https://example.com").into_bytes(),
    }))));
    render(&node, &mut bullet_stack, &mut false, &mut output);
    assert_eq!("[https://example.com]", String::from_utf8(output).unwrap());
}
