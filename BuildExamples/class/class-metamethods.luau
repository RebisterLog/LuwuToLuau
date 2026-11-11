-- Luwu -> Luau: metamethods on objects.
--
-- Luwu metamethods live in the class and apply to its objects. In the builder's
-- lowering the class table *is* the object's metatable, so `__add`, `__mul`,
-- `__eq` and `__tostring` go on it verbatim -- they are ordinary metamethods.
--
-- `Vec2(...)` inside them still works because the class table is callable.

const Vec2 = {}
Vec2.__index = Vec2

function Vec2.new(x, y)
	const self = setmetatable({}, Vec2)
	self.x = x
	self.y = y
	return self
end

function Vec2:__add(other)
	return Vec2(self.x + other.x, self.y + other.y)
end

function Vec2:__mul(scalar)
	return Vec2(self.x * scalar, self.y * scalar)
end

function Vec2:__eq(other)
	return self.x == other.x and self.y == other.y
end

function Vec2:__tostring()
	return `({self.x}, {self.y})`
end

setmetatable(Vec2, {
	__call = function(_, ...)
		return Vec2.new(...)
	end,
})

const a = Vec2(1, 2)
const b = Vec2(3, 4)
print(a + b)
print(a * 3)
print(a + b == Vec2(4, 6))
