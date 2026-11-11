-- Luwu -> Luau: generic classes.
--
-- Generic parameters are a type-system feature. They are checked in Luwu and
-- erased at runtime, and the Luau output carries no types at all, so `Box<T>`
-- becomes a plain `Box` and `Pair<A, B>` a plain `Pair`. Nothing else changes:
-- the same object layout and the same methods, just without type arguments.

const Box = {}
Box.__index = Box

function Box.new(value)
	const self = setmetatable({}, Box)
	self.value = value
	return self
end

function Box:get()
	return self.value
end

function Box:unwrap_or(fallback)
	if self.value ~= nil then
		return self.value
	end
	return fallback
end

setmetatable(Box, {
	__call = function(_, ...)
		return Box.new(...)
	end,
})

const numbers = Box(10)
const words = Box("hi")

print(numbers:get(), words:get())
print(numbers:unwrap_or(0), words:unwrap_or("empty"))

const Pair = {}
Pair.__index = Pair

function Pair.new(first, second)
	const self = setmetatable({}, Pair)
	self.first = first
	self.second = second
	return self
end

function Pair:swap()
	return Pair(self.second, self.first)
end

setmetatable(Pair, {
	__call = function(_, ...)
		return Pair.new(...)
	end,
})

const swapped = Pair(1, "one"):swap()
print(swapped.first, swapped.second)
