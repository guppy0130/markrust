# markrust

Convert from Markdown to Atlassian markup.

## Usage

```console
$ ./markrust -h
markrust 1.2.2
guppy0130 <guppy0130@yahoo.com>
Converts Markdown to Atlassian markup

USAGE:
    markrust.exe [FLAGS] [OPTIONS] [ARGS]

FLAGS:
    -e, --editor     Launch $EDITOR as input
    -h, --help       Prints help information
    -t, --toc        Prepend TOC markup
    -V, --version    Prints version information

OPTIONS:
    -m, --modify-headers=<modify_headers>    add N to each header level. Can be negative

ARGS:
    <input>     FILE input, or empty for stdin
    <output>    FILE output, or empty for stdout
```

## Features

Compared to the default Markdown converter

* Code block macro with syntax highlighting
* Code block macro with automatic language mapping
  * Console -> bash, language aliases, etc.
* Automatic TOC markup (pass `-t` flag)
* Header level modifier (add/remove to header levels across document)

## Notes

* `markrust -e output` will launch your `$EDITOR` with `$TMP/markrust.md` as an argument, then when the editor returns, write the Atlassian markup to `output`.
  * You can only supply one path with the `-e` flag as a result
