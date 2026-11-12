# Luwu -> Luau QA corpus

These `.luwu` files exercise the transpiler end to end: parsing, lowering and
the generated Luau runtime behavior. They are also compiled by `cargo test`
(`qa_fixtures_compile`).

Run the runtime checks locally (requires a `luau` executable):

```powershell
$env:LUAU = "C:\path\to\luau.exe"   # optional, defaults to `luau` on PATH
pwsh ./tests/qa/run.ps1
```

The runner transpiles each case into `tests/qa/.out`, executes the result and
prints its output. It fails when a fixture does not transpile or when the
generated Luau exits with a non-zero status.

## Experimental fixtures

`tests/qa/experimental/*.luwu.txt` hold constructs that the current `luwu-lsp`
cannot type-check yet: `try` expressions, bitwise operators, `static` fields and
explicit generic instantiation. They are still compiled by `cargo test` and run
by `run.ps1`, but they use a `.luwu.txt` suffix so the editor does not report
false diagnostics for them.

`39_patterns.luwu` uses the experimental `.(...)` collection patterns. Enable
`DebugLuwuCollections` in the language server settings (the Zed settings in this
repository already do) or the editor will report them as disabled features.

## Known limitations

The transpiler targets plain Luau. A few Luwu features cannot be reproduced
exactly and are either rejected or approximated:

- advanced destructuring: nested patterns, `as` renaming, `.[ ... ]` array
  patterns, non-identifier keys, and `value.{ ... }` suffixes are not parsed;
  simple `.{ a, b }` and `.(a, b)` patterns work;
- non-local `return`/`break`/`continue` inside a `do ... give` expression are
  scoped to the generated helper function (use `give` to leave the block);
- classes are lowered to ordinary tables/metatables, so `type(obj) == "object"`
  and `type(Class) == "class"` are not reproduced, and `private`/`const`
  members are enforced only by the source validator, not at runtime;
- `none` is a table sentinel; it is falsy through generated truthiness checks,
  but `type(none)` is not `"symbol"`;
- `export`/`require` are emitted as ordinary Luau bindings/calls; Luwu module
  semantics (exports, `.luwu` module resolution) are not reproduced;
- extern/userdata types and other host-specific features have no runtime
  equivalent.
