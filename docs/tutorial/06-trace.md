# 6. Watching a machine run

`accepts` answers yes or no. When the answer surprises you, you want to see
*why*: which state the machine was in after each symbol. That is what
`trace` shows.

## Tracing a DFA

```kstar
alphabet B = {0, 1}

# Binary numbers divisible by 3. The state is the remainder so far:
# reading a bit doubles the number and adds the bit.
dfa Div3 over B {
    start  r0
    accept r0
    r0 --0--> r0
    r0 --1--> r1
    r1 --0--> r2
    r1 --1--> r0
    r2 --0--> r1
    r2 --1--> r2
}

trace Div3 on "10110"
```

```text
Input: 10110

Step  Symbol  From  To
0     —       —     r0
1     1       r0    r1
2     0       r1    r2
3     1       r2    r2
4     1       r2    r2
5     0       r2    r1
Result: REJECTED
Reason: Ended in non-accepting state r1
```

Reading the table:

- **Step 0** is before anything is read: the machine sits in its start state.
  It has no symbol and no "from" state, so those cells show `—`.
- Each later row is one symbol: the state before it, and the state after it.
- **Result** is the same answer `accepts` would give, and **Reason** says why.

`10110` is 22 in binary, and 22 leaves remainder 1 when divided by 3, which
is exactly where the machine ended: `r1`. Try `"110"` (6) and the run ends in
`r0` with `Result: ACCEPTED`.

The columns grow to fit, so long state names are fine.

## Tracing an NFA

An NFA can be in several states at once, so each cell is the **set** of
states it could be in. The sets already include everything reachable by eps
moves, so they are exactly the states the subset construction would build.

```kstar
alphabet B = {0, 1}

nfa EndsIn01 over B {
    start  a
    accept c
    a --0,1--> a
    a --0-->   b
    b --1-->   c
}

trace EndsIn01 on "1101"
```

```text
Input: 1101

Step  Symbol  From   To
0     —       —      {a}
1     1       {a}    {a}
2     1       {a}    {a}
3     0       {a}    {a,b}
4     1       {a,b}  {a,c}
Result: ACCEPTED
Reason: Ended in {a,c}, which includes accepting state c
```

At step 3 the machine "guesses" both ways: stay in `a`, or move to `b`
because this `0` might be the second-to-last symbol. The guess pays off at
step 4.

If the set of possible states ever becomes empty, every guess has failed and
the NFA can never accept; the reason then says so.

## Symbols outside the alphabet

A symbol the machine does not know stops the trace at that point. The
program itself carries on, so you can trace many words in a row.

```kstar
alphabet B = {0, 1}

dfa Any over B {
    start  q
    accept q
    q --0,1--> q
}

trace Any on "0121"
print "the program goes on"
```

```text
Input: 0121

Step  Symbol  From  To
0     —       —     q
1     0       q     q
2     1       q     q
Result: REJECTED
Reason: Symbol `2` (position 3) is not in the alphabet {0, 1}, so the run stops

the program goes on
```

## What can be traced

`trace` works on a `Dfa`, an `Nfa`, a `Pda` (see [chapter 7](07-context-free.md)) or a `Tm` (see [chapter 8](08-turing.md)). A regex or a finite set has no states to
show, so convert it first:

```kstar-error
trace /(0|1)*01/ on "101"
```

```text
Error in demo.kstar, line 1:
    trace /(0|1)*01/ on "101"
          ^^^^^^^^^^
TypeError: `trace` needs a machine (Dfa, Nfa, Pda or Tm), but this is a Regex
  hint: a regex or set has no states to show; convert it first, for example `trace to_dfa(r) on "01"`
```

```kstar
trace rename(minimize(/(0|1)*01/)) on "101"
```

```text
Input: 101

Step  Symbol  From  To
0     —       —     q0
1     1       q0    q0
2     0       q0    q1
3     1       q1    q2
Result: ACCEPTED
Reason: Ended in accepting state q2
```

`rename` gives the states short names; without it they would be named after
the sets of NFA states they stand for.

The machine to trace is any expression, so `trace to_dfa(r) on w` or
`trace div_by(3) on "110"` work too.

## A note on the dash

On a Windows console that cannot show the em dash (for example when the
output is piped into a program that reads it with an old code page), `kstar`
prints `-` instead. Everything else is the same.
