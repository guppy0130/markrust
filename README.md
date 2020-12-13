# markrust

Convert from Markdown to Atlassian markup.

## Usage

```console
$ ./markrust -h
markrust 1.1.0
guppy0130 <guppy0130@yahoo.com>


USAGE:
    markrust [FLAGS] [OPTIONS] [ARGS]

FLAGS:
    -h, --help       Prints help information
    -t, --toc        Prepend TOC markup
    -V, --version    Prints version information

OPTIONS:
    -m, --modify-headers=<modify_headers>    add N to each header level

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
