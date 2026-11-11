-- Luwu -> Luau: `__init` as the constructor body.
--
-- Luwu allocates the object, fills declared fields with their defaults (or
-- nil), then calls `__init(self, ...)` with the call's arguments.
--
-- The builder keeps that order: allocate, call `__init`, return the object.
-- `__init` is a normal method in Luau; there is nothing to protect, because
-- the output has no encapsulation to break.

const Point = {}
Point.__index = Point

function Point.new(...)
	const self = setmetatable({}, Point)
	Point.__init(self, ...)
	return self
end

function Point:__init(text)
	const x, y = string.match(text, "^(%-?%d+),(%-?%d+)$")
	self.x = tonumber(x) or 0
	self.y = tonumber(y) or 0
end

setmetatable(Point, {
	__call = function(_, ...)
		return Point.new(...)
	end,
})

const point = Point("3,4")
print(point.x, point.y)

const broken = Point("not a point")
print(broken.x, broken.y)
