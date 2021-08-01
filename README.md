# markrust

Convert from Markdown to Atlassian markup.

## Usage

```console
$ ./markrust -h
markrust 1.5.0
guppy0130 <guppy0130@yahoo.com>
Converts Markdown to Atlassian markup

USAGE:
    markrust.exe [FLAGS] [ARGS]

FLAGS:
    -e, --editor            Launch $EDITOR as input
    -h, --help              Prints help information
    -m, --modify_headers    Add N to each header level. Can be negative
    -t, --toc               Prepend TOC markup
    -V, --version           Prints version information

ARGS:
    <input>     FILE input, or empty for stdin
    <output>    FILE output, or empty for stdout
```

## Features

Compared to the Markdown converter that comes with Atlassian products:

* Code block macro with syntax highlighting
* Code block macro with automatic language mapping
  * Console -> bash, language aliases, etc.
* Automatic TOC markup (pass `-t` flag)
* Header level modifier (add/remove to header levels across document)

## Notes

* `markrust -e output` will launch your `$EDITOR` with `$TMP/markrust.md` as an argument, then when the editor returns, write the Atlassian markup to `output`.
  * You can only supply one path with the `-e` flag as a result

## Testing

* `cargo test`
* `cargo bench` - coming soon
* `make coverage` to compute coverage
  * if you're on Windows, you may want to run `setup_env.ps1` first.
