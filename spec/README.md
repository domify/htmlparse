# Test data

- `tokenizer/`: html5lib-tests' tokenizer cases, at commit c777c408b610.
- `tree-construction/`: its tree construction cases, at commit 9329e64694e7,
  the last before they moved to web-platform-tests.
- `LICENSE`: html5lib-tests' (MIT).
- `golden/`: what html5ever 0.35, as the Rust port vendors it, makes of
  every input above: tokens, the tree dumped as html5lib-tests dumps one,
  and its serialization. The tests hold htmlparse to these, not to the
  cases' own expectations, which follow later changes to the standard
  (32 tree cases differ). Written by `oracle/`:

      cargo run --release --offline --manifest-path oracle/Cargo.toml

  Cases with lone surrogates (`doubleEscaped`) are left out: neither a
  Rust nor a ris string holds one.
