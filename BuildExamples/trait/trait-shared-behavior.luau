-- Luwu -> Luau: a trait that shares behaviour.
--
-- A trait is a plain table of default methods. `implements` copies those
-- methods onto the class, so `Lootable` is written once and both `Chest` and
-- `Slime` get `drop` without restating it.
--
-- `expect loot` is only checked in Luwu and has no runtime form here: the
-- method just reads `self.loot`, and a class that forgot the field fails the
-- same way any missing field does.

const Lootable = {}

function Lootable:drop()
	const dropped = self.loot
	self.loot = {}
	return dropped
end

const Chest = {}
Chest.__index = Chest

function Chest.new(loot)
	const self = setmetatable({}, Chest)
	self.loot = loot
	return self
end

Chest.drop = Lootable.drop

setmetatable(Chest, {
	__call = function(_, ...)
		return Chest.new(...)
	end,
})

const Slime = {}
Slime.__index = Slime

function Slime.new(hp, loot)
	const self = setmetatable({}, Slime)
	self.hp = hp
	self.loot = loot
	return self
end

Slime.drop = Lootable.drop

setmetatable(Slime, {
	__call = function(_, ...)
		return Slime.new(...)
	end,
})

const chest = Chest({ "gold", "map" })
const slime = Slime(5, { "goo" })
print(#chest:drop(), #chest:drop(), slime:drop()[1])
