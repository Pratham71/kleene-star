# 3. Regular expressions

A **regular expression** describes a language by its shape instead of by a
machine. Regexes, NFAs and DFAs describe exactly the same class of languages,
the regular languages, and Kleene Star converts freely between them.

## Writing a regex

A regex goes between slashes.

```kstar
let r = /(0|1)*01/

print r accepts "1101"
print r accepts "10"
```

```text
true
false
```

| You write | Meaning |
|---|---|
| `a` | the symbol `a` (any letter or digit) |
| `ab` | `a` followed by `b` |
| `a\|b` | `a` or `b` |
| `a*` | zero or more `a` |
| `a+` | one or more `a` |
| `a?` | `a` or nothing |
| `(ab)*` | parentheses group |
| `.` | any one symbol of the alphabet |
| `()` | the empty string |
| `[]` | the empty language: matches nothing |

`*`, `+` and `?` bind tightest, then concatenation, then `|`. So `ab*|c` means
`(a(b*))|c`. Spaces inside a regex are ignored.

## Regex to machine

`to_nfa` uses **Thompson's construction**: one small fragment per operator,
joined with eps moves. It is simple but produces many states. `to_dfa` then
applies the subset construction, and `minimize` finds the smallest DFA.

```kstar
let r = /(a|b)*abb/

let n = to_nfa(r)
let d = to_dfa(n)
let m = minimize(d)

print "eps-NFA:", states(n), "states"
print "DFA:    ", states(d), "states"
print "min-DFA:", states(m), "states"
```

```text
eps-NFA: 14 states
DFA:     5 states
min-DFA: 4 states
```

`print` takes several values separated by commas and puts spaces between them.

## Readable state names

Converted machines have descriptive but long state names such as `{q1,q3,q7}`.
`rename` replaces them with `q0, q1, ...`, numbered outward from the start.

```kstar
let m = rename(minimize(/(a|b)*abb/))
print m
```

```text
DFA with 4 states over {a, b}
       | a  | b
->  q0 | q1 | q0
    q1 | q1 | q2
    q2 | q1 | q3
   *q3 | q1 | q0
```

Most functions accept any description of a language, so `minimize` took the
regex directly here and did the conversions itself.

## Machine to regex

`to_regex` works by **state elimination**: it removes states one at a time,
replacing the paths through each with a regex.

```kstar
alphabet B = {0, 1}

dfa EvenZeros over B {
    start  even
    accept even
    even --0--> odd
    even --1--> even
    odd  --0--> even
    odd  --1--> odd
}

let r = to_regex(EvenZeros)
print r
test equivalent(r, EvenZeros)
```

```text
/(1|01*0)*/
tests: 1 passed, 0 failed
```

Read it as: any number of pieces, each either a `1` or a pair of `0`s with 1s
between them. That is exactly "an even number of 0s".

The regex that comes out is correct but not always the shortest. Round trips
are a good exercise: convert, and use `equivalent` to confirm nothing changed.

## Which alphabet does a regex use?

A regex on its own uses just the symbols written in it. When it is combined
with a machine, it adopts the machine's alphabet. You can also choose one.

```kstar
alphabet B = {0, 1}

print to_dfa(/0*/)
print to_dfa(/0*/, B)
```

```text
DFA with 2 states over {0}
               | 0
-> *{q0,q1,q2} | {q1,q2,q3}
   *{q1,q2,q3} | {q1,q2,q3}
DFA with 3 states over {0, 1}
               | 0          | 1
-> *{q0,q1,q2} | {q1,q2,q3} | {}
   *{q1,q2,q3} | {q1,q2,q3} | {}
    {}         | {}         | {}
```

Over `{0, 1}` the DFA needs a dead state, named `{}` because it is the empty
set of NFA states, to swallow any `1`.

Next: [combining and comparing languages](04-closure.md).
