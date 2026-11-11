# 1. Deterministic machines

A **deterministic finite automaton** (DFA) reads a string one symbol at a time.
It is always in exactly one state, and each symbol moves it to exactly one next
state. If it ends in an accepting state, the string is in its language.

## Alphabets

Every machine works over an alphabet: the set of symbols its strings are made of.

```kstar
alphabet B = {0, 1}
alphabet Letters = {a, b, c}

print B
print len(Letters)
```

```text
{0, 1}
3
```

A symbol is a single letter or digit.

## Your first DFA

This machine accepts binary strings that contain `11`. Its three states
remember how much of `11` has been seen so far.

```kstar
alphabet B = {0, 1}

dfa HasDouble1 over B {
    start  none
    accept both
    none --0--> none
    none --1--> one
    one  --0--> none
    one  --1--> both
    both --0,1--> both
}

print HasDouble1
```

```text
DFA with 3 states over {0, 1}
         | 0    | 1
->  none | none | one
    one  | none | both
   *both | both | both
```

Reading the definition:

- `start none` names the start state. There is exactly one.
- `accept both` names the accepting states. List several with commas.
- `none --1--> one` is a transition: in state `none`, reading `1`, go to `one`.
  It looks like the arrow you would draw.
- `both --0,1--> both` is two transitions written at once.

States are not declared separately. A state exists as soon as it is mentioned.

In the printed table, `->` marks the start state and `*` marks accepting states.

## Asking whether a string is accepted

`machine accepts "string"` is true or false.

```kstar
alphabet B = {0, 1}
dfa HasDouble1 over B {
    start  none
    accept both
    none --0--> none
    none --1--> one
    one  --0--> none
    one  --1--> both
    both --0,1--> both
}

print HasDouble1 accepts "0110"
print HasDouble1 accepts "0101"
print HasDouble1 accepts ""
```

```text
true
false
false
```

`""` is the empty string, usually written ε in textbooks.

## Tests

`print` shows a value. `test` checks one: it stays silent when the condition is
true and reports the line when it is false. At the end you get a count.

```kstar
alphabet B = {0, 1}
dfa HasDouble1 over B {
    start  none
    accept both
    none --0--> none
    none --1--> one
    one  --0--> none
    one  --1--> both
    both --0,1--> both
}

test HasDouble1 accepts "0110"
test not (HasDouble1 accepts "0101")
```

```text
tests: 2 passed, 0 failed
```

Write tests as you build a machine. They are the quickest way to find out that
a transition goes to the wrong place.

## A DFA must be complete

"Deterministic" means *exactly one* move for every state and symbol. Kleene Star
checks this before your program runs:

```kstar-error
alphabet B = {0, 1}

dfa Broken over B {
    start  q0
    accept q1
    q0 --0--> q1
    q1 --0,1--> q1
}
```

```text
Error in demo.kstar, line 3:
    dfa Broken over B {
    ^^^^^^^^^^^^^^^^^
MachineError: DFA `Broken` is not total: no transition from `q0` on `1`
  hint: a DFA needs a transition from every state on every symbol; add the missing ones (often to a dead state), or use `nfa`
```

The usual fix is a **dead state**: a non-accepting state that loops to itself
on every symbol.

```kstar
alphabet B = {0, 1}

dfa Fixed over B {
    start  q0
    accept q1
    q0 --0--> q1
    q0 --1--> dead
    q1 --0,1--> q1
    dead --0,1--> dead
}

test Fixed accepts "01"
test not (Fixed accepts "10")
```

Two transitions from the same state on the same symbol, or a symbol that is not
in the alphabet, are reported the same way.

## Drawing

`draw` writes a Graphviz file you can turn into a picture.

```kstar
alphabet B = {0, 1}
dfa Fixed over B {
    start  q0
    accept q1
    q0 --0--> q1
    q0 --1--> dead
    q1 --0,1--> q1
    dead --0,1--> dead
}

draw Fixed
```

```text
wrote Fixed.dot
```

If [Graphviz](https://graphviz.org) is installed, `kstar` also writes `Fixed.svg`.
Otherwise paste the contents of the `.dot` file into any online Graphviz viewer.

Next: [nondeterminism and eps moves](02-nfa.md).
