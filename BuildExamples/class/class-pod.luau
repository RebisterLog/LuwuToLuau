-- Luwu -> Luau: plain old data (POD) classes.
--
-- A POD class has no positional constructor: `Settings()` and
-- `Settings { fullscreen = true }` both go to the same `new` that takes an
-- optional table of overrides.
--
-- Defaults are re-evaluated on every call, so each object gets its own table.
-- That is why `new` starts from a fresh defaults table instead of a shared one.

const Settings = {}
Settings.__index = Settings

local function settings_defaults()
	return {
		volume = 0.8,
		fullscreen = false,
		player_name = "Player",
	}
end

function Settings.new(init)
	const self = settings_defaults()
	for key, value in init or {} do
		self[key] = value
	end
	return setmetatable(self, Settings)
end

setmetatable(Settings, {
	__call = function(_, init)
		return Settings.new(init)
	end,
})

const defaults = Settings()
const custom = Settings { fullscreen = true, player_name = "Ada" }
print(defaults.volume, custom.fullscreen, custom.player_name)

const Inventory = {}
Inventory.__index = Inventory

local function inventory_defaults()
	return {
		items = {},
	}
end

function Inventory.new(init)
	const self = inventory_defaults()
	for key, value in init or {} do
		self[key] = value
	end
	return setmetatable(self, Inventory)
end

setmetatable(Inventory, {
	__call = function(_, init)
		return Inventory.new(init)
	end,
})

const first = Inventory()
const second = Inventory()
table.insert(first.items, "sword")
print(#first.items, #second.items)
