-- Luwu -> Luau: the simplest class.
--
-- class Dog(name, age)   -> a table used as the object's metatable, plus
--                           Dog.new(name, age); __call keeps `Dog(...)` working
-- function bark(self)    -> Dog.bark(self), reachable as `dog:bark()`
-- self.name              -> an ordinary table field
--
-- Calls need no rewriting: `Dog("Oreo", 3)` stays exactly as written in Luwu.

const Dog = {}
Dog.__index = Dog

function Dog.new(name, age)
	const self = setmetatable({}, Dog)
	self.name = name
	self.age = age
	return self
end

function Dog:bark()
	print(`{self.name} barked!`)
end

function Dog:birthday()
	self.age += 1
end

setmetatable(Dog, {
	__call = function(_, ...)
		return Dog.new(...)
	end,
})

const oreo = Dog("Oreo", 3)
oreo:bark()
oreo:birthday()
print(oreo.age)
