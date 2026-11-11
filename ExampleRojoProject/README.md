# ExampleRojoProject

Пример Roblox-проекта на **Luwu**: исходники лежат в `src/`, транспилятор
превращает их в чистый Luau в `build/`, а Rojo собирает из результата place.

```
src/shared/Types.luwu     -> build/shared/Types.luau      (ModuleScript)
src/shared/Events.luwu    -> build/shared/Events.luau     (ModuleScript)
src/shared/Config.luwu    -> build/shared/Config.luau     (ModuleScript)
src/server/init.server.luwu -> build/server/init.server.luau (Script)
src/client/init.client.luwu -> build/client/init.client.luau (LocalScript)
src/network/Network.model.json -> build/network/Network.model.json (RemoteEvent'ы)
src/world/Arena.model.json     -> build/world/Arena.model.json     (Model с партами)
```

Модели, json и любые другие файлы из `src/` просто копируются в `build/`
(структура папок сохраняется), поэтому Rojo смотрит только на `build/` и всё
нужное всегда рядом со скриптами.

## Как запустить

```powershell
# 1. Собрать Luau из Luwu
luwu build            # -> build/

# 2. Проверить, что всё разбирается
luwu check

# 3. Собрать place
rojo build -o out.rbxl

# 4. Открыть out.rbxl в Roblox Studio и запустить
```

Для разработки удобно держать две задачи:

```powershell
luwu watch            # пересобирает build/ при изменении .luwu
rojo serve            # отдаёт изменения в редактор
```

## Что внутри

**`src/shared/Types.luwu`** — классы, трейты и «перечисления»:

- трейты с обязательными членами (`expect const name`, `expect function ...`)
  и методами по умолчанию;
- `class Health(max_health: number = 100) implements Damageable` — primary-конструктор,
  вычисляемое поле `public health = max_health`, трейт-методы `is_alive`/`describe`,
  которые копируются в класс;
- «перечисление» как трейт + по классу на значение (`Fire`, `Water`, `Earth`);
- метаметоды класса `Point`: `__add`, `__mul`, `__tostring`;
- `none` — «значения нет»: `find_element` возвращает `none`, а `or return`,
  `if const ... when` и проверки умеют с этим работать;
- `try`/`Ok`/`Err` (в примере это `parse_amount`), `do ... give`,
  comprehensions, `or return`, default-аргументы, bundles.

**`src/shared/Events.luwu`** — обёртки над `RemoteEvent`, одинаковые для сервера
и клиента: трейт `Channel` (`is_ready`, `describe`) и класс `RemoteChannel`
(`fire_server`, `fire_client`, `on_server`, `on_client`), плюс реестр каналов.

**`src/shared/Config.luwu`** — POD-классы настроек: `Settings()` создаёт объект
со свежими дефолтами, `Settings { damage_multiplier = 2 }` — с переопределениями.

**`src/server/init.server.luwu`** — сервер: создаёт каналы, принимает действия
игроков через `RemoteEvent`, отвечает через `FireClient`, работает с моделью
`Arena` из `rojo.model.json` и создаёт объект `Beacon` прямо из кода.

**`src/client/init.client.luwu`** — клиент: подписывается на ответы, отправляет
действие серверу, печатает класс объекта через `class.of` / `class.name`.

## Проверка

Общие модули не обращаются к Roblox API, поэтому их поведение проверяется
обычным интерпретатором:

```powershell
lune run tests/smoke.luau
```

Скрипт `tests/smoke.luau` проверяет 36 утверждений: конструкторы, трейты,
`none`, comprehensions, метаметоды, POD-классы и каналы.
