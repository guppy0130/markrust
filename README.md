# markrust

Convert from Markdown to Atlassian markup.

## Usage

```console
$ ./markrust -h
markrust 2.1.0
guppy0130 <guppy0130@yahoo.com>
Converts Markdown to Atlassian markup

USAGE:
    markrust [OPTIONS] [ARGS]

ARGS:
    <INPUT>     FILE input, or empty for stdin
    <OUTPUT>    FILE output, or empty for stdout

OPTIONS:
    -e, --editor
            Launch $EDITOR as input

    -h, --help
            Print help information

    -l, --language <LANGUAGE>
            [default: confluence] [possible values: jira, confluence]

    -m, --modify-headers <MODIFY_HEADERS>
            Add N to header level (can be negative) [default: 0]

    -t, --toc
            Prepend TOC markup

    -V, --version
            Print version information
```

## Features

Compared to the Markdown converter that comes with Atlassian products:

* Code block macro with syntax highlighting
* Code block macro with automatic language mapping
  * Console -> bash, language aliases, etc.
* Automatic TOC markup (pass `-t` flag)
* Header level modifier (add/remove to header levels across document)
* Limited support for `details` and `summary` HTML elements

## Notes

* `markrust -e output` will launch your `$EDITOR` with `$TMP/markrust.md` as an
  argument, then when the editor returns, markrust will write the Atlassian
  markup to `output`.
  * You can only supply one path with the `-e` flag as a result
* Markdown content in `details` and `summary` will not be parsed, because once
  you're in HTML, only text will be kept as-is.

## Testing

* `cargo test`
* `cargo bench` - coming soon
* `make coverage` to compute coverage
  * if you're on Windows, you may want to run `setup_env.ps1` first.
