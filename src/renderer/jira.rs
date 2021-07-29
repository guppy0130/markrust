extern crate pulldown_cmark;
use pulldown_cmark::*;

use std::collections::HashMap;
use std::convert::TryFrom;
use std::io::{self, Write};

/// Builds the language mapper
///
/// # Returns
///
/// * `lang_map` - HashMap<String, String> from markdown to confluence-supported code block langs
fn build_lang_map() -> HashMap<String, String> {
    let mut lang_map = HashMap::new();
    let approved_langs = [
        "actionscript3",
        "applescript",
        "bash",
        "c#",
        "c++",
        "css",
        "coldfusion",
        "delphi",
        "diff",
        "erlang",
        "groovy",
        "xml",
        "java",
        "jfx",
        "javascript",
        "php",
        "text",
        "powershell",
        "python",
        "ruby",
        "sql",
        "sass",
        "scala",
        "vb",
        "yaml",
    ];
    for &lang in &approved_langs {
        // add map from self to self
        lang_map.insert(lang.to_string(), lang.to_string());
    }

    /// build aliases/mappings from markdown -> confluence
    ///
    /// # Arguments
    ///
    /// * `sub_map` - the language map
    /// * `approved_lang` - the confluence keyword
    /// * `aliases` - vec![] of markdown keywords
    fn build_aliases(
        sub_map: &mut std::collections::HashMap<std::string::String, std::string::String>,
        approved_lang: &str,
        aliases: Vec<&str>,
    ) {
        for alias in aliases {
            sub_map.insert(alias.to_string(), approved_lang.to_string());
        }
    }

    // aliases and mapping between languages
    // honestly, there should be a better way of doing this...
    build_aliases(&mut lang_map, "actionscript3", vec!["as3", "actionscript"]);
    build_aliases(&mut lang_map, "applescript", vec!["osascript"]);
    build_aliases(&mut lang_map, "bash", vec!["console", "shell", "zsh", "sh"]);
    build_aliases(&mut lang_map, "c#", vec!["csharp"]);
    build_aliases(&mut lang_map, "c++", vec!["cpp"]);
    build_aliases(
        &mut lang_map,
        "coldfusion",
        vec!["cfm", "cfml", "coldfusion html"],
    );
    build_aliases(&mut lang_map, "delphi", vec!["pascal", "objectpascal"]);
    build_aliases(&mut lang_map, "diff", vec!["udiff"]);
    build_aliases(&mut lang_map, "xml", vec!["html"]);
    build_aliases(&mut lang_map, "jfx", vec!["java fx"]);
    build_aliases(&mut lang_map, "javascript", vec!["js", "node"]);
    build_aliases(&mut lang_map, "php", vec!["inc"]);
    build_aliases(&mut lang_map, "powershell", vec!["posh"]);
    build_aliases(
        &mut lang_map,
        "ruby",
        vec!["jruby", "macruby", "rake", "rb", "rbx"],
    );
    build_aliases(&mut lang_map, "sass", vec!["scss", "less", "stylus"]);
    build_aliases(&mut lang_map, "vb", vec!["visual basic", "vb.net", "vbnet"]);
    return lang_map;
}

/// Makes a list of characters to escape when inside curly braces
///
/// # Returns
///
/// * `HashMap<String, String>` - from original character to escaped sequence
fn make_escape_list() -> HashMap<String, String> {
    let mut escape_map = HashMap::new();

    fn add_escape(
        sub_map: &mut std::collections::HashMap<std::string::String, std::string::String>,
        key: &str,
        value: &str,
    ) {
        sub_map.insert(key.to_string(), value.to_string());
    }

    // we may not need to escape all; view [JiraWriter::write_escaped()]
    add_escape(&mut escape_map, "{", "&#123;");
    add_escape(&mut escape_map, "}", "&#125;");
    add_escape(&mut escape_map, "*", "\\*");

    return escape_map;
}

