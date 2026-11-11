-- Luwu -> Luau: final and static trait members.
--
-- `total_area` has no `self`, so it is static: it belongs to the trait, not to
-- an object, and is called as `Shape.total_area(shapes)`. In Luau that is just
-- a function on the trait table, and being `final` means nothing is copied
-- down to the classes -- there is no override to prevent.
--
-- What each class contributes is `area`, which the trait only expects.

const Shape = {}

function Shape.total_area(shapes)
	local total = 0
	for _, shape in shapes do
		total += shape:area()
	end
	return total
end

const Rectangle = {}
Rectangle.__index = Rectangle

function Rectangle.new(width, height)
	const self = setmetatable({}, Rectangle)
	self.width = width
	self.height = height
	return self
end

function Rectangle:area()
	return self.width * self.height
end

setmetatable(Rectangle, {
	__call = function(_, ...)
		return Rectangle.new(...)
	end,
})

const Circle = {}
Circle.__index = Circle

function Circle.new(radius)
	const self = setmetatable({}, Circle)
	self.radius = radius
	return self
end

function Circle:area()
	return math.pi * self.radius ^ 2
end

setmetatable(Circle, {
	__call = function(_, ...)
		return Circle.new(...)
	end,
})

print(Shape.total_area({ Rectangle(2, 3), Circle(1) }))
