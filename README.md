# Luwu → Luau

Транспилятор из **Luwu** (`.luwu`) в чистый **Luau** (`.luau`) для Roblox/Rojo
проектов. На выходе нет ни типов, ни generic'ов, ни алиасов — только обычный
Luau, который понимают Roblox, Rojo, `luau-analyze` и `lune`.

```
class Greeter(name: string)
    function hello(self): string
        return `hi {self.name}`
    end
end

print(Greeter("Ada"):hello())
```

```lua
-- build/main.luau
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

---

## Установка

**Готовый бинарник:** скачайте `luwu.exe` из релизов (или соберите его сами,
см. ниже) и положите в любую папку. Чтобы команда работала из любого места,
добавьте папку в `PATH`:

```powershell
$env:PATH += ";C:\tools\luwu"
```

**Из исходников:**

```powershell
git clone <репозиторий> luwu
cd luwu
cargo install --path .        # бинарник попадёт в ~/.cargo/bin
```

**Сборка release-версии и упаковка:**

```powershell
powershell -ExecutionPolicy Bypass -File scripts/package.ps1 -Zip
# -> dist/luwu.exe и dist/luwu-<version>.zip
```

---

## Быстрый старт

```powershell
cd MyGame
luwu new MyGame --rojo   # конфиг + исходники + Rojo-проект
# пишем код в src/*.luwu
luwu build               # -> build/*.luau (+ остальные файлы из src)
luwu watch               # пересобирает при изменениях
```

Рядом с `rojo serve`:

```powershell
luwu watch                    # в одном терминале
rojo serve                    # в другом
```

В `default.project.json` Rojo должен смотреть на сгенерированное дерево:

```json
{
  "name": "MyGame",
  "tree": {
    "$path": "build"
  }
}
```

---

## Команды

| Команда | Что делает |
| --- | --- |
| `luwu build [PATH]...` | Транспилирует `.luwu` в `.luau` (по умолчанию в `build/`) и переносит остальные файлы |
| `luwu check [PATH]...` | Разбирает и понижает исходники, ничего не записывая |
| `luwu watch [PATH]...` | Пересобирает при изменении файлов |
| `luwu init [PATH]` | Создаёт `luwu.toml`, папку `src/`, стартовые файлы и `.gitignore` |
| `luwu new <NAME>` | Создаёт новый проект целиком (конфиг + исходники + `.gitignore`) |
| `luwu clean` | Удаляет выходную папку |
| `luwu print-config` | Показывает итоговую (конфиг + флаги) конфигурацию |

### `init` и `new`

```powershell
luwu init                 # в текущей папке: luwu.toml, src/, .gitignore
luwu init --rojo          # ещё и default.project.json + заглушки скриптов
luwu init path/to/project # в другой папке
luwu init --force         # перезаписать существующие файлы

luwu new MyGame           # новая папка MyGame/ со всем скелетом
luwu new MyGame --rojo    # + Rojo-проект, готовый к сборке
```

Скелет `--rojo` уже смотрит на `build/`, поэтому после `luwu build` проект
собирается без ручной настройки:

```
MyGame/
  luwu.toml
  default.project.json
  .gitignore
  src/shared/init.luwu       -> ModuleScript
  src/server/init.server.luwu -> Script
  src/client/init.client.luwu -> LocalScript
```

### Флаги

| Флаг | Значение |
| --- | --- |
| `-o, --out <DIR>` | Выходная папка (перекрывает `output.dir`) |
| `-c, --config <FILE>` | Путь к файлу конфигурации |
| `--no-config` | Полностью игнорировать `luwu.toml` |
| `--exclude <NAME>` | Дополнительно исключить папку/файл (можно повторять) |
| `--no-assets` | Не переносить файлы, отличные от `.luwu` (модели, json, ассеты) |
| `-q, --quiet` / `-v, --verbose` | Меньше / больше вывода |
| `--dry-run` | Показать, что будет сделано, ничего не меняя |
| `-h, --help` / `-V, --version` | Справка / версия |

Примеры:

```powershell
luwu build src --out dist       # конкретные пути и выходная папка
luwu build --dry-run            # посмотреть список файлов и результат
luwu check                      # только проверка, без записи
luwu clean --dry-run            # посмотреть, что будет удалено
luwu print-config               # что реально используется
```

---

## Конфигурация: `luwu.toml`

Файл ищется в текущей папке и выше; флаги командной строки имеют приоритет.
`luwu init` создаёт такой файл:

```toml
version = 1

