-- Luwu -> Luau: destructuring and restructuring.
--
-- `const .{ name, level } = player` becomes one multiple assignment that reads
-- the fields, and in a `for` header the same lines move into the loop body.
-- It works the same for objects: `const .{ x, y } = Point(3, 4)` reads fields.
--
-- Restructuring is the mirror image: `.{ name, level }` builds a table, which
-- in Luau is the shorthand-free `{ name = name, level = level }`.

const player = { name = "Ada", level = 12 }
const name, level = player.name, player.level
print(name, level)

const person = { name = name, level = level }
print(person.name, person.level)

const tasks = {
	{ title = "Feed Nyla", done = true },
	{ title = "Write docs", done = false },
}

for _, task in tasks do
	const title, done = task.title, task.done
	print(title, done)
end

const Point = {}
Point.__index = Point

function Point.new(x, y)
	const self = setmetatable({}, Point)
	self.x = x
	self.y = y
	return self
end

setmetatable(Point, {
	__call = function(_, ...)
		return Point.new(...)
	end,
})

const point = Point(3, 4)
const x, y = point.x, point.y
print(x, y)
