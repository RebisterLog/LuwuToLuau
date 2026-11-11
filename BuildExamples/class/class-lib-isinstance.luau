-- Luwu -> Luau: class.isinstance
--
-- class.isinstance(object, Class) asks "was this built by Class?". With
-- metatable classes the same question is `getmetatable(object) == Class`,
-- so the check is written inline and no helper is needed.
--
-- Non-objects are false: a string's metatable is the string metatable, never a
-- class table.

const Circle = {}
Circle.__index = Circle

function Circle.new(radius)
	const self = setmetatable({}, Circle)
	self.radius = radius
	return self
end

setmetatable(Circle, {
	__call = function(_, ...)
		return Circle.new(...)
	end,
})

const Square = {}
Square.__index = Square

function Square.new(side)
	const self = setmetatable({}, Square)
	self.side = side
	return self
end

setmetatable(Square, {
	__call = function(_, ...)
		return Square.new(...)
	end,
})

const function area(shape)
	if getmetatable(shape) == Circle then
		return math.pi * shape.radius ^ 2
	end
	return shape.side ^ 2
end

print(area(Circle(1)))
print(area(Square(3)))
print(getmetatable(Circle(1)) == Square)
print(getmetatable("circle") == Circle)