[input]
# что компилировать (файлы или папки)
paths = ["src"]
# дополнительно исключить (поверх встроенных: build, target, .git, node_modules)
exclude = []

[output]
# куда класть сгенерированные .luau
dir = "build"
# очищать выходную папку перед сборкой
clean = false
# переносить ли остальные файлы (модели, json, ассеты) в build как есть
copy_assets = true

[watch]
# задержка пересборки в watch-режиме, мс
debounce_ms = 400
```

Неизвестные поля — ошибка с указанием, что именно не так, так что опечатки
не проходят молча.

### Что переносится в `build/`

`build/` — это полная копия входных папок, в которой `.luwu` превращён в
`.luau`, а всё остальное скопировано как есть:

```
src/shared/Types.luwu          -> build/shared/Types.luau
src/world/Arena.model.json     -> build/world/Arena.model.json
src/assets/logo.png            -> build/assets/logo.png
src/empty-folder/              -> build/empty-folder/   (пустые папки тоже)
```

Поэтому Rojo может смотреть только на `build/`, а модели и ассеты лежат рядом
со скриптами. Отключается флагом `--no-assets` или `output.copy_assets = false`.

**Приоритеты**

1. Флаги командной строки (`--out`, `--exclude`, явные пути) важнее конфига.
2. `input.exclude` применяется к путям из `input.paths`. Если путь указан явно
   в командной строке (`luwu build BuildExamples`), он компилируется «как есть»:
   действуют только встроенные исключения (`build`, `target`, `.git`,
   `node_modules`).
3. Если конфига нет, `luwu build` сам выберет `src/`, а если там нет `.luwu` —
   текущую папку.

Пример конфига этого репозитория (`luwu.toml` в корне):

```toml
version = 1

[input]
paths = ["."]
exclude = ["BuildExamples", "tests", "dist", "target"]

[output]
dir = "build"

