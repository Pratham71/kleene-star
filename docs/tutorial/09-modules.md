# 9. Programs in several files

Once a course has given you a few machines worth keeping, such as a parity
checker or a family of "divisible by n" DFAs, you will want to use them
from more than one program without copying them. Kleene Star lets one file
**import** another. The file being imported is called a **module**.

The design borrows from Python: `import` brings in a whole module under a
name, and `from ... import` picks out single names. As in Rust, a module
only offers what it defines itself, and everything is checked before
anything runs.

## A module is an ordinary file

Any `.kstar` file can be a module. Here is one, `parity.kstar`:

```kstar file=parity.kstar
# Machines about parity, for other programs to import.
alphabet B = {0, 1}

# Words with an even number of 0s.
dfa EvenZeros over B {
    start  even
    accept even
    even --0--> odd
    even --1--> even
    odd  --0--> even
    odd  --1--> odd
}

fn ends_in(bit: Str) -> Regex {
    if bit == "0" {
        return /(0|1)*0/
    }
    return /(0|1)*1/
}

print "parity.kstar was run by itself"
test EvenZeros accepts "00"
```

Its top-level alphabets, machines, grammars, `let` values and functions are
its **names**, and every one of them can be used by a file that imports it.
There is no `pub` or `export`: everything is public.

## `import`: a module under a name

`import` takes a file name in quotes, written relative to the importing
file, and binds the module to a name: by default the file name without
`.kstar`. Its names are then written with a dot:

```kstar
import "parity.kstar"

print parity.EvenZeros accepts "0110"
print parity.ends_in("1") accepts "101"

# A qualified name works wherever a name does, even after `over`.
dfa AllOnes over parity.B {
    start  s
    accept s
    s --1--> s
    s --0--> dead
    dead --0,1--> dead
}
print equivalent(AllOnes, /1*/)
trace parity.EvenZeros on "010"
```

```text
true
true
true
Input: 010

Step  Symbol  From  To
0     —       —     even
1     0       even  odd
2     1       odd   odd
3     0       odd   even
Result: ACCEPTED
Reason: Ended in accepting state even
```

Notice what is *not* printed: the module's own `print`, and its `test`. When
a file is loaded as a module, its top-level `print`, `test`, `draw` and
`trace` statements are checked but not run, and its tests are not counted.
They are there for when you run `parity.kstar` by itself; importing it
should not fill your program's output with someone else's. A function from
the module behaves normally when you call it, though, prints included.

`as` chooses another name. That is needed when the file name is not a valid
name (`import "my-machines.kstar" as mine`), and handy when two modules have
the same file name:

```kstar
import "parity.kstar" as p
print p.EvenZeros accepts "000"
```

```text
false
```

## `from ... import`: single names

`from` puts chosen names straight into your file, so you can use them without
the module's name. `as` renames one:

```kstar
from "parity.kstar" import EvenZeros, ends_in as ending

print EvenZeros accepts "1001", ending("0") accepts "10"
```

```text
true true
```

There is no `from ... import *`: listing the names keeps every name in a
file traceable to the line that brought it in.

## Names that clash

An import cannot take a name that is already used in the file, and two
imports cannot take the same one. The fix is always `as`:

```kstar-error
let EvenZeros = 1
from "parity.kstar" import EvenZeros
```

```text
Error in demo.kstar, line 2:
    from "parity.kstar" import EvenZeros
                               ^^^^^^^^^
NameError[E0209]: `EvenZeros` is already defined in this file
  hint: import it under another name with `as`: `... as EvenZeros2`, or rename the other one
```

## When a module runs

A module is read and checked when it is imported, so a mistake anywhere in
it is reported before your program starts, even in a part you never use.
But its top level only **runs** the first time one of its names is used.
That is called *lazy* loading, and it means importing a large library you
barely use costs almost nothing.

Here is a module with a mistake that only shows when it runs:

```kstar file=risky.kstar
let Zero = 0
let Ratio = 10 % Zero
let Name = "risky"
```

Importing it is harmless; using it is not, and the traceback starts at the
line that first used it:

```kstar-error
import "risky.kstar"
print "before"
print risky.Name
```

```text
before
Traceback (most recent call last):
  demo.kstar, line 3, in <main>
    print risky.Name
  risky.kstar, line 2, in <module>
    let Ratio = 10 % Zero
                ^^^^^^^^^
MathError[E0601]: modulo by zero
  hint: the right-hand side of `%` is 0 here
```

`kstar run --eager-imports` runs every module at its `import` line instead,
which can be easier to reason about when you are debugging a module.

A module runs at most once, however many files import it, and it keeps its
own globals: a function always sees the names of the file it was written
in, not those of the file that calls it.

## Mistakes the checker catches

- A file that does not exist (paths are relative to the importing file).
- Files that import each other in a circle, shown as a chain:
  `a.kstar -> b.kstar -> a.kstar`.
- A name the module does not have, with a suggestion if you mistyped it:

```kstar-error
import "parity.kstar"
print parity.EvenZero accepts "00"
```

```text
Error in demo.kstar, line 2:
    print parity.EvenZero accepts "00"
                 ^^^^^^^^
NameError[E0208]: module `parity` has no `EvenZero`
  hint: did you mean `EvenZeros`?
```

- Reaching through a module to the modules *it* imported, as in
  `shapes.parity.B`. A module passes on only what it defines; import
  `parity.kstar` yourself.

An error inside a module is shown with that module's file name and line,
and a traceback can pass from file to file.

## The standard library

`std` is the standard library that comes with Kleene Star. Its modules are
reached by their full name with no import, as in `std.math.gcd(12, 18)`, or
imported like a file: `import std.math` (bound as `math`), `import std.math
as m`, or `from std.math import gcd`. The name `std` always means the
standard library, so nothing else can be called that. The modules are
described in [the reference](../reference.md#the-standard-library).

`std.math` fills in the arithmetic the language leaves out, such as
division, which comes in handy when building machine families:

```kstar
from std.math import div, gcd

# The numbers divisible by both 4 and 6 are the multiples of lcm(4, 6).
print std.math.lcm(4, 6), div(4 * 6, gcd(4, 6))
print std.math.is_prime(97), std.math.pow(2, 10)
```

```text
12 12
true 1024
```

## Summary

| You write | You get |
|---|---|
| `import "lib/parity.kstar"` | the module as `parity`; its names as `parity.EvenZeros` |
| `import "parity.kstar" as p` | the module as `p` |
| `from "parity.kstar" import EvenZeros, ends_in as ending` | `EvenZeros` and `ending` in your file |
| `std.math.gcd(4, 6)`, `import std.math` | the standard library |
