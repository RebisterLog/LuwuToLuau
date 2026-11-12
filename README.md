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
| `luwu build [PATH]...` | Transpile `.luwu` to `.luau`, copy assets, and generate sourcemap artifacts |
| `luwu check [PATH]...` | Parse, validate, and lower sources without writing files |
| `luwu watch [PATH]...` | Rebuild when files change (can also listen for Roblox Studio sourcemaps) |
| `luwu sourcemap [PROJECT]` | Generate a Rojo-compatible `sourcemap.json` from a project file |
| `luwu sourcemap --studio` | Listen for Roblox Studio companion plugin updates and write a filtered assets sourcemap |
| `luwu init [PATH]` | Create `luwu.toml`, `src/`, starter files, and `.gitignore` |
| `luwu new <NAME>` | Create a new project in the `<NAME>` directory |
| `luwu clean` | Remove the output directory |
| `luwu print-config` | Show the effective (config + flags) configuration |
| `luwu install` | Install `luwu` into a user directory and add it to PATH (Windows) |
| `luwu uninstall` | Remove the installed `luwu` |

Main flags: `-o, --out <DIR>`, `-c, --config <FILE>`, `--no-config`, `--exclude <NAME>`, `--no-assets`, `--fail-on <CATEGORY>`, `-q, --quiet`, `-v, --verbose`, `--dry-run`, `-h, --help`, `-V, --version`.

Sourcemap flags: `luwu sourcemap --build`, `--studio`, `--once`, `--studio-port <PORT>`, `--studio-input <FILE>`. Watch flags: `--generate-sourcemap`, `--studio-sourcemap`, `--studio-port <PORT>`.

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

[sourcemap]
assets = "sourcemap.assets.json" # merged into root sourcemap.json for luau-lsp

[sourcemap.studio]
enabled = false
port = 3667
output = "sourcemap.assets.json"
apply_to_root = true

[sourcemap.studio.filter]
include = []         # whitelist by DataModel dot-path/prefix (e.g. "ReplicatedStorage.Assets")
exclude = []         # blacklist by DataModel dot-path/prefix
include_class = []   # whitelist by ClassName
exclude_class = []   # blacklist by ClassName
include_name = []    # whitelist by instance name (supports '*' wildcard)
exclude_name = []    # blacklist by instance name (supports '*' wildcard)
prune_empty = true
```

## Errors and build strictness

On blocking errors, `luwu build`/`luwu check` exit with a non-zero code. `build` runs atomically: if there are errors, output `.luau` files are not written, assets are not copied, and `output.clean` is not applied (no partially built state remains). In `watch`, a failed rebuild does not overwrite `build/`.

Diagnostic categories (`[build].fail_on` / `--fail-on`):

- `syntax` — lexer/parser errors and failures to lower/compile.
- `contracts` — explicit violations of Luwu contracts within information visible in the file (for example, an unimplemented `expect`, an incompatible implementation of an expected method, or overriding a trait's `final` method).
- `types` — explicit type errors based on annotations/signatures (incompatible literals and values with a known annotated type in declarations, assignments, field values, returns, and call arguments).

`default` includes all categories.

## Sourcemaps

`luwu` generates sourcemaps compatible with **luau-lsp** and **Rojo**:

- **Root `sourcemap.json`** (source-oriented, includes `.luwu` where applicable) — generated from the workspace `default.project.json` for luau-lsp. If `[sourcemap].assets` points to an existing file, it is merged into the root sourcemap.
- **`build/default.project.json`** and **`build/sourcemap.json`** (build-oriented, `.luau`/assets only, no `.luwu`) — generated for Rojo/build output.
- **When generated:** automatically on successful `luwu build` (post-build artifacts), refreshed by `luwu watch` as files change, and created during `luwu init --rojo`.
- **On demand:** `luwu sourcemap [PROJECT]` generates a sourcemap from a Rojo project (`--build` for build-mode output, `-o/--out` to choose the file). If no project path is given, the workspace `default.project.json` is used.

## Roblox Studio integration (filtered assets sourcemap)

You can generate `sourcemap.assets.json` (or another path) directly from the **Roblox Studio DataModel** using the [luau-lsp companion plugin](https://github.com/JohnnyMorganz/luau-lsp) and let `luwu` filter it to only the instances you need.

### How it works

- `luwu` listens on `127.0.0.1:<port>` (default **3667**) and implements the companion plugin HTTP protocol:
  - `GET /get-file-paths` → returns `[]` (no file-path mapping provided).
  - `POST /full` → receives the full DataModel tree (`{"tree": ...}`, gzip-compressed bodies are supported), applies filters, writes the result, and refreshes the root `sourcemap.json` when enabled.
  - `POST /clear` → clears the studio output (when `clear_on_request = true`) and refreshes the root `sourcemap.json` when enabled.
- The filtered output is written to `[sourcemap.studio].output` (default `sourcemap.assets.json`).
- With `apply_to_root = true`, `luwu` regenerates root `sourcemap.json` and merges `[sourcemap].assets`, so luau-lsp sees updates as the file changes.

### Running

```powershell
# Single command: rebuild/watch + Studio listener
luwu watch --studio-sourcemap

