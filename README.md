# htmlparse

An HTML parser for ris: the WHATWG tokenizer and tree construction, ported
rule for rule from html5ever 0.35 (as Campfire's Rust port vendors it), and
its serializer. It builds the same tree as html5ever for all 1792 of
html5lib-tests' tree construction cases and the same tokens for all 7037
tokenizer cases.

```ris
import htmlparse::{htmlparse}

tape do
  html = htmlparse(body) # a Str: the HTML, parsed and written back
  puts html
end
```

`htmlparse` parses its input as a fragment in `body` (`document: true`
for a whole document) and answers the HTML that html5ever's serializer
writes for it, on the tape, at the same speed as the calls it makes.

A longer example, which parses a fragment with `parse_fragment`, walks
its tree node by node and writes it back, is in `examples/tree.ri`
(`ris run examples/tree.ri`).

`parse_document` reads a whole document; `Document#root` is then the
document node.

## Memory

A parse works on the caller's tape and opens none of its own. The tree,
its names, text and attribute values are tape values: views of the input
wherever they are its bytes unchanged, copied only where they differ (a
name lowercased, a character reference read, a NUL replaced). The input
must outlive the tree. Nodes are one `Ary`, linked by index, so
foster parenting and the adoption agency move a node in a few writes.
Text added next to text is kept in pieces and joined once, when read.

## Options

`Options.new` has html5ever's defaults: scripting on, a `<template>`'s
content in a fragment of its own (`Dom#contents`), a byte order mark
dropped at the start and after each `</script>`.

`Options.gumbo` is what Nokogiri's HTML5 parser does: no scripting,
template content among the children, a byte order mark left (Gumbo drops
only a leading one; the caller does), and Gumbo's limits of 400 elements
deep and 400 attributes a tag, which end the parse with
`Document#error` set.

`SerializeOptions` writes html5ever's serialization; Nokogiri's is the
same with `raw_noscript` always on. `escape_attribute_brackets` writes
`<` and `>` in attribute values as the standard has since 2025.

## Departures from html5ever

None that html5lib-tests' inputs show in the trees, tokens or
serialization. Parse errors are not reported. The tree
sink's choices are fixed to the ones the Rust port and the oracle make:
no form association, no declarative shadow roots, no MathML
annotation-xml integration points.

## Speed

`python3 bench/run.py` times htmlparse against html5ever 0.35 from
crates.io (into `markup5ever_rcdom`, LTO, one codegen unit). Each run
parses an input n times and serializes every tree, timing the two apart;
the two parsers' outputs must be the same byte for byte. The inputs are
two chat messages, parsed as fragments in `body`, and generated pages of
1, 20, 400 and 2000 articles, parsed as documents: text with entities,
attributes, tables, lists, SVG, comments, a script and style, unclosed
and misnested tags. One warmup and five rounds in a shuffled order;
medians.

Local result, 2026-10-21, AMD Ryzen 7 PRO 6850U, Linux 7.0, rustc 1.99.0,
ris 0.1.0:

Parse:

| Input | htmlparse | html5ever | html5ever vs htmlparse |
| --- | ---: | ---: | ---: |
| message, 118 B | 2.5 µs | 2.9 µs | 18% slower |
| rich message, 1 KiB | 25.5 µs | 36.2 µs | 42% slower |
| page, 3 KiB | 55.5 µs | 92.0 µs | 66% slower |
| page, 45 KiB | 766.9 µs | 1,306.6 µs | 70% slower |
| page, 904 KiB | 18,013.7 µs | 25,259.9 µs | 40% slower |
| page, 4.5 MiB | 94,194.9 µs | 130,299.8 µs | 38% slower |

Serialize:

| Input | htmlparse | html5ever | html5ever vs htmlparse |
| --- | ---: | ---: | ---: |
| message, 118 B | 0.3 µs | 0.9 µs | 3.0× slower |
| rich message, 1 KiB | 3.5 µs | 10.8 µs | 3.1× slower |
| page, 3 KiB | 6.2 µs | 16.4 µs | 2.7× slower |
| page, 45 KiB | 89.8 µs | 268.1 µs | 3.0× slower |
| page, 904 KiB | 2,230.5 µs | 6,806.3 µs | 3.1× slower |
| page, 4.5 MiB | 11,992.8 µs | 39,140.1 µs | 3.3× slower |

Parsing, html5ever is 18 to 70% slower than htmlparse; serializing,
about three times slower throughout.

The memory one parse takes, without serializing: the peak RSS of a
process that parses the input once, less that of one that only reads it.
That is whole pages, so it is given for the inputs of 44 KiB and up.

Memory per parse:

| Input | htmlparse | html5ever | html5ever vs htmlparse |
| --- | ---: | ---: | ---: |
| page, 45 KiB | 0.5 MiB | 0.6 MiB | 28% more |
| page, 904 KiB | 7.5 MiB | 11.8 MiB | 57% more |
| page, 4.5 MiB | 37.7 MiB | 58.2 MiB | 54% more |

htmlparse's tree takes about eight bytes for each byte of input, and
html5ever's about half as much again on the larger pages. The tape keeps all of
it until the parse's `tape do` ends, and gives its pages back when it
does: a loop of parses faults them in again each time, where html5ever's
allocator reuses its own.

On threads, `bench/threads.ri` and `bench/rust/src/bin/threads.rs`: 1
thread and then 8 (`--threads`) at once, each parsing and serializing the
input as many times as a single run does, `htmlparse(input)` on its own
tape; parses per second over all of them, from the first start to the
last join. The CPU has 8 cores, 16 hardware threads.

Parses and serializations per second on 8 threads:

| Input | htmlparse | html5ever | html5ever vs htmlparse |
| --- | ---: | ---: | ---: |
| message, 118 B | 2,083,256 | 1,478,438 | 41% slower |
| rich message, 1 KiB | 190,547 | 113,655 | 68% slower |
| page, 45 KiB | 6,862 | 3,544 | 94% slower |
| page, 904 KiB | 272 | 160 | 70% slower |

From 1 thread to 8, how many times as many per second:

| Input | htmlparse | html5ever |
| --- | ---: | ---: |
| message, 118 B | 6.4× | 6.2× |
| rich message, 1 KiB | 6.1× | 6.1× |
| page, 45 KiB | 6.4× | 6.2× |
| page, 904 KiB | 6.1× | 5.8× |

Each thread's tape is its own, so the parses share nothing and need no
lock: htmlparse goes from 1 thread to 8 about as html5ever does, six
times, and keeps its lead. These are one machine's numbers, not a
promise; `bench/results.json` has the raw samples.