/// The JiraWriter takes events from pulldown-cmark and formats it into Atlassian markup
struct JiraWriter<I, W> {
    iter: I,
    writer: W,
    // if we ended on a newline so we can fix newlines for lists
    end_newline: bool,
    // if we're on a table header cell
    table_header: bool,
    // what bullets we're working with
    bullet_stack: Vec<u8>,
    // if we come across a link, set this to true so we can capture the incoming string in the
    // first half of the link
    link: bool,
    // if we're working with an image, we'll need to keep track of states
    image: bool,
    image_text: bool,
    // must ensure space after inline code end curly brace
    inline_code: bool,
    // map between markdown/confluence code block langs
    lang_map: HashMap<String, String>,
    // add modify_headers to header level
    modify_headers: i8,
    // if the current line should be output. Solves the issue of header parts being output when
    // unnecessary
    should_output_line: bool,
    // escape some stuff in the code blocks, etc.
    escape_map: HashMap<String, String>,
}

impl<'a, I, W> JiraWriter<I, W>
where
    I: Iterator<Item = Event<'a>>,
    W: Write,
{
    /// return a new JiraWriter
    ///
    /// # Arguments
    ///
    /// * `iter` - iterator of elements provided by `pulldowm_cmark`
    /// * `writer` - something implementing Write to write output to
    fn new(iter: I, writer: W, modify_headers: i8) -> Self {
        // confluence/jira only implements the following language highlighting
        // doing this now means the cost is 1 instead of N
        Self {
            iter,
            writer,
            end_newline: false,
            table_header: false,
            bullet_stack: vec![],
            link: false,
            image: false,
            image_text: false,
            inline_code: false,
            lang_map: build_lang_map(),
            modify_headers: modify_headers,
            should_output_line: true,
            escape_map: make_escape_list(),
        }
    }

    /// Writes `s` to underlying `writer`, if it should write.
    /// Sets `self.end_newline` to true if `s` ends in a newline.
    ///
    /// # Arguments
    ///
    /// * `s` - string to write
    fn write(&mut self, s: &str) -> io::Result<()> {
        if self.should_output_line {
            self.end_newline = s.ends_with("\n");
            self.writer.write_all(s.as_bytes())
        } else {
            Ok(())
        }
    }

    /// Writes a newline to underlying `writer`.
    fn write_newline(&mut self) -> io::Result<()> {
        self.write("\n")
    }

    /// Replace curly braces (and other special chars) so macros don't explode
    ///
    /// # Arguments
    ///
    /// * `s` - string to check
    ///
    /// # Returns
    ///
    /// * `s` - string with {} replaced with HTML equivalent
    fn write_escaped(&mut self, s: &str) -> io::Result<()> {
        let mut r = String::from(s);
        for (key, value) in self.escape_map.iter() {
            r = r.replace(key, value);
        }
        // if these characters are first, they break rendering, but it doesn't matter if they show
        // up later, so you only need to replace the first!
        match r.chars().nth(0).unwrap() {
            '-' => {
                r.replace_range(0..1, "\\-");
            }
            _ => (),
        }
        self.write(&r)
    }

    /// Main part of the parser, outputting to underlying `writer`.
    ///
    /// Passes start/end tags out to `start_tag` and `end_tag`, respectively.
    /// Writes out the rest of the inline content as necessary.
    /// Does not render raw HTML or footnote references.
    fn run(&mut self) -> io::Result<()> {
        // using this form means you have to have the Ok(()) at the end?
        while let Some(event) = self.iter.next() {
            match event {
                Event::Start(tag) => {
                    self.start_tag(tag)?;
                }
                Event::End(tag) => {
                    self.end_tag(tag)?;
                }
                Event::Text(text) => {
                    if self.image {
                        self.write("|title=\"")?;
                    }
                    if self.inline_code && !text.starts_with(" ") {
                        // put a space after ending double curly brace
                        self.write(" ")?;
                        self.inline_code = false;
                    }
                    self.write(&text)?;
                    if self.image {
                        self.write("\"")?;
                        self.image_text = true;
                    }
                }
                Event::Code(text) => {
                    self.write("{{")?;
                    self.write_escaped(&text)?;
                    self.write("}}")?;
                    self.inline_code = true;
                }
                Event::SoftBreak => {
                    // a softbreak in GH markdown is not a newline in Atlassian markup
                    self.write(" ")?;
                }
                Event::HardBreak => {
                    // this is the double space followed by newline
                    self.write_newline()?;
                }
                Event::Rule => {
                    self.write_newline()?;
                    self.write("----")?;
                    self.write_newline()?;
                }
                Event::TaskListMarker(_) => {
                    self.write_newline()?;
                    self.write("[] ")?;
                }
                // File a PR if you need a feature
                _ => (),
            }
        }

        Ok(())
    }

    /// Handles opening tags
    /// Since Jira/Confluence doesn't have table alignment built in, we skip that here
    /// Also, skip starting numbered lists at a non-one value...
    ///
    /// # Arguments
    ///
    /// * `tag` - tag to open
    fn start_tag(&mut self, tag: Tag<'a>) -> io::Result<()> {
        match tag {
            Tag::Paragraph => self.write_newline(),
            Tag::Heading(level) => {
                if self.end_newline {
                    self.write_newline()?;
                }
                let parsed_level = i8::try_from(level).unwrap() + self.modify_headers;
                if parsed_level > 0 {
                    if parsed_level < 7 {
                        // valid headers are between 0..=6
                        self.write(&format!("h{}. ", parsed_level))
                    } else {
                        // if the header is > 6, then just treat it as regular text.
                        Ok(())
                    }
                } else {
                    self.should_output_line = false; // skip header contents if header level <= 0
                    Ok(())
                }
            }
            Tag::BlockQuote => {
                self.write_newline()?;
                self.write("{quote}")
            }
            Tag::CodeBlock(code_block_kind) => {
                self.write_newline()?;
                self.write("{code")?;
                match code_block_kind {
                    CodeBlockKind::Fenced(language) => {
                        let default = "text".to_string();
                        let lang = self
                            .lang_map
                            .get(&language.to_string())
                            .unwrap_or(&default)
                            .clone();
                        self.write(&format!(":language={}", &lang))?;
                    }
                    _ => (), // skips indented type
                }
                self.write("}")?;
                self.write_newline()
            }
            Tag::List(first_number) => {
                if first_number.is_some() {
                    self.bullet_stack.push(b'#');
                } else {
                    self.bullet_stack.push(b'*');
                }
                self.write_newline()
            }
            Tag::Item => {
                if !self.end_newline {
                    self.write_newline()?;
                }
                self.write(
                    &(String::from_utf8(self.bullet_stack.to_vec()).unwrap() + &String::from(" ")),
                )
            }
            Tag::TableHead => {
                self.table_header = true;
                self.write_newline()?;
                self.write("||")
            }
            Tag::TableRow => {
                if self.table_header {
                    self.write("||")
                } else {
                    self.write("|")
                }
            }
            Tag::Emphasis => self.write("_"),
            Tag::Strong => self.write("*"),
            Tag::Strikethrough => self.write("-"),
            Tag::Link(_, _, _) => {
                self.link = true;
                self.write("[")
            }
            Tag::Image(_, destination, _) => {
                self.image = true;
                self.write("!")?;
                self.write(&format!("{}", &destination))
            }
            _ => Ok(()),
        }
    }

    /// Handles closing tags
    ///
    /// # Arguments
    ///
    /// * `tag` - tag to close
    fn end_tag(&mut self, tag: Tag<'a>) -> io::Result<()> {
        match tag {
            Tag::Paragraph => self.write_newline(),
            Tag::Heading(_) => {
                if !self.should_output_line {
                    self.should_output_line = true;
                    Ok(())
                } else {
                    self.write_newline()
                }
            }
            Tag::BlockQuote => {
                self.write("{quote}")?;
                self.write_newline()
            }
            Tag::CodeBlock(_) => {
                self.write("{code}")?;
                self.write_newline()
            }
            Tag::List(_) => {
                self.bullet_stack.pop();
                if self.bullet_stack.is_empty() {
                    self.write_newline()
                } else {
                    Ok(())
                }
            }
            Tag::TableHead => {
                self.table_header = false;
                self.write_newline()
            }
            Tag::TableRow => self.write_newline(),
            Tag::TableCell => {
                if self.table_header {
                    self.write("||")
                } else {
                    self.write("|")
                }
            }
            Tag::Emphasis => self.write("_"),
            Tag::Strong => self.write("*"),
            Tag::Strikethrough => self.write("-"),
            Tag::Link(_, destination, _) => {
                if self.link {
                    self.write("|")?;
                }
                self.link = false;
                self.write(&format!("{}]", destination))
            }
            Tag::Image(_, _, alt) => {
                if self.image_text {
                    self.write(",")?;
                } else {
                    self.write("|")?;
                }
                self.write(&format!("alt=\"{}\"", alt))?;
                self.image = false;
                self.image_text = false;
                self.write("!")
            }
            // handle Item
            _ => Ok(()),
        }
    }
}

