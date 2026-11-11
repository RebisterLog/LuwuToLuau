-- smoke.luau — проверка общих модулей без Roblox.
--
-- Запуск:  lune run tests/smoke.luau
-- Модули в build/shared не обращаются к Roblox API, поэтому их можно
-- проверить обычным интерпретатором.

local Types = require("../build/shared/Types")
local Events = require("../build/shared/Events")
local Config = require("../build/shared/Config")

local failures = 0
local checks = 0

local function check(name, actual, expected)
    checks += 1
    if actual ~= expected then
        failures += 1
        print(`FAIL {name}: expected {expected}, got {actual}`)
    end
end

-- Класс с primary-конструктором и дефолтами.
local health = Types.Health()
check("health default", health:current_health(), 100)
check("health damage", health:damage(30), 70)
check("health floor", health:damage(1000), 0)
check("health heal", health:heal(25), 25)
check("health trait method", health:is_alive(), true)
check("health describe", health:describe(), "vitals (25/100)")

-- Трейты: методы по умолчанию попадают в классы-варианты.
local elements = Types.ELEMENTS
check("elements count", #elements, 3)
check("element friendly", elements[2]:is_friendly(), true)
check("element label", elements[1]:to_string(), "Fire")

-- none: значение есть, но оно «пустое».
local missing = Types.find_element(elements, "air")
check("none is not nil", missing == nil, false)
check("none sentinel", missing == Types.find_element(elements, "air"), true)

-- `or return` срабатывает и на none.
check("element label found", Types.element_label(elements, "water"), "Water")
check("element label missing", Types.element_label(elements, "air"), "unknown")
check("friendly label", Types.friendly_label(elements, "water"), "friendly: Water")
check("unfriendly label", Types.friendly_label(elements, "fire"), "not friendly")

-- Comprehension и `do ... give`.
check("friendly labels", table.concat(Types.friendly_labels(elements), ","), "Water,Earth")
check("friendly count", Types.friendly_count(elements), 2)

-- Метаметоды класса Point.
local origin = Types.Point(0, 0)
local target = Types.Point(3, 4)
check("point add", tostring(origin + target), "(3, 4)")
check("point mul", tostring(target * 2), "(6, 8)")
check("point midpoint", tostring(Types.midpoint(origin, target)), "(1.5, 2)")
check("class name", Types.describe_class(origin), "Point")
check("class of plain value", Types.describe_class({}), "plain value")

-- Default arguments.
check("clamp default", Types.clamp(50), 50)
check("clamp range", Types.clamp(500, 0, 100), 100)

-- Ошибка превращается в Err через pcall (в серверном скрипте это `try`).
local ok, problem = pcall(Types.parse_amount, "nope")
check("parse fails", ok, false)
check("parse error message", string.find(tostring(problem), "not a number") ~= nil, true)

-- POD-классы настроек.
local settings = Config.default_settings()
check("settings max health", settings.max_health, 100)
check("settings elements", table.concat(settings.friendly_elements, ","), "water,earth")
check("settings summary", table.concat(Config.summary(settings), "|"), "friendly: water|friendly: earth")

local rules = Config.default_rules()
check("rule multiplier", rules:multiplier_for("fire"), 2.0)
check("rule default", rules:multiplier_for("unknown"), 1.0)

-- Каналы: трейт Channel проверяет готовность канала.
local fake_remote = {
    Name = "PlayerAction",
    OnServerEvent = { Connect = function() end },
    OnClientEvent = { Connect = function() end },
    FireServer = function() end,
    FireClient = function() end,
}
local channel = Events.RemoteChannel(fake_remote)
check("channel ready", channel:is_ready(), true)
check("channel describe", channel:describe(), "channel PlayerAction")

local registry = Events.build_registry({ PlayerAction = fake_remote })
check("registry size", #registry:names(), 1)
check("registry get", registry:get("PlayerAction"):describe(), "channel PlayerAction")
check("registry missing", registry:get("Nope") == nil, false)

print(`{checks - failures}/{checks} checks passed`)
if failures > 0 then
    error(`${failures} check(s) failed`)
end
