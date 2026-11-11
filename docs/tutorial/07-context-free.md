# 7. Grammars and pushdown automata

Some languages are beyond every finite automaton. The classic one is
a<sup>n</sup>b<sup>n</sup>: some `a`s followed by the same number of `b`s. A
DFA has only finitely many states, so it cannot count arbitrarily high. Two
new tools can: **context-free grammars**, which generate words, and
**pushdown automata**, which recognise them with the help of a stack.

## Context-free grammars

A grammar is a set of rewriting rules. This one generates
a<sup>n</sup>b<sup>n</sup>:

```kstar
alphabet AB = {a, b}

grammar Equal over AB {
    S -> a S b | eps
}

print Equal
print Equal accepts "aaabbb", Equal accepts "aab"
```

```text
Grammar with 1 variable over {a, b}, start S
S -> a S b | eps
true false
```

Reading the definition:

- `S` is a **variable**. Variables start with a capital letter: `S`, `A`,
  `Expr`.
- `a` and `b` are **terminals**: symbols of the alphabet after `over`.
- `S -> a S b | eps` says `S` can be rewritten as `a S b`, or (`|`) as `eps`,
  the empty string.
- The variable of the **first** rule is the start variable.

Symbols in a rule are separated by spaces, so `a S b` is three symbols and
`aSb` is a mistake:

```kstar-error
alphabet AB = {a, b}
grammar G over AB {
    S -> aSb | eps
}
```

```text
Error in demo.kstar, line 3:
    S -> aSb | eps
         ^^^
SyntaxError[E0108]: `aSb` is not one symbol
  hint: write terminals one at a time with spaces between: `a S b`; variables start with a capital letter
```

Every variable used on a right-hand side needs rules of its own, and every
terminal must be in the alphabet. Both are checked before the program runs.

### Derivations

A **derivation** shows how the start variable becomes a word, one rule at a
time. `derive(g, word)` finds a *leftmost* derivation, where each step
rewrites the leftmost variable, and returns it as a string:

```kstar
alphabet AB = {a, b}
grammar Equal over AB {
    S -> a S b | eps
}
print derive(Equal, "aabb")
```

```text
S
=> a S b
=> a a S b b
=> a a b b
```

Asking for a word the grammar cannot make is an error, not a silent `false`:

```kstar-error
alphabet AB = {a, b}
grammar Equal over AB {
    S -> a S b | eps
}
print derive(Equal, "abb")
```

```text
Traceback (most recent call last):
  demo.kstar, line 5, in <main>
    print derive(Equal, "abb")
                        ^^^^^
ValueError[E0606]: the grammar cannot derive "abb"
  hint: check first with `g accepts "..."`; only words in the language have a derivation
```

### Chomsky normal form and membership

`g accepts w` is answered with the **CYK algorithm**, which needs the grammar
in **Chomsky normal form** (CNF): every rule is `A -> B C` (two variables) or
`A -> a` (one terminal), and only the start variable may have `eps`.
`to_cnf(g)` shows the converted grammar:

```kstar
alphabet LR = {l, r}

# Balanced brackets, written with l and r.
grammar Balanced over LR {
    S -> S S | l S r | eps
}

print to_cnf(Balanced)
print Balanced accepts "llrlrr", Balanced accepts "lrrl"
```

```text
Grammar with 5 variables over {l, r}, start S0
S0  -> S S | T_l X1 | eps
S   -> S S | T_l X1
T_l -> l
X1  -> r | S T_r
T_r -> r
true false
```

The conversion adds a new start variable `S0`, gives each terminal its own
variable (`T_l -> l`), splits long right-hand sides with new variables (`X1`),
removes `eps` rules and unit rules like `A -> B`, and drops variables that can
never finish.

CYK fills a table: for every piece of the word, which variables can generate
it. Single symbols come from rules `A -> a`; longer pieces are split in every
possible place and combined with rules `A -> B C`. The word is accepted if the
start variable can generate the whole of it.

### Empty grammars

`is_empty(g)` asks whether a grammar generates no words at all:

```kstar
alphabet AB = {a, b}
grammar Endless over AB {
    S -> a S
}
print is_empty(Endless)
```

```text
true
```

`S` can only ever produce more `a`s followed by another `S`; it never reaches
a word made of terminals alone.

## Pushdown automata

A **pushdown automaton** (PDA) is an NFA with a stack. Each move reads an
input symbol (or nothing), pops a symbol off the stack (or nothing), and
pushes a string of symbols. It is written with the same arrows as an NFA,
with the stack action after the input symbol:

```text
q0 --a, Z/AZ--> q1        read a, pop Z, push AZ (A ends up on top)
```

Any of the three parts can be `eps`. The stack starts holding a single `Z`,
so a PDA can tell when it has reached the bottom. Stack symbols are letters
and digits written together, top first.

```kstar
alphabet AB = {a, b}

# Push an A for every a; then pop one A for every b.
pda AnBn over AB {
    start  push
    accept done
    push --a, eps/A    --> push
    push --eps, eps/eps--> pop
    pop  --b, A/eps    --> pop
    pop  --eps, Z/Z    --> done
}

print AnBn
for w in {"ab", "aabb", "abb"} {
    print w, AnBn accepts w
}
```

