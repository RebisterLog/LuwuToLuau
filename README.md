# Luwu → Luau

Транспилятор из **Luwu** (`.luwu`) в чистый **Luau** (`.luau`). На выходе нет типов, generic-параметров и type-алиасов — только обычный Luau (таблицы и метатаблицы), пригодный для Roblox/Rojo, `luau-analyze` и `lune`.

```
class Greeter(name: string)
    function hello(self): string
        return `hi {self.name}`
    end
end

print(Greeter("Ada"):hello())
```

```lua
-- build/main.luau (пример результата)
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

## Сборка

Требуется Rust (Cargo).

```powershell
cargo build --release
# бинарник: target/release/luwu.exe
```

Проверка исходников транспилятора:

```powershell
cargo test
```

## Быстрый старт

```powershell
luwu init --rojo         # создать luwu.toml, src/ и каркас Rojo-проекта
# или: luwu new MyGame --rojo

luwu build               # .luwu -> .luau в build/ (+ копирование ассетов)
luwu check               # проверить без записи
luwu watch               # пересобирать при изменениях
```

`luwu init` создаёт `luwu.toml`, папку `src/` (и опционально `default.project.json` со скелетом, указывающим на `build/`) и `.gitignore`.

## Команды

| Команда | Что делает |
| --- | --- |
| `luwu build [PATH]...` | Транспилирует `.luwu` в `.luau` и переносит остальные файлы в `output.dir` |
| `luwu check [PATH]...` | Разбирает, валидирует и понижает исходники без записи |
| `luwu watch [PATH]...` | Пересобирает при изменении файлов |
| `luwu init [PATH]` | Создаёт `luwu.toml`, `src/`, стартовые файлы и `.gitignore` |
| `luwu new <NAME>` | Создаёт новый проект в папке `<NAME>` |
| `luwu clean` | Удаляет выходную папку |
| `luwu print-config` | Показывает итоговую (конфиг + флаги) конфигурацию |
| `luwu install` | Устанавливает `luwu` в пользовательскую папку и добавляет в PATH (Windows) |
| `luwu uninstall` | Удаляет установленный `luwu` |

Основные флаги: `-o, --out <DIR>`, `-c, --config <FILE>`, `--no-config`, `--exclude <NAME>`, `--no-assets`, `--fail-on <CATEGORY>`, `-q, --quiet`, `-v, --verbose`, `--dry-run`, `-h, --help`, `-V, --version`.

## Конфигурация: `luwu.toml`

Файл ищется в текущей папке и выше; флаги командной строки имеют приоритет. Неизвестные поля считаются ошибкой.

```toml
version = 1

[input]
paths = ["src"]      # что компилировать (файлы или папки)
exclude = []         # имена папок/файлов для пропуска (поверх встроенных)

[output]
dir = "build"        # куда класть сгенерированные .luau
clean = false        # очищать выходную папку перед сборкой
copy_assets = true   # копировать не-.luwu файлы (модели, json, ассеты) как есть

[watch]
debounce_ms = 400

[build]
# "default" | "all" | "syntax" | "contracts" | "types"
# или список, например: ["syntax", "contracts", "types"]
fail_on = "default"
```

## Ошибки и строгость сборки

При блокирующих ошибках `luwu build`/`luwu check` завершаются с ненулевым кодом. `build` выполняется атомарно: если есть ошибки, выходные `.luau` не записываются, ассеты не копируются и `output.clean` не применяется (частично собранного состояния не остаётся). В `watch` неудачная пересборка не перезаписывает `build/`.

Категории диагностик (`[build].fail_on` / `--fail-on`):

- `syntax` — ошибки лексера/парсера и невозможность понижения/компиляции.
- `contracts` — явные нарушения контрактов Luwu в пределах видимой в файле информации (например, нереализованный `expect`, несовместимая реализация ожидаемого метода, переопределение `final`-метода трейта).
- `types` — явные ошибки типов по аннотациям/сигнатурам (несовместимые литералы и значения с известным аннотированным типом в объявлениях, присваиваниях, значениях полей, возвратах и аргументах вызовов).

`default` (по умолчанию) включает все категории.

## Отображение файлов

Относительные пути сохраняются, расширение `.luwu` меняется на `.luau`, суффиксы Rojo не теряются (`Foo.server.luwu` → `Foo.server.luau`, `init.luwu` → `init.luau`). При `copy_assets = true` остальные файлы и пустые папки копируются в `output.dir` с сохранением структуры.

## Поддерживаемые конструкции

- стирание типов, generic-параметров и `type`-алиасов;
- `const`/`local`, функции, значения по умолчанию;
- классы (primary-конструктор, POD, `__init`, поля/методы, статика, метаметоды);
- трейты (`expect`, `needs`, override, `final`, параметризованные трейты, `__create`/`__implemented`);
- `class.*` (`isinstance`, `of`, `name`, `fields`, `member`, `implements`);
- `none`, bundles/restructuring/destructuring, comprehensions;
- `or return`, `if const ... when`, `do ... give`, `for ... when`;
- `try` / `is Ok` / `is Err`, `table.drop` и другая поддержка из примеров Luwu.

## Структура исходников

- `src/lexer.rs` — лексер
- `src/parser.rs` — парсер и AST-построение
- `src/ast.rs` — узлы AST
- `src/lower.rs` — понижение Luwu-конструкций в Luau
- `src/luau.rs` — печать Luau-кода
- `src/validate.rs` — проверки контрактов и явных ошибок типов
- `src/config.rs` — `luwu.toml`
- `src/main.rs` — CLI, сборка, watch, установка
- `src/install.rs` — установка/удаление бинарника
