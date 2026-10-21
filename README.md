# htmlparse

An HTML parser for ris: the WHATWG tokenizer and tree construction, ported
rule for rule from html5ever 0.35 (as Campfire's Rust port vendors it), and
its serializer. It builds the same tree as html5ever for all 1792 of
html5lib-tests' tree construction cases and the same tokens for all 7037
tokenizer cases.

```ris
import htmlparse::{htmlparse}

tape { puts htmlparse(body) }
```

`htmlparse` parses its input as a fragment in `body` (`document: true`
for a whole document) and answers the HTML that html5ever's serializer
writes for it, on the tape, at the same speed as the calls it makes. For
the tree itself:

```ris
import htmlparse/htmlparse/parse::{parse_fragment, Options}
import htmlparse/htmlparse/dom::{Namespace}
import htmlparse/htmlparse/serialize::{serialize_children, SerializeOptions}

tape do
  doc = parse_fragment(body.as_tape, "body".as_tape, Namespace::Html,
    Options.gumbo)
  out = Tape::String::Writer.new(body.bytesize)
  serialize_children(doc.dom, doc.root, out, SerializeOptions.new)
  puts out
end
```

`parse_document` reads a whole document; `Document#root` is then the
document node.

## Memory

A parse works on the caller's tape and opens none of its own. The tree,
its names, text and attribute values are tape values: views of the input
wherever they are its bytes unchanged, copied only where they differ (a
name lowercased, a character reference read, a NUL replaced). The input
must outlive the tree. Nodes are one `Tape::Array`, linked by index, so
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

| Input | Parser | Parse, µs | Parse, MiB/s | Serialize, µs |
| --- | --- | ---: | ---: | ---: |
| message, 118 B | htmlparse | 2.6 | 43.7 | 0.3 |
| message, 118 B | html5ever | 2.9 | 38.2 | 1.0 |
| rich message, 1 KiB | htmlparse | 39.7 | 34.7 | 3.7 |
| rich message, 1 KiB | html5ever | 38.0 | 36.3 | 11.3 |
| page, 3 KiB | htmlparse | 79.2 | 38.2 | 6.4 |
| page, 3 KiB | html5ever | 96.0 | 31.5 | 17.2 |
| page, 45 KiB | htmlparse | 937.7 | 46.7 | 90.1 |
| page, 45 KiB | html5ever | 1,352.3 | 32.4 | 278.2 |
| page, 904 KiB | htmlparse | 19,621.0 | 45.0 | 2,360.9 |
| page, 904 KiB | html5ever | 26,297.2 | 33.6 | 7,835.6 |
| page, 4.5 MiB | htmlparse | 101,051.5 | 44.3 | 12,097.1 |
| page, 4.5 MiB | html5ever | 134,113.2 | 33.4 | 39,705.4 |

Parsing, htmlparse takes 10 to 30% less time than html5ever on all but
the rich message, where it is 4% behind; serializing, about a third of
the time throughout.

The memory one parse takes, without serializing: the peak RSS of a
process that parses the input once, less that of one that only reads it.
That is whole pages, so it is given for the inputs of 44 KiB and up.

| Input | Parser | Memory per parse, MiB | Per input byte |
| --- | --- | ---: | ---: |
| page, 45 KiB | htmlparse | 0.4 | 8.6 |
| page, 45 KiB | html5ever | 0.6 | 14.4 |
| page, 904 KiB | htmlparse | 9.1 | 10.3 |
| page, 904 KiB | html5ever | 11.8 | 13.4 |
| page, 4.5 MiB | htmlparse | 45.4 | 10.1 |
| page, 4.5 MiB | html5ever | 58.0 | 13.0 |

The tree takes about ten bytes for each byte of input, a fifth less than
html5ever's. The tape keeps all of it until the parse's `tape do` ends,
and gives its pages back when it does: a loop of parses faults them in
again each time, where html5ever's allocator reuses its own. These are
one machine's numbers, not a promise; `bench/results.json` has the raw
samples.