[watch]
debounce_ms = 400
```

---

## Куда попадают файлы

Относительные пути сохраняются, меняется только расширение `.luwu` → `.luau`.
Суффиксы Rojo не теряются:

| Luwu | Luau |
| --- | --- |
| `src/main.luwu` | `build/main.luau` |
| `src/Foo.server.luwu` | `build/Foo.server.luau` |
| `src/Bar.client.luwu` | `build/Bar.client.luau` |
| `src/Module/init.luwu` | `build/Module/init.luau` |

---

## Что поддерживается

- полное стирание типов, generics, union'ов и `type`-алиасов;
- `const`, `const function` и локальные биндинги;
- классы: primary-конструктор, POD с `new(init)`, `__init`, вычисляемые поля,
  статические методы, метаметоды, `__call`;
- `public` / `private` / `const` у полей (модификаторы отбрасываются, private
  не попадает в метаданные);
- библиотека `class`: `isinstance`, `of`, `name`, `fields`, `member`,
  `implements`;
- трейты: методы по умолчанию, `expect`, `needs` (включая транзитивное
  копирование методов и полей), override, `final`, статические методы,
  параметризованные трейты-фабрики, `__create` / `__implemented`;
- enum-паттерны из примеров (trait + классы-варианты);
- destructuring / restructuring `.{...}`, bundles `.(...)` и `point.1`;
- `none` через sentinel-таблицу;
- table comprehensions;
- expressive control flow: `or return`, `if const ... when`, `do ... give`,
  `for ... when`;
- `try` -> `pcall`, `is Ok.(v)` / `is Err.(e)`;
- default arguments;
- `table.drop` через helper;
- вызовы с таблицей-аргументом: `Class { ... }`.

---

## Пример проекта

`ExampleRojoProject/` — рабочий Roblox-проект на Luwu: общие модули, серверный
скрипт, клиентский скрипт, `RemoteEvent`-ы и модели из `rojo.model.json`.

```
src/shared/Types.luwu        -> build/shared/Types.luau        (ModuleScript)
src/shared/Events.luwu       -> build/shared/Events.luau       (ModuleScript)
src/shared/Config.luwu       -> build/shared/Config.luau       (ModuleScript)
src/server/init.server.luwu  -> build/server/init.server.luau  (Script)
src/client/init.client.luwu  -> build/client/init.client.luau  (LocalScript)
src/network/Network.model.json -> build/network/Network.model.json (RemoteEvent'ы)
src/world/Arena.model.json     -> build/world/Arena.model.json     (Model с партами)
```

Модели и прочие файлы не компилируются, а просто переносятся в `build/` — Rojo
смотрит только на `build/`, и всё нужное лежит рядом.

Проверяется одной командой — транспиляция, синтаксис, поведение и сборка place:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/verify-example.ps1
```

```
1/4 transpiling Luwu -> Luau...
2/4 checking generated syntax...
     5 file(s) parsed
3/4 running behavioural checks...
     36/36 checks passed
4/4 building the place with Rojo...
     place built: ExampleRojoProject\out.rbxl
```

Поведение общих модулей (классы, трейты, `none`, comprehensions, метаметоды,
POD-классы, каналы) проверяется отдельным скриптом на обычном интерпретаторе:

```powershell
cd ExampleRojoProject
lune run tests/smoke.luau
```

---

## Тесты

Три уровня проверки:

```powershell
# 1. Юнит-тесты компилятора: структура lowering'а, отсутствие типов в выводе,
#    все фикстуры из tests/fixtures должны компилироваться.
cargo test

# 2. Поведенческие тесты фикстуры: компиляция -> luau-analyze -> lune run
#    и сравнение вывода с эталонным *.expected файлом.
powershell -ExecutionPolicy Bypass -File tests/run-fixtures.ps1

# 3. Регрессия по примерам из BuildExamples (эталон — соседние *.luau).
cargo run -- build BuildExamples --out build_test
Get-ChildItem -Recurse build_test -Filter *.luau | ForEach-Object {
    luau-analyze $_.FullName
    lune run $_.FullName
}

# 4. Полная проверка примера Roblox-проекта: транспиляция -> синтаксис ->
#    поведение -> сборка place через Rojo.
powershell -ExecutionPolicy Bypass -File scripts/verify-example.ps1
```

### Фикстуры

`tests/fixtures/` — набор сложных случаев, которые держат транспилятор в
форме. Для каждого `*.luwu` рядом лежит `*.expected` с ожидаемым stdout.

| Фикстура | Что проверяет |
| --- | --- |
| `class/heavy-class.luwu` | primary-конструктор, `private`/`const` поля, вычисляемые поля, статические методы, вся библиотека `class.*` |
| `class/pod-composition.luwu` | POD-классы, свежие дефолты на каждый экземпляр, вложенные таблицы, `Class { ... }` |
| `class/init-and-lib.luwu` | `__init` как конструктор, разбор аргументов, цепочки вызовов методов |
| `trait/deep-needs.luwu` | цепочки `needs`, override с вызовом трейта, транзитивное копирование методов и полей |
| `trait/param-multi.luwu` | параметризованные трейты: разные аргументы у разных классов, отсутствие общего состояния |
| `trait/static-final-override.luwu` | `final`-методы, статические методы трейта, `expect`-члены |
| `trait/factory-registry.luwu` | `__create` / `__implemented`, реестр реализаций, обработка ошибки |
| `trait/multiple-traits.luwu` | несколько трейтов у класса, конфликт имён, приоритет собственных методов |
| `mixed/classes-traits.luwu` | метаметоды из трейта, изоляция `__eq` между классами, `class.of` / `class.member` |
| `mixed/error-handling.luwu` | `try` / `Ok` / `Err`, `none`, comprehensions и работа с коллекциями внутри методов |

---

## Ограничения

- Выход намеренно «сырой» Luau: типы, инкапсуляция и проверки Luwu не
  эмулируются, если у них нет прямого аналога в Luau.
- `class.member` и `class.fields` используют небольшие helper-функции,
  которые добавляются в начало файла только при использовании.
- `class.of("not a table")` возвращает `nil`; пример в `BuildExamples`
  намеренно не печатает этот случай, поэтому stdout может отличаться на одну
  строку.
- Параметризованный трейт и `__implemented` рассчитаны на один trait-хук на
  файл (как в примерах).
- Статические методы трейта остаются на трейте и вызываются как
  `Trait.method(...)`; в классы копируются только instance-методы.
- `__eq`, пришедший из трейта, оборачивается в проверку класса: объекты
  разных классов никогда не равны друг другу.
