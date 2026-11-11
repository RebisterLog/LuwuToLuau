-- Luwu -> Luau: computed fields and defaults that use constructor parameters.
--
-- Luwu evaluates field initializers while building the object, after the
-- primary-constructor parameters exist but before the constructor returns.
-- The builder keeps that order: assign the parameters, then run each field
-- initializer in declaration order. Later initializers can read earlier fields.
--
-- The parameters stay fields too: `name.raw` is still there.

const Username = {}
Username.__index = Username

function Username.new(raw)
	const self = setmetatable({}, Username)
	self.raw = raw
	self.text = string.lower((string.gsub(raw, "%s+", "_")))
	return self
end

setmetatable(Username, {
	__call = function(_, ...)
		return Username.new(...)
	end,
})

const name = Username("Ada Lovelace")
print(name.text, name.raw)

const Rect = {}
Rect.__index = Rect

function Rect.new(width, height)
	const self = setmetatable({}, Rect)
	self.width = width
	self.height = height
	self.area = width * height
	self.label = `{width}x{height}`
	return self
end

setmetatable(Rect, {
	__call = function(_, ...)
		return Rect.new(...)
	end,
})

const rect = Rect(3, 4)
print(rect.area, rect.label)
