-- Luwu -> Luau: an enum as a trait plus one class per variant.
--
-- Luwu has no `enum` keyword, so a closed set of values is written as a trait
-- with the shared behaviour and a class per variant. Each variant is a class
-- with fixed const fields, and `Mercury()` / `Earth()` build the values.
--
-- In Luau the trait is a table of shared methods, the variants are classes
-- whose fields are just values, and the enum is the list you iterate.
-- The `const` on the fields is a Luwu guarantee and is dropped.

const Planet = {}

function Planet:weight(mass)
	return mass * self.gravity
end

const Mercury = {}
Mercury.__index = Mercury

function Mercury.new()
	const self = setmetatable({
		name = "Mercury",
		gravity = 3.7,
	}, Mercury)
	return self
end

Mercury.weight = Planet.weight

setmetatable(Mercury, {
	__call = function()
		return Mercury.new()
	end,
})

const Earth = {}
Earth.__index = Earth

function Earth.new()
	const self = setmetatable({
		name = "Earth",
		gravity = 9.8,
	}, Earth)
	return self
end

Earth.weight = Planet.weight

setmetatable(Earth, {
	__call = function()
		return Earth.new()
	end,
})

const planets = { Mercury(), Earth() }

for _, planet in planets do
	print(planet.name, planet:weight(80))
end
