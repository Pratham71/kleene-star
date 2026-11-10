# 2. Nondeterminism and eps moves

A **nondeterministic finite automaton** (NFA) may have several moves for the
same state and symbol, or none at all. It accepts a string if *some* sequence of
choices ends in an accepting state. You can think of it as guessing correctly.

## An NFA

"The third symbol from the end is 1." A DFA would have to remember the last
three symbols. An NFA just guesses when the third-from-last symbol arrives.

```toc
alphabet B = {0, 1}

nfa ThirdFromEnd over B {
    start  a
    accept d
    a --0,1--> a
    a --1-->   b
    b --0,1--> c
    c --0,1--> d
}

print ThirdFromEnd
test ThirdFromEnd accepts "0100"
test not (ThirdFromEnd accepts "0010")
```

```text
NFA with 4 states over {0, 1}
      | 0   | 1
->  a | {a} | {a, b}
    b | {c} | {c}
    c | {d} | {d}
   *d | -   | -
tests: 2 passed, 0 failed
```

State `a` has two moves on `1`: stay in `a`, or guess that this is the one and
go to `b`. State `d` has no moves at all, shown as `-`. Neither is allowed in a
DFA; both are fine in an NFA.

## eps moves

An **eps move** (ε-transition) changes state without reading a symbol. Write
`eps` where a symbol would go.

This machine accepts strings of 0s whose length is a multiple of 2 or of 3. It
guesses which, before reading anything.

```toc
alphabet Z = {0}

nfa TwosOrThrees over Z {
    start  s
    accept two0, three0
    s --eps--> two0
    s --eps--> three0
    two0   --0--> two1
    two1   --0--> two0
    three0 --0--> three1
    three1 --0--> three2
    three2 --0--> three0
}

test TwosOrThrees accepts "0000"
test TwosOrThrees accepts "000"
test not (TwosOrThrees accepts "00000")
```

An NFA with eps moves is often called an **eps-NFA**. In TOC-Lang it is still
an `nfa`; the printed table gains an `eps` column.

## From NFA to DFA

Every NFA has an equivalent DFA. The **subset construction** builds it: each
DFA state is the *set* of NFA states the NFA could currently be in.

```toc
alphabet B = {0, 1}
nfa EndsIn01 over B {
    start  a
    accept c
    a --0,1--> a
    a --0-->   b
    b --1-->   c
}

let d = to_dfa(EndsIn01)
print d
test equivalent(d, EndsIn01)
```

```text
DFA with 3 states over {0, 1}
          | 0     | 1
->  {a}   | {a,b} | {a}
    {a,b} | {a,b} | {a,c}
   *{a,c} | {a,b} | {a}
tests: 1 passed, 0 failed
```

The state names show the construction at work: `{a,b}` means "the NFA is in `a`
or `b`". A state is accepting if its set contains an accepting NFA state.

`let d = ...` gives a name to a value. Names cannot be reassigned later.

## Removing eps moves

`remove_eps` gives an ordinary NFA for the same language.

```toc
alphabet Z = {0}
nfa TwosOrThrees over Z {
    start  s
    accept two0, three0
    s --eps--> two0
    s --eps--> three0
    two0   --0--> two1
    two1   --0--> two0
    three0 --0--> three1
    three1 --0--> three2
    three2 --0--> three0
}

let plain = remove_eps(TwosOrThrees)
print plain
test equivalent(plain, TwosOrThrees)
```

```text
NFA with 6 states over {0}
           | 0
-> *s      | {two1, three1}
    two1   | {two0}
    three1 | {three2}
   *two0   | {two1}
    three2 | {three0}
   *three0 | {three1}
tests: 1 passed, 0 failed
```

Notice that `s` became accepting: it could reach an accepting state by eps
moves alone, so the empty string must still be accepted.

## Making machines smaller

`minimize` returns the smallest DFA for a language. It is unique, so two
languages are equal exactly when their minimal DFAs have the same shape.

`reduce` shrinks an NFA by dropping useless states and merging equivalent ones.
It does not promise the smallest possible NFA, because finding that is
PSPACE-hard, which means no efficient method is known.

```toc
alphabet B = {0, 1}
nfa ThirdFromEnd over B {
    start  a
    accept d
    a --0,1--> a
    a --1-->   b
    b --0,1--> c
    c --0,1--> d
}

print states(ThirdFromEnd)
print states(to_dfa(ThirdFromEnd))
print states(minimize(ThirdFromEnd))
```

```text
4
8
8
```

Four NFA states need eight DFA states, and no smaller DFA exists. That gap
grows exponentially; [chapter 5](05-functions.md) measures it.

Next: [regular expressions](03-regex.md).
