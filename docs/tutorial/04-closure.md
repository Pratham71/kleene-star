# 4. Combining and comparing languages

Regular languages are **closed** under the usual set operations: combine two
regular languages and the result is regular again. Each closure property is
proved by a construction, and each construction is a function here.

## Union, intersection, difference

These use the **product construction**: run both machines side by side, with
one state for every pair of states.

```toc
let even_zeros = /1*(01*01*)*/
let ends_in_1  = /(0|1)*1/

let both   = intersect(even_zeros, ends_in_1)
let either = union(even_zeros, ends_in_1)
let only   = difference(even_zeros, ends_in_1)

test both accepts "001"
test not (both accepts "01")
test either accepts "01"
test only accepts "00"
test not (only accepts "001")
```

The arguments can be any mix of DFAs, NFAs, regexes and finite sets. The result
is a DFA whose states are named after the pairs, like `(q0,q1)`.

## Complement

`complement` swaps accepting and non-accepting states. This is only correct for
a complete DFA, which is one reason DFAs must have every transition.

```toc
alphabet B = {0, 1}
let ends_in_1 = to_dfa(/(0|1)*1/, B)

let not_ending_in_1 = complement(ends_in_1)
test not_ending_in_1 accepts ""
test not_ending_in_1 accepts "10"
test not (not_ending_in_1 accepts "01")
```

Complement depends on the alphabet: the complement of `/0*/` over `{0}` is
empty, but over `{0, 1}` it is every string containing a `1`. Use
`to_dfa(regex, B)` to say which you mean.

## Asking questions

`equivalent(a, b)` is true when the two languages are exactly the same.
`is_empty(a)` is true when a language contains no strings at all.

```toc
alphabet B = {0, 1}
let a = to_dfa(/1*(01*01*)*/, B)
let b = to_dfa(/(0|1)*1/, B)

# De Morgan's law: not (A or B) = (not A) and (not B)
test equivalent(complement(union(a, b)), intersect(complement(a), complement(b)))

# A language and its complement share nothing and together cover everything.
test is_empty(intersect(a, complement(a)))
test equivalent(union(a, complement(a)), /(0|1)*/)
```

`equivalent` works by building the machine for "strings accepted by exactly one
of the two" and checking that it is empty.

You cannot compare machines with `==`. Two machines can look completely
different and still accept the same language, so TOC-Lang makes you say which
question you are asking:

```toc-error
let same = /a*/ == /a*a*/
```

```text
Error in demo.toc, line 1:
    let same = /a*/ == /a*a*/
               ^^^^^^^^^^^^^^
TypeError: `==` cannot compare machines or regexes
  hint: to ask whether two of them describe the same language, use `equivalent(a, b)`
```

## Concatenation, star, reversal

These are built with eps moves, so they return NFAs.

```toc
test equivalent(concat(/0*/, /1*/), /0*1*/)
test equivalent(star(/01/), /(01)*/)
test equivalent(reverse(/001*/), /1*00/)
```

## Finite languages

A finite language is just a set of strings, written in braces. It can be used
anywhere a language is expected.

```toc
let words = {"0", "01", "011"}

test words accepts "01"
test not (words accepts "10")
test equivalent(words, /01?1?/)

print len(words)
print rename(minimize(words))
```

```text
3
DFA with 5 states over {0, 1}
       | 0  | 1
->  q0 | q1 | q2
   *q1 | q2 | q3
    q2 | q2 | q2
   *q3 | q2 | q4
   *q4 | q2 | q2
tests: 3 passed, 0 failed
```

## Alphabets must match

Two machines can only be combined when they are over the same alphabet.

```toc-error
alphabet B = {0, 1}
alphabet L = {a, b}
dfa Zeros over B { start s  accept s  s --0--> s  s --1--> t  t --0,1--> t }
dfa As over L { start s  accept s  s --a--> s  s --b--> t  t --a,b--> t }

let both = union(Zeros, As)
```

```text
Traceback (most recent call last):
  demo.toc, line 6, in <main>
    let both = union(Zeros, As)
                            ^^
AlphabetError: this machine is over {a, b}, but {0, 1} is needed here
  hint: languages can only be combined when they share an alphabet
```

Next: [functions and machine families](05-functions.md).