/// Writes Jira output
///
/// # Arguments
///
/// * `writer` - something implementing the Write trait
/// * `iter` - an iterator of Events from pulldown-cmark
/// * `modify_headers` - a signed int to modify header levels
///
/// # Returns
///
/// * `Result` - if the JiraWriter wrote successfully to `writer`
pub fn write_jira<'a, I, W>(writer: W, iter: I, modify_headers: i8) -> io::Result<()>
where
    I: Iterator<Item = Event<'a>>,
    W: Write,
{
    JiraWriter::new(iter, writer, modify_headers).run()
}

/// Writes the table of contents macro
///
/// # Arguments
///
/// * `writer` - something implementing the Write trait
///
/// # Returns
///
/// * `Result` - if wrote successfully to `writer`
pub fn write_toc<'a, W>(mut writer: W) -> io::Result<()>
where
    W: Write,
{
    // one set of curly braces is consumed to escape the other.
    // the output should be single curly brace (macro)
    write!(writer, "{{toc}}\n\n")
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_headings() {
        let input = "# hello world";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 0).is_ok());
        assert_eq!("h1. hello world\n", String::from_utf8(output).unwrap());

        let input = "## hello world";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 0).is_ok());
        assert_eq!("h2. hello world\n", String::from_utf8(output).unwrap());
    }

    #[test]
    fn test_blockquote() {
        let input = "> hello blockquote";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 0).is_ok());
        assert_eq!(
            "\n\
                {quote}\n\
                hello blockquote\n\
                {quote}\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_codeblock() {
        let input = "\
        ```java\n\
        System.out.println(\"hello world\")\n\
        ```";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 0).is_ok());
        assert_eq!(
            "\n\
                {code:language=java}\n\
                System.out.println(\"hello world\")\n\
                {code}\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_console_codeblock() {
        let input = "\
        ```console\n\
        $ ./console-test.sh\n\
        should be bash\n\
        ```";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 0).is_ok());
        assert_eq!(
            "\n\
                {code:language=bash}\n\
                $ ./console-test.sh\n\
                should be bash\n\
                {code}\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_unknown_codeblock() {
        let input = "\
        ```unknown\n\
        should be text\n\
        ```";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 0).is_ok());
        assert_eq!(
            "\n\
                {code:language=text}\n\
                should be text\n\
                {code}\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_nested_markup_inline_code() {
        let input = "`inline code with an asterisk *` like `rm -rf ./*.extension`";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 0).is_ok());
        assert_eq!(
            "\n{{inline code with an asterisk \\*}} like {{rm -rf ./\\*.extension}}\n",
            String::from_utf8(output).unwrap()
        );
        let input = "a flag like `-r`";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 0).is_ok());
        assert_eq!(
            "\na flag like {{\\-r}}\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_unordered_list() {
        let input = "\
        * item one\n\
        * item two\n\
        * item three";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 0).is_ok());
        assert_eq!(
            "\n\
                * item one\n\
                * item two\n\
                * item three\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_nested_unordered_list() {
        let input = "\
        * item one\n\
        * item two\n\
        \t* nested item one\n\
        \t* nested item two\n\
        * item three";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 0).is_ok());
        assert_eq!(
            "\n\
                * item one\n\
                * item two\n\
                ** nested item one\n\
                ** nested item two\n\
                * item three\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_nested_ordered_in_unordered_list() {
        let input = "\
        * item one\n\
        * item two\n\
        \t1. nested item one\n\
        \t2. nested item two\n\
        * item three";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 0).is_ok());
        assert_eq!(
            "\n\
                * item one\n\
                * item two\n\
                *# nested item one\n\
                *# nested item two\n\
                * item three\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_ordered_list() {
        let input = "\
        1. item one\n\
        2. item two\n\
        3. item three";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 0).is_ok());
        assert_eq!(
            "\n\
                # item one\n\
                # item two\n\
                # item three\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_table() {
        let input = "\
        | header 1 | header 2 |\n\
        |----------|----------|\n\
        | item 1   | item 2   |";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 0).is_ok());
        assert_eq!(
            "\n\
                ||header 1||header 2||\n\
                |item 1|item 2|\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_emphasis() {
        let input = "this is _italics_ in a string";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 0).is_ok());
        assert_eq!(
            "\nthis is _italics_ in a string\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_bold() {
        let input = "this is **bold** in a string";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 0).is_ok());
        assert_eq!(
            "\nthis is *bold* in a string\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_bold_italics() {
        let input = "this is _**bold italics**_ in a string";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 0).is_ok());
        assert_eq!(
            "\nthis is _*bold italics*_ in a string\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_strikethrough() {
        let input = "this is ~~strikethrough~~ in a string";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 0).is_ok());
        assert_eq!(
            "\nthis is -strikethrough- in a string\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_link() {
        let input = "[link](https://example.com)";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 0).is_ok());
        assert_eq!(
            "\n[link|https://example.com]\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_image() {
        let input = "![img title](https://example.com/image.jpg)";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 0).is_ok());
        assert_eq!(
            "\n!https://example.com/image.jpg|title=\"img title\",alt=\"\"!\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_inline_code() {
        let input = "some `inline code` here";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 0).is_ok());
        assert_eq!(
            "\nsome {{inline code}} here\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_inline_code_trailing_char() {
        let input = "`inline`s content";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 0).is_ok());
        assert_eq!(
            "\n{{inline}} s content\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_horizontal_rule() {
        let input = "---";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 0).is_ok());
        assert_eq!("\n----\n", String::from_utf8(output).unwrap());
    }

    #[ignore] // doesn't work yet, weird parsing issues
    #[test]
    fn test_task_list() {
        let input = "\
        * [ ] task one\n\
        * [ ] task two\n\
        * [x] completed task";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 0).is_ok());
        assert_eq!(
            "\n\
                [] task one\n\
                [] task two\n\
                [x] completed task\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_modified_headings() {
        // header level 1 + 1 = 2
        let input = "# hello world";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 1).is_ok());
        assert_eq!("h2. hello world\n", String::from_utf8(output).unwrap());

        // header level 2 - 1 = 1
        let input = "## hello world";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), -1).is_ok());
        assert_eq!("h1. hello world\n", String::from_utf8(output).unwrap());

        // header level 1 - 1 = 0
        let input = "# hello world";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), -1).is_ok());
        assert_eq!("", String::from_utf8(output).unwrap());

        // header level 6 + 1 = 7
        let input = "###### hello world";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 1).is_ok());
        assert_eq!("hello world\n", String::from_utf8(output).unwrap());
    }

    #[test]
    fn test_modified_headings_with_inline() {
        // header level 1 - 1 = 0
        let input = "# hello world `inline code`";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), -1).is_ok());
        assert_eq!("", String::from_utf8(output).unwrap());
    }

    #[test]
    fn test_softbreak_newline() {
        // softbreak should be a space, not a newline
        let input = "new\nline";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 0).is_ok());
        assert_eq!("\nnew line\n", String::from_utf8(output).unwrap());
    }

    #[test]
    fn test_hardbreak_newline() {
        let input = "new  \nline";
        let mut output = Vec::new();
        assert!(write_jira(&mut output, Parser::new_ext(input, Options::all()), 0).is_ok());
        assert_eq!("\nnew\nline\n", String::from_utf8(output).unwrap());
    }

    #[test]
    fn test_toc() {
        let mut output = Vec::new();
        assert!(write_toc(&mut output).is_ok());
        assert_eq!("{toc}\n\n", String::from_utf8(output).unwrap());
    }
}
