-- Luwu -> Luau: an enum with payloads, as a tagged union of classes.
--
-- A variant that carries data gets its own class: `Circle` holds a radius,
-- `Rect` holds width and height. The `kind` field is the tag, and the trait is
-- the common interface the values are used through.
--
-- Luau keeps the same three parts -- a tag field, a class per variant, and a
-- trait table for what they share (here nothing, so `Shape` emits no table).
-- `__init` becomes the constructor body, as in any other translated class.

const Circle = {}
Circle.__index = Circle

function Circle.new(...)
	const self = setmetatable({
		kind = "circle",
		radius = 0,
	}, Circle)
	Circle.__init(self, ...)
	return self
end

function Circle:__init(radius)
	self.radius = radius
end

function Circle:area()
	return math.pi * self.radius ^ 2
end

function Circle:describe()
	return `circle with radius {self.radius}`
end

setmetatable(Circle, {
	__call = function(_, ...)
		return Circle.new(...)
	end,
})

const Rect = {}
Rect.__index = Rect

function Rect.new(...)
	const self = setmetatable({
		kind = "rect",
		width = 0,
		height = 0,
	}, Rect)
	Rect.__init(self, ...)
	return self
end

function Rect:__init(width, height)
	self.width = width
	self.height = height
end

function Rect:area()
	return self.width * self.height
end

function Rect:describe()
	return `rect {self.width}x{self.height}`
end

setmetatable(Rect, {
	__call = function(_, ...)
		return Rect.new(...)
	end,
})

const shapes = { Circle(1), Rect(2, 3) }

for _, shape in shapes do
	print(shape.kind, shape:describe(), shape:area())
end