# Or run only the Studio listener
luwu sourcemap --studio
```

- `luwu watch --studio-sourcemap` (or `--studio-port <PORT>`) starts the listener in the same process as watch.
- If `[sourcemap.studio].enabled = true` in `luwu.toml`, `luwu watch` starts the listener automatically.
- `luwu sourcemap --studio` runs the listener continuously; add `--once` to accept a single payload and exit.
- Debug without Studio: `luwu sourcemap --studio --studio-input dump.json` applies a saved payload file (`{"tree": ...}`).

Point the luau-lsp companion plugin to the same host/port (default `localhost:3667`).

### Configuration

```toml
[sourcemap]
assets = "sourcemap.assets.json" # merged into root sourcemap.json for luau-lsp

[sourcemap.studio]
enabled = false
port = 3667
output = "sourcemap.assets.json" # filtered DataModel sourcemap from Studio
apply_to_root = true             # refresh root sourcemap.json after updates
clear_on_request = true          # clear output on POST /clear

[sourcemap.studio.filter]
include = ["ReplicatedStorage.Assets", "Workspace.Map"] # whitelist (dot-paths/prefixes); empty = all
exclude = ["ReplicatedStorage.Assets._WIP"]            # blacklist (dot-paths/prefixes)
include_class = []   # whitelist by ClassName; empty = all
exclude_class = []   # blacklist by ClassName
include_name = []    # whitelist by instance name (* wildcard); empty = all
exclude_name = []    # blacklist by instance name (* wildcard)
prune_empty = true   # drop branches with no kept descendants
```

**Filter semantics:** `include` (paths) selects which branches can be kept; within those branches a node must satisfy class/name whitelist/blacklist rules (blacklists take precedence). If `include` is empty, the whole tree is considered before exclude/class/name/pruning rules. `filePaths` provided by the plugin payload are preserved in the output.

## File mapping

Relative paths are preserved, the `.luwu` extension is replaced with `.luau`, and Rojo suffixes are kept (`Foo.server.luwu` → `Foo.server.luau`, `init.luwu` → `init.luau`). When `copy_assets = true`, other files and empty folders are copied into `output.dir` with the same structure.

## Supported constructs

- erasure of types, generic parameters, and `type` aliases;
- `const`/`local`, multiple assignment, functions, default values;
- numeric `for` loops, generic `for`, `break`/`continue`;
- `if`/`elseif`, `if` expressions, `if const`/`if local`, `when` chains,
  `while const`, and `for ... when`;
- classes (primary constructor, POD, `__init`, fields/methods, statics, metamethods);
- traits (`expect`, `needs`, override, `final`, parameterized traits, `__create`/`__implemented`);
- `class.*` (`isinstance`, `of`, `name`, `fields`, `member`, `implements`);
- `none` with Luwu truthiness, bundles/restructuring/destructuring, comprehensions;
- `or return`, `or break`, `or continue`, `and break`, `or continue` guards,
  `do ... give` expressions;
- `try` / `is Ok` / `is Err`, `table.drop`, and other features from Luwu examples;
- Luau-compatible declarations (`function M.foo`, `function M:foo`), attributes
  (`@native`), type assertions (`::`), long strings/comments, and bitwise
  operators (lowered to `bit32`).

See [`tests/qa`](tests/qa/README.md) for the end-to-end QA corpus and the list of
known limitations (advanced destructuring, class primitives, module exports, and
other host-specific features).

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
