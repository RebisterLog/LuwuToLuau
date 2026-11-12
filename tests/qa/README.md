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
