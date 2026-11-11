-- Luwu -> Luau: overriding a trait's function.
--
-- Implementing a trait copies its methods, but only where the class does not
-- already define one: `Pirate.greet` wins and `Sailor` keeps the default.
--
-- The trait table still holds its own version, so the override can extend it
-- with `Greeting.greet(self)`. That is a plain function call on the trait
-- table -- no `super`, no inheritance chain.

const Greeting = {}

function Greeting:greet()
	return "Hello!"
end

const Pirate = {}
Pirate.__index = Pirate

function Pirate.new()
	return setmetatable({}, Pirate)
end

function Pirate:greet()
	return Greeting.greet(self) .. " Arr!"
end

setmetatable(Pirate, {
	__call = function(_, ...)
		return Pirate.new(...)
	end,
})

const Sailor = {}
Sailor.__index = Sailor

function Sailor.new()
	return setmetatable({}, Sailor)
end

Sailor.greet = Greeting.greet

setmetatable(Sailor, {
	__call = function(_, ...)
		return Sailor.new(...)
	end,
})

print(Pirate():greet())
print(Sailor():greet())