```text
PDA with 3 states over {a, b}, stack starts as Z
start push, accept done
From | Read | Pop | Push | To
push | a    | eps | A    | push
push | eps  | eps | eps  | pop
pop  | b    | A   | eps  | pop
pop  | eps  | Z   | Z    | done
aabb true
ab true
abb false
```

A PDA here **accepts by final state**: a word is accepted if some sequence of
moves reads all of it and ends in an accepting state. What is left on the
stack does not matter. (Textbooks also define acceptance by empty stack; the
two recognise the same languages.)

Like an NFA, a PDA may have several moves to choose from. Kleene Star tries
every possibility, breadth first. Because eps moves can push forever, the
search stops after 50 000 configurations, or when the stack would grow past
1 000 symbols, and then reports that it does not know the answer instead of
guessing:

```kstar-error
alphabet AB = {a, b}
pda Forever over AB {
    start  p
    accept q
    p --eps, eps/A--> p
}
print Forever accepts "ab"
```

```text
Traceback (most recent call last):
  demo.kstar, line 7, in <main>
    print Forever accepts "ab"
                          ^^^^
LimitError[E0605]: gave up after looking at 50000 configurations of the PDA without finding an accepting run
  hint: eps moves that push without reading can let a PDA run forever; the answer is unknown, not "no"
```

### Tracing a PDA

`trace` shows a run: the move taken, the state, the input still to read, and
the stack, top first.

```kstar
alphabet AB = {a, b}
pda AnBn over AB {
    start  push
    accept done
    push --a, eps/A    --> push
    push --eps, eps/eps--> pop
    pop  --b, A/eps    --> pop
    pop  --eps, Z/Z    --> done
}
trace AnBn on "aabb"
trace AnBn on "abb"
```

```text
Input: aabb

Step  Move          State  Input  Stack
0     —             push   aabb   Z
1     a, eps/A      push   abb    AZ
2     a, eps/A      push   bb     AAZ
3     eps, eps/eps  pop    bb     AAZ
4     b, A/eps      pop    b      AZ
5     b, A/eps      pop    —      Z
6     eps, Z/Z      done   —      Z
Result: ACCEPTED
Reason: Read all of the input and reached accepting state done

Input: abb

Step  Move          State  Input  Stack
0     —             push   abb    Z
1     a, eps/A      push   bb     AZ
2     eps, eps/eps  pop    bb     AZ
3     b, A/eps      pop    b      Z
Result: REJECTED
Reason: No run reads all of the input and ends in an accepting state; the run shown reads the most (2 of 3 symbols)
```

For an accepted word the run shown is a shortest accepting run. For a
rejected word no run works, so the trace shows one that got furthest through
the input.

## Converting

| From | To | Built-in |
|---|---|---|
| grammar | grammar in CNF | `to_cnf(g)` |
| grammar | PDA | `to_pda(g)` |
| regular language | grammar | `to_grammar(lang)` |

`to_pda(g)` builds the standard three-state PDA that simulates leftmost
derivations: the stack holds what is still to be matched. A variable on top is
replaced by one of its right-hand sides (an eps move); a terminal on top is
matched against the input.

```kstar
alphabet AB = {a, b}
grammar Pal over AB {
    S -> a S a | b S b | a | b | eps
}
trace to_pda(Pal) on "aba"
```

```text
Input: aba

Step  Move        State   Input  Stack
0     —           start   aba    Z
1     eps, Z/SZ   loop    aba    SZ
2     eps, S/aSa  loop    aba    aSaZ
3     a, a/eps    loop    ba     SaZ
4     eps, S/b    loop    ba     baZ
5     b, b/eps    loop    a      aZ
6     a, a/eps    loop    —      Z
7     eps, Z/Z    accept  —      Z
Result: ACCEPTED
Reason: Read all of the input and reached accepting state accept
```

`to_grammar` turns any regular language into a **right-linear grammar**: one
variable per state of its minimal DFA, a rule `A -> a B` for each transition,
and `A -> eps` for each accepting state.

```kstar
print to_grammar(/(0|1)*01/)
```

```text
Grammar with 3 variables over {0, 1}, start S
S -> 0 A | 1 S
A -> 0 A | 1 B
B -> 0 A | 1 S | eps
```

## What cannot be asked

For regular languages, `equivalent(a, b)` always has an answer. For
context-free languages it cannot: **no algorithm can decide whether two
grammars generate the same language**. Kleene Star says so before running:

```kstar-error
alphabet AB = {a, b}
grammar G over AB {
    S -> a S b | eps
}
grammar H over AB {
    S -> a T | eps
    T -> S b
}
print equivalent(G, H)
```

```text
Error in demo.kstar, line 9:
    print equivalent(G, H)
                     ^
TypeError[E0306]: `equivalent` cannot compare context-free languages, and this is a Grammar
  hint: whether two grammars or PDAs describe the same language is undecidable: no algorithm can answer it for every pair. Compare them on sample words with `accepts` instead
```

The same goes for the other operations that only work on regular languages,
such as `minimize`, `complement` and `union`: they need a Dfa, Nfa, Regex or
set of strings.

The [next chapter](08-turing.md) goes one step further, to machines that can
compute anything a program can.
