# Appendix: tools

The `toc` command does more than run programs. This page covers the tools
that help you write them.

## Formatting with `toc fmt`

People lay code out differently, and a TOC program does not care: line breaks
and spaces are not significant. But code is easier to read when it always
looks the same. `toc fmt` rewrites a file in the standard style, so you never
have to line up arrows by hand.

Suppose `messy.toc` contains:

```text
alphabet B={0,1}
dfa HasDouble1 over B{
start none
  accept both
none--0-->none
none --1--> one
	one --0--> none   # back to the beginning
one --1-->both


both --0,1--> both}
```

Running

```bash
toc fmt messy.toc
```

rewrites it as:

```toc
alphabet B = {0, 1}
dfa HasDouble1 over B {
    start  none
    accept both
    none --0  --> none
    none --1  --> one
    one  --0  --> none  # back to the beginning
    one  --1  --> both

    both --0,1--> both
}
```

The style:

- four spaces of indentation per `{ ... }`, one statement per line;
- single spaces around operators (`a + b`, `w accepts "01"`), none around
  `..` (`0..n`), and `, ` between items;
- inside a machine, `start` and `accept` line up, and the transitions are
  padded so that every `--` and every `-->` sits in the same column;
- comments stay where they were; comments at the ends of neighbouring lines
  are lined up;
- several blank lines in a row become one, and blank lines just inside
  `{` or `}` disappear;
- parentheses are kept exactly as you wrote them, and so are numbers,
  strings and regexes.

Formatting never changes what a program does. As a safety check, `toc fmt`
compares the program before and after, token by token, and refuses to write
anything if they differ.

To find files that are not formatted yet, without changing them, use
`--check`. It prints the name of every file that would change and exits with
status 1, which makes it easy to use in a script or CI:

```bash
toc fmt --check examples/*.toc
```

A file must be free of syntax errors to be formatted; otherwise `toc fmt`
shows the error and leaves the file alone. Type errors do not matter: the
formatter only reads the program, it does not check it.

One thing to know: a comment in the middle of a statement that spans several
lines, such as between the items of a long set, moves to the line after
that statement, because the formatter writes each statement on one line.
