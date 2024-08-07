use ego_tree::NodeRef;
use markup5ever::local_name;
use pulldown_cmark::*;
use scraper::{Html, Node};
use std::collections::HashMap;
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
    lang_map
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

    escape_map
}

/// The JiraWriter takes events from pulldown-cmark and formats it into Atlassian markup
struct AtlassianWriter<I, W> {
    iter: I,
    writer: W,
    // if we ended on a newline so we can fix newlines for lists
    end_newline: bool,
    // if we're on a table header cell
    table_header: bool,
    // what bullets we're working with
    bullet_stack: Vec<u8>,
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
    // jira or confluence
    flavor: char,
    cached_html_content: String,
    // cache the url for links because we need to put the text first
    dest_url: String,
}

impl<'a, I, W> AtlassianWriter<I, W>
where
    I: Iterator<Item = Event<'a>>,
    W: Write,
{
    /// return a new AtlassianWriter
    ///
    /// # Arguments
    ///
    /// * `iter` - iterator of elements provided by `pulldowm_cmark`
    /// * `writer` - something implementing Write to write output to
    /// * `modify_headers` - int to increment/decrement headers by
    /// * `flavor` - j or c for jira or confluence, respectively
    fn new(iter: I, writer: W, modify_headers: i8, flavor: char) -> Self {
        // confluence/jira only implements the following language highlighting
        // doing this now means the cost is 1 instead of N
        AtlassianWriter {
            iter,
            writer,
            end_newline: false,
            table_header: false,
            bullet_stack: vec![],
            inline_code: false,
            lang_map: build_lang_map(),
            modify_headers,
            should_output_line: true,
            escape_map: make_escape_list(),
            flavor,
            cached_html_content: "".to_string(),
            dest_url: "".to_string(),
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
            self.end_newline = s.ends_with('\n');
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
        if r.starts_with('-') {
            r.replace_range(0..1, "\\-");
        }
        self.write(&r)
    }

    /// Parses HTML to Atlassian markup
    ///
    /// # Arguments
    ///
    /// * `node` - node to parse
    fn parse_html(&mut self, node: Option<NodeRef<Node>>) -> io::Result<()> {
        match node {
            // if there's no node to check, you've hit a leaf, so you're done here
            None => Ok(()),
            Some(n) => match n.value() {
                Node::Element(elem) => {
                    // we might need to skip parsing the child, because otherwise we get two
                    // summary texts.
                    let mut already_parsed = false;
                    match elem.name.local {
                        local_name!("details") => {
                            self.write("{expand")?;
                            // figure out if there is a summary amongst the children
                            // if so, we should not write the ending curly brace.
                            let mut should_write = true;
                            for next_child in n.children() {
                                // if the next scraper node elem is an element, and if it is a
                                // summary element, we should not write the closing curly; the
                                // summary handler will do that for us
                                if let Node::Element(next_elem) = next_child.value() {
                                    if matches!(next_elem.name.local, local_name!("summary")) {
                                        should_write = false;
                                    }
                                }
                            }
                            // if there is no summary children, write the curly brace now
                            if should_write {
                                self.write("}\n")?;
                            }
                        }
                        local_name!("summary") => {
                            self.write("|title=")?;
                            self.parse_html(n.first_child())?;
                            self.write("}\n")?;
                            // we don't need to parse the first child again
                            already_parsed = true;
                        }
                        _ => (),
                    }
                    // if the next child is not yet parsed (wasn't a summary), parse it
                    if !already_parsed {
                        self.parse_html(n.first_child())?;
                    }
                    // close off the expand tag
                    if matches!(elem.name.local, local_name!("details")) {
                        self.write("\n{expand}\n")?;
                    }
                    // parse the rest of the elements
                    self.parse_html(n.next_sibling())
                }
                Node::Text(text) => {
                    // strip some noise
                    let str_text = text.trim_start_matches('\n').trim_start_matches(' ');
                    self.write_escaped(str_text)?;
                    self.parse_html(n.next_sibling())
                }
                Node::Fragment => return self.parse_html(n.first_child()),
                // we don't care about comments, because those shouldn't make it to the output
                // we won't have a document, because we're generating/parsing fragments only
                _ => Ok(()),
            },
        }
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
                    if self.inline_code && !text.starts_with(' ') {
                        // put a space after ending double curly brace
                        self.write(" ")?;
                        self.inline_code = false;
                    }
                    self.write(&text)?;
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
                Event::Html(string) => {
                    self.cached_html_content += &string;
                    // attempt to parse it. if it fails, we don't have a complete fragment yet.
                    // this approach is highly naive and unoptimized!
                    let parsed_html = Html::parse_fragment(&self.cached_html_content);
                    if parsed_html.errors.is_empty() {
                        // parse
                        self.parse_html(Some(parsed_html.tree.root()))?;
                        // clear the cached HTML content
                        self.cached_html_content = String::new()
                    }
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
            Tag::Heading { level, .. } => {
                if self.end_newline {
                    self.write_newline()?;
                }
                let mut parsed_level = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
                parsed_level += self.modify_headers;
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
            Tag::BlockQuote(_) => {
                // TODO: handle block_quote_kind later
                self.write_newline()?;
                self.write("{quote}")
            }
            Tag::CodeBlock(code_block_kind) => {
                self.write_newline()?;
                self.write("{code")?;
                if let CodeBlockKind::Fenced(language) = code_block_kind {
                    let default = "text".to_string();
                    let lang = self
                        .lang_map
                        .get(&language.to_string())
                        .unwrap_or(&default)
                        .clone();
                    match self.flavor {
                        'j' => self.write(&format!(":{}", &lang))?,
                        'c' => self.write(&format!(":language={}", &lang))?,
                        // panic if we don't know which flavor
                        _ => panic!("Unknown atlassian markup flavor"),
                    }
                }
                // skipping 4-space indented type
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
            Tag::Link { dest_url, .. } => {
                self.dest_url = dest_url.to_string();
                self.write("[")
            }
            Tag::Image { dest_url, .. } => self.write(&format!(r#"!{}|title=""#, dest_url)),
            _ => Ok(()),
        }
    }

    /// Handles closing tags
    ///
    /// # Arguments
    ///
    /// * `tag` - tag to close
    fn end_tag(&mut self, tag: TagEnd) -> io::Result<()> {
        match tag {
            TagEnd::Paragraph => self.write_newline(),
            TagEnd::Heading(..) => {
                if !self.should_output_line {
                    self.should_output_line = true;
                    Ok(())
                } else {
                    self.write_newline()
                }
            }
            TagEnd::BlockQuote => {
                self.write("{quote}")?;
                self.write_newline()
            }
            TagEnd::CodeBlock => {
                self.write("{code}")?;
                self.write_newline()
            }
            TagEnd::List(_) => {
                self.bullet_stack.pop();
                if self.bullet_stack.is_empty() {
                    self.write_newline()
                } else {
                    Ok(())
                }
            }
            TagEnd::TableHead => {
                self.table_header = false;
                self.write_newline()
            }
            TagEnd::TableRow => self.write_newline(),
            TagEnd::TableCell => {
                if self.table_header {
                    self.write("||")
                } else {
                    self.write("|")
                }
            }
            TagEnd::Emphasis => self.write("_"),
            TagEnd::Strong => self.write("*"),
            TagEnd::Strikethrough => self.write("-"),
            TagEnd::Link => self.write(&format!("|{}]", self.dest_url)),
            TagEnd::Image => self.write(r#"",alt=""!"#), // TODO: handle this better
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
pub fn write<'a, I, W>(writer: W, iter: I, modify_headers: i8, flavor: char) -> io::Result<()>
where
    I: Iterator<Item = Event<'a>>,
    W: Write,
{
    AtlassianWriter::new(iter, writer, modify_headers, flavor).run()
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
pub fn write_toc<W>(mut writer: W) -> io::Result<()>
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
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'j').is_ok());
        assert_eq!("h1. hello world\n", String::from_utf8(output).unwrap());

        let input = "## hello world";
        let mut output = Vec::new();
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'j').is_ok());
        assert_eq!("h2. hello world\n", String::from_utf8(output).unwrap());
    }

    #[test]
    fn test_blockquote() {
        let input = "> hello blockquote";
        let mut output = Vec::new();
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'j').is_ok());
        assert_eq!(
            "\n\
                {quote}\n\
                hello blockquote\n\
                {quote}\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_codeblock_jira() {
        let input = "\
        ```java\n\
        System.out.println(\"hello world\")\n\
        ```";
        let mut output = Vec::new();
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'j').is_ok());
        assert_eq!(
            "\n\
                {code:java}\n\
                System.out.println(\"hello world\")\n\
                {code}\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_codeblock_confluence() {
        let input = "\
        ```java\n\
        System.out.println(\"hello world\")\n\
        ```";
        let mut output = Vec::new();
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'c').is_ok());
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
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'j').is_ok());
        assert_eq!(
            "\n\
                {code:bash}\n\
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
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'j').is_ok());
        assert_eq!(
            "\n\
                {code:text}\n\
                should be text\n\
                {code}\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_nested_markup_inline_code() {
        let input = "`inline code with an asterisk *` like `rm -rf ./*.extension`";
        let mut output = Vec::new();
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'j').is_ok());
        assert_eq!(
            "\n{{inline code with an asterisk \\*}} like {{rm -rf ./\\*.extension}}\n",
            String::from_utf8(output).unwrap()
        );
        let input = "a flag like `-r`";
        let mut output = Vec::new();
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'j').is_ok());
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
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'j').is_ok());
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
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'j').is_ok());
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
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'j').is_ok());
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
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'j').is_ok());
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
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'j').is_ok());
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
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'j').is_ok());
        assert_eq!(
            "\nthis is _italics_ in a string\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_bold() {
        let input = "this is **bold** in a string";
        let mut output = Vec::new();
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'j').is_ok());
        assert_eq!(
            "\nthis is *bold* in a string\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_bold_italics() {
        let input = "this is _**bold italics**_ in a string";
        let mut output = Vec::new();
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'j').is_ok());
        assert_eq!(
            "\nthis is _*bold italics*_ in a string\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_strikethrough() {
        let input = "this is ~~strikethrough~~ in a string";
        let mut output = Vec::new();
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'j').is_ok());
        assert_eq!(
            "\nthis is -strikethrough- in a string\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_link() {
        let input = "[link](https://example.com)";
        let mut output = Vec::new();
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'j').is_ok());
        assert_eq!(
            "\n[link|https://example.com]\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_image() {
        let input = "![img title](https://example.com/image.jpg)";
        let mut output = Vec::new();
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'j').is_ok());
        assert_eq!(
            r#"
!https://example.com/image.jpg|title="img title",alt=""!
"#,
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_inline_code() {
        let input = "some `inline code` here";
        let mut output = Vec::new();
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'j').is_ok());
        assert_eq!(
            "\nsome {{inline code}} here\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_inline_code_trailing_char() {
        let input = "`inline`s content";
        let mut output = Vec::new();
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'j').is_ok());
        assert_eq!(
            "\n{{inline}} s content\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_horizontal_rule() {
        let input = "---";
        let mut output = Vec::new();
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'j').is_ok());
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
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'j').is_ok());
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
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 1, 'j').is_ok());
        assert_eq!("h2. hello world\n", String::from_utf8(output).unwrap());

        // header level 2 - 1 = 1
        let input = "## hello world";
        let mut output = Vec::new();
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), -1, 'j').is_ok());
        assert_eq!("h1. hello world\n", String::from_utf8(output).unwrap());

        // header level 1 - 1 = 0
        let input = "# hello world";
        let mut output = Vec::new();
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), -1, 'j').is_ok());
        assert_eq!("", String::from_utf8(output).unwrap());

        // header level 6 + 1 = 7
        let input = "###### hello world";
        let mut output = Vec::new();
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 1, 'j').is_ok());
        assert_eq!("hello world\n", String::from_utf8(output).unwrap());
    }

    #[test]
    fn test_modified_headings_with_inline() {
        // header level 1 - 1 = 0
        let input = "# hello world `inline code`";
        let mut output = Vec::new();
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), -1, 'j').is_ok());
        assert_eq!("", String::from_utf8(output).unwrap());
    }

    #[test]
    fn test_softbreak_newline() {
        // softbreak should be a space, not a newline
        let input = "new\nline";
        let mut output = Vec::new();
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'j').is_ok());
        assert_eq!("\nnew line\n", String::from_utf8(output).unwrap());
    }

    #[test]
    fn test_hardbreak_newline() {
        let input = "new  \nline";
        let mut output = Vec::new();
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'j').is_ok());
        assert_eq!("\nnew\nline\n", String::from_utf8(output).unwrap());
    }

    #[test]
    fn test_toc() {
        let mut output = Vec::new();
        assert!(write_toc(&mut output).is_ok());
        assert_eq!("{toc}\n\n", String::from_utf8(output).unwrap());
    }

    #[test]
    fn test_details_no_summary() {
        let input = "<details>Content</details>";
        let mut output = Vec::new();
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'c').is_ok());
        assert_eq!(
            "{expand}\nContent\n{expand}\n",
            String::from_utf8(output).unwrap()
        );
    }

    #[test]
    fn test_details_with_summary() {
        let input = "<details><summary>Summary</summary>Content</details>";
        let mut output = Vec::new();
        assert!(write(&mut output, Parser::new_ext(input, Options::all()), 0, 'c').is_ok());
        assert_eq!(
            "{expand|title=Summary}\nContent\n{expand}\n",
            String::from_utf8(output).unwrap()
        );
    }
}
