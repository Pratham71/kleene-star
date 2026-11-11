# 5. Functions and machine families

Many results in the theory are about a whole *family* of machines: "for every
n, there is a DFA with n states that...". Functions let you write the family
once and build any member of it.

## Functions

```kstar
fn double(n: Int) -> Int {
    return n * 2
}

fn longer(a: Str, b: Str) -> Str {
    if len(a) >= len(b) {
        return a
    } else {
        return b
    }
}

print double(21)
print longer("01", "0110")
```

```text
42
0110
```

- Each parameter has a type, and the return type follows `->`.
- Every path through a function must end in `return`.
- The types are `Int`, `Bool`, `Str`, `Alphabet`, `Dfa`, `Nfa`, `Regex` and
  `Set[T]`, for example `Set[Str]`.
- Functions can use values defined above them, and can call each other in any
  order, including themselves.

Types are checked before the program runs, so a wrong argument is caught even
in a line that would never execute.

## Machines as values

Leave out the name and a machine definition becomes a value you can return or
store.

```kstar
alphabet B = {0, 1}

fn only(word_length: Int) -> Nfa {
    return nfa over B {
        start  s[0]
        accept s[word_length]
        for i in 0..word_length {
            s[i] --0,1--> s[i + 1]
        }
    }
}

let three = only(3)
test three accepts "010"
test not (three accepts "01")
print states(three)
```

```text
4
tests: 2 passed, 0 failed
```

Two new things make this work:

- `s[i]` is a **numbered state**: the name `s` followed by the value of `i`. So
  `s[0]` is the state `s0`, and `s[i + 1]` is the next one along.
- `for i in 0..n { ... }` repeats the transitions inside for `i = 0, 1, ..., n - 1`.
  The range `a..b` includes `a` and stops before `b`.

## A classic: divisibility

Binary numbers divisible by `n`. State `q[i]` means "the number read so far
leaves remainder `i`". Reading a bit doubles the number and adds the bit.

```kstar
alphabet B = {0, 1}

fn div_by(n: Int) -> Dfa {
    return dfa over B {
        start  q[0]
        accept q[0]
        for i in 0..n {
            q[i] --0--> q[(2 * i) % n]
            q[i] --1--> q[(2 * i + 1) % n]
        }
    }
}

test div_by(3) accepts "110"          # 6
test not (div_by(3) accepts "111")    # 7
test div_by(5) accepts "1010"         # 10

# Divisible by 6 means divisible by 2 and by 3.
test equivalent(div_by(6), intersect(div_by(2), div_by(3)))
```

`%` is the remainder. It is never negative for a positive divisor, so it is
always safe as a state number.

## Measuring the exponential blow-up

"The k-th symbol from the end is 1" needs `k + 1` NFA states but `2^k` DFA
states. With a function, you can watch it happen.

```kstar
alphabet B = {0, 1}

fn kth_from_end(k: Int) -> Nfa {
    return nfa over B {
        start  s[0]
        accept s[k]
        s[0] --0,1--> s[0]
        s[0] --1-->   s[1]
        for i in 1..k {
            s[i] --0,1--> s[i + 1]
        }
    }
}

for k in 1..6 {
    let guess = kth_from_end(k)
    print "k =", k, " NFA:", states(guess), " min-DFA:", states(minimize(guess))
}
```

```text
k = 1  NFA: 2  min-DFA: 2
k = 2  NFA: 3  min-DFA: 4
k = 3  NFA: 4  min-DFA: 8
k = 4  NFA: 5  min-DFA: 16
k = 5  NFA: 6  min-DFA: 32
```

## Loops and conditions

`for` also works outside machines. It can step through a range, a set, the
characters of a string, or the symbols of an alphabet.

```kstar
alphabet B = {0, 1}
let r = /(0|1)*01/

for w in {"01", "10", "1101"} {
    if r accepts w {
        print w, "matches"
    } else {
        print w, "does not match"
    }
}

for symbol in B {
    print symbol
}
```

```text
01 matches
10 does not match
1101 matches
0
1
```

Conditions combine with `and`, `or` and `not`.

## Reading a traceback

When something fails inside a function, the error shows how the program got
there. Read it from the bottom: the last line is what went wrong, the lines
above are who called whom.

```kstar-error
alphabet B = {0, 1}

fn div_by(n: Int) -> Dfa {
    return dfa over B {
        start  q[0]
        accept q[0]
        for i in 0..n + 1 {
            q[i] --0--> q[(2 * i) % n]
            q[i] --1--> q[(2 * i + 1) % n]
        }
    }
}

fn smallest(n: Int) -> Dfa {
    return minimize(div_by(n))
}

test smallest(0) accepts "110"
```

```text
Traceback (most recent call last):
  demo.kstar, line 18, in <main>
    test smallest(0) accepts "110"
  demo.kstar, line 15, in smallest
    return minimize(div_by(n))
  demo.kstar, line 8, in div_by
    q[i] --0--> q[(2 * i) % n]
                  ^^^^^^^^^^^
MathError: modulo by zero
  hint: the right-hand side of `%` is 0 here
```

That covers the whole language. The [next chapter](06-trace.md) shows how to
watch a machine run step by step, and [the reference](../reference.md) lists
everything on one page.
