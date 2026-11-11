# Luwu → Luau

A transpiler from **Luwu** (`.luwu`) to plain **Luau** (`.luau`). The output contains no types, generic parameters, or type aliases—only regular Luau (tables and metatables), suitable for Roblox/Rojo, `luau-analyze`, and `lune`.

```
class Greeter(name: string)
    function hello(self): string
        return `hi {self.name}`
    end
end

print(Greeter("Ada"):hello())
```

```lua
-- build/main.luau (example output)
const Greeter = {}
Greeter.__index = Greeter
function Greeter.new(name)
    const self = setmetatable({}, Greeter)
    self.name = name
    return self
end
function Greeter:hello()
    return `hi {self.name}`
end
setmetatable(Greeter, {
    __call = function(_, ...)
        return Greeter.new(...)
    end,
})
print(Greeter("Ada"):hello())
```

## Build

Requires Rust (Cargo).

```powershell
cargo build --release
# binary: target/release/luwu.exe
```

Check the transpiler sources:

```powershell
cargo test
```

## Quick start

```powershell
luwu init --rojo         # create luwu.toml, src/, and a Rojo project skeleton
# or: luwu new MyGame --rojo

luwu build               # transpile .luwu -> .luau into build/ (+ copy assets)
luwu check               # validate without writing files
luwu watch               # rebuild on changes
```

`luwu init` creates `luwu.toml`, a `src/` folder (and optionally `default.project.json` with a skeleton pointing at `build/`), and `.gitignore`.

## Commands

| Command | Description |
| --- | --- |
| `luwu build [PATH]...` | Transpile `.luwu` to `.luau` and copy other files into `output.dir` |
| `luwu check [PATH]...` | Parse, validate, and lower sources without writing files |
| `luwu watch [PATH]...` | Rebuild when files change |
| `luwu init [PATH]` | Create `luwu.toml`, `src/`, starter files, and `.gitignore` |
| `luwu new <NAME>` | Create a new project in the `<NAME>` directory |
| `luwu clean` | Remove the output directory |
| `luwu print-config` | Show the effective (config + flags) configuration |
| `luwu install` | Install `luwu` into a user directory and add it to PATH (Windows) |
| `luwu uninstall` | Remove the installed `luwu` |

Main flags: `-o, --out <DIR>`, `-c, --config <FILE>`, `--no-config`, `--exclude <NAME>`, `--no-assets`, `--fail-on <CATEGORY>`, `-q, --quiet`, `-v, --verbose`, `--dry-run`, `-h, --help`, `-V, --version`.

## Configuration: `luwu.toml`

The file is searched for in the current directory and above; command-line flags take precedence. Unknown fields are treated as an error.

```toml
version = 1

[input]
paths = ["src"]      # what to compile (files or folders)
exclude = []         # folder/file names to skip (in addition to built-ins)

[output]
dir = "build"        # where to put generated .luau files
clean = false        # clean the output directory before a build
copy_assets = true   # copy non-.luwu files (models, json, assets) as is

[watch]
debounce_ms = 400

[build]
# "default" | "all" | "syntax" | "contracts" | "types"
# or a list, for example: ["syntax", "contracts", "types"]
fail_on = "default"
```

## Errors and build strictness

On blocking errors, `luwu build`/`luwu check` exit with a non-zero code. `build` runs atomically: if there are errors, output `.luau` files are not written, assets are not copied, and `output.clean` is not applied (no partially built state remains). In `watch`, a failed rebuild does not overwrite `build/`.

Diagnostic categories (`[build].fail_on` / `--fail-on`):

- `syntax` — lexer/parser errors and failures to lower/compile.
- `contracts` — explicit violations of Luwu contracts within information visible in the file (for example, an unimplemented `expect`, an incompatible implementation of an expected method, or overriding a trait's `final` method).
- `types` — explicit type errors based on annotations/signatures (incompatible literals and values with a known annotated type in declarations, assignments, field values, returns, and call arguments).

`default` includes all categories.

## File mapping

Relative paths are preserved, the `.luwu` extension is replaced with `.luau`, and Rojo suffixes are kept (`Foo.server.luwu` → `Foo.server.luau`, `init.luwu` → `init.luau`). When `copy_assets = true`, other files and empty folders are copied into `output.dir` with the same structure.

## Supported constructs

- erasure of types, generic parameters, and `type` aliases;
- `const`/`local`, functions, default values;
- classes (primary constructor, POD, `__init`, fields/methods, statics, metamethods);
- traits (`expect`, `needs`, override, `final`, parameterized traits, `__create`/`__implemented`);
- `class.*` (`isinstance`, `of`, `name`, `fields`, `member`, `implements`);
- `none`, bundles/restructuring/destructuring, comprehensions;
- `or return`, `if const ... when`, `do ... give`, `for ... when`;
- `try` / `is Ok` / `is Err`, `table.drop`, and other features from Luwu examples.

## Source layout

- `src/lexer.rs` — lexer
- `src/parser.rs` — parser and AST construction
- `src/ast.rs` — AST nodes
- `src/lower.rs` — lowering of Luwu constructs to Luau
- `src/luau.rs` — Luau code printing
- `src/validate.rs` — contract checks and explicit type errors
- `src/config.rs` — `luwu.toml`
- `src/main.rs` — CLI, build, watch, installation
- `src/install.rs` — binary installation/removal
