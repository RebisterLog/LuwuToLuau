-- Luwu -> Luau: default arguments.
--
-- Luwu writes the default in the signature, so inside the function the
-- parameter is always a real value and never nil. Luau has no default values,
-- so the same check moves into the body: when the caller passed nothing, the
-- parameter is nil and gets the default before anything uses it.
--
-- Class constructor parameters work the same way -- their defaults run at the
-- top of `new`, before the fields are assigned.

const function greet(name, greeting)
	if greeting == nil then
		greeting = "Hello"
	end
	print(`{greeting}, {name}!`)
end

greet("Ada")
greet("Grace", "Welcome")

const Window = {}
Window.__index = Window

function Window.new(width, height, title)
	if width == nil then
		width = 800
	end
	if height == nil then
		height = 600
	end
	if title == nil then
		title = "Untitled"
	end

	const self = setmetatable({}, Window)
	self.width = width
	self.height = height
	self.title = title
	return self
end

function Window:describe()
	return `{self.title} ({self.width}x{self.height})`
end

setmetatable(Window, {
	__call = function(_, ...)
		return Window.new(...)
	end,
})

print(Window():describe())
print(Window(1024, 768):describe())
print(Window(1024, 768, "Editor"):describe())
