# 8. Turing machines

A **Turing machine** is a finite automaton with an unlimited tape it can read,
write, and move along in both directions. That small change makes it as
powerful as any computer: anything a program can compute, a Turing machine
can compute.

## The machine

The input starts on the tape with the head on its first symbol; every other
cell holds the **blank**, written `_`. Each step:

1. reads the symbol under the head,
2. writes a symbol in its place (possibly the same one),
3. moves the head one cell left (`L`), right (`R`), or not at all (`S`),
4. changes state.

A move is written `read/write,move`:

```text
q0 --0/1,R--> q1        read 0, write 1, move right, go to q1
```

The machine **halts** when it enters an `accept` state or a `reject` state.
It also halts, rejecting, when there is no move for the current state and
symbol.

## A first machine: binary increment

This machine adds one to a binary number. It walks to the right end, then
works back to the left turning trailing `1`s into `0`s until it can turn a
`0` (or a blank) into a `1`.

```kstar
alphabet B = {0, 1}

tm Increment over B {
    start  right
    accept done
    right --0/0,R--> right
    right --1/1,R--> right
    right --_/_,L--> carry
    carry --1/0,L--> carry
    carry --0/1,S--> done
    carry --_/1,S--> done
}

print run(Increment, "1011")
print run(Increment, "111")
```

```text
1100
1000
```

`run(M, w)` runs the machine on `w` and returns what is left on the tape,
without the blanks at either end.

## Watching it work

`trace` shows every step: the move taken, the state after it, and the tape
with the head's cell in brackets.

```kstar
alphabet B = {0, 1}
tm Increment over B {
    start  right
    accept done
    right --0/0,R--> right
    right --1/1,R--> right
    right --_/_,L--> carry
    carry --1/0,L--> carry
    carry --0/1,S--> done
    carry --_/1,S--> done
}
trace Increment on "11"
```

```text
Input: 11

Step  Move   State  Tape
0     —      right  [1]1
1     1/1,R  right  1[1]
2     1/1,R  right  11[_]
3     _/_,L  carry  1[1]
4     1/0,L  carry  [1]0
5     1/0,L  carry  [_]00
6     _/1,S  done   [1]00
Result: ACCEPTED
Reason: Entered accepting state done
Tape: 100
```

## Deciding a language: a<sup>n</sup>b<sup>n</sup>c<sup>n</sup>

No PDA can recognise a<sup>n</sup>b<sup>n</sup>c<sup>n</sup>, because one
stack cannot keep two counts. A Turing machine can: it crosses off one `a`,
one `b` and one `c` per pass, marking them `X`, `Y` and `Z`, until nothing is
left.

Symbols the machine writes, like `X`, do not need to be in the alphabet: the
alphabet is only what the *input* may contain.

```kstar
alphabet ABC = {a, b, c}

tm Abc over ABC {
    start  find_a
    accept yes
    reject no
    # Cross off an a, or check that only crossed-off symbols are left.
    find_a --a/X,R--> find_b
    find_a --Y/Y,R--> check
    find_a --_/_,S--> yes
    find_a --b/b,S--> no
    find_a --c/c,S--> no
    # Skip to the first b and cross it off.
    find_b --a/a,R--> find_b
    find_b --Y/Y,R--> find_b
    find_b --b/Y,R--> find_c
    find_b --c/c,S--> no
    find_b --_/_,S--> no
    # Skip to the first c and cross it off.
    find_c --b/b,R--> find_c
    find_c --Z/Z,R--> find_c
    find_c --c/Z,L--> back
    find_c --a/a,S--> no
    find_c --_/_,S--> no
    # Go back to the leftmost a that is still there.
    back   --a/a,L--> back
    back   --b/b,L--> back
    back   --Y/Y,L--> back
    back   --Z/Z,L--> back
    back   --X/X,R--> find_a
    # Only Y and Z may remain.
    check  --Y/Y,R--> check
    check  --Z/Z,R--> check
    check  --_/_,S--> yes
    check  --a/a,S--> no
    check  --b/b,S--> no
    check  --c/c,S--> no
}

for w in {"", "abc", "aabbcc", "aabbc", "abcabc", "acb"} {
    print w, Abc accepts w
}
print run(Abc, "aabbcc")
```

```text
 true
aabbc false
aabbcc true
abc true
abcabc false
acb false
XXYYZZ
```

## Machines that never stop

A Turing machine can run forever, and no program can tell in general whether
it will: that is the **halting problem**. So Kleene Star runs a machine for at
most 100 000 steps (100 for `trace`). If it has not halted by then, you get
an error that says so, instead of a wrong answer or a frozen program:

```kstar-error
alphabet B = {0, 1}
tm Forever over B {
    start  walk
    accept never
    walk --0/0,R--> walk
    walk --1/1,R--> walk
    walk --_/_,R--> walk
}
print Forever accepts "01"
```

```text
Traceback (most recent call last):
  demo.kstar, line 9, in <main>
    print Forever accepts "01"
                          ^^^^
LimitError[E0604]: the Turing machine is still running after 100000 steps
  hint: it may never halt. By the halting problem, no program can decide in general whether a Turing machine halts, so Kleene Star stops here instead of waiting forever
```

For the same reason, `equivalent` refuses Turing machines: whether two of
them accept the same language is undecidable.

## Rules

- One `start` state; any number of `accept` and `reject` states, but no state
  can be both.
- One move per state and symbol: these machines are deterministic.
- `_` is the blank. It cannot be in the input alphabet.
- `trace`, `accepts`, `run`, `states`, `print` and `draw` all work on a `Tm`.
