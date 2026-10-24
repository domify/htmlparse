# Working on htmlparse

A native ris module: html5ever 0.35's tokenizer, tree builder and
serializer, ported state for state and rule for rule. The README is for
using it; this page is for changing it.

## Layout

- `htmlparse/tokenizer.ri`: the tokenizer, over bytes, whole input.
- `htmlparse/tree_builder.ri`: the insertion modes, in rules.rs's order.
- `htmlparse/dom.ri`: the tree, on the tape, linked by index.
- `htmlparse/parse.ri`, `htmlparse/serialize.ri`: the API.
- `htmlparse/tags.ri`, `htmlparse/entity_data.ri`: written by `tools/`;
  do not edit.
- `oracle/`: Rust that writes `spec/golden/` with the vendored html5ever.

## Checks

```sh
./check.sh
```

## Rules

- html5ever is the oracle: a change of behaviour needs a golden from
  `oracle/` that shows html5ever does the same. Never edit `spec/golden/`
  by hand.
- Follow html5ever's code closely; name the Rust function a ris one ports
  where it is not obvious.
- Tape first: take `Str`, write into the caller's writer, return
  views of the input. No heap copies in the parse.
- Every line is at most 80 characters in documents and commit messages.
