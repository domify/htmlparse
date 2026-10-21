# htmlparse

An HTML parser for ris: the WHATWG tokenizer and tree construction, ported
rule for rule from html5ever 0.35 (as Campfire's Rust port vendors it), and
its serializer. It builds the same tree as html5ever for all 1792 of
html5lib-tests' tree construction cases and the same tokens for all 7037
tokenizer cases.

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
