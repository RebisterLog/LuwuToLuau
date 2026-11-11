-- Luwu -> Luau: traits as factories.
--
-- `__create` makes the trait callable: `Component("button")` returns a new
-- object of whichever implementing class answers `tag()` with that name.
-- `__implemented` runs once per class when it starts implementing the trait.
--
-- Both are ordinary functions here. The registry is an array next to the
-- trait, `__create` becomes the trait's call, and `__implemented` becomes a
-- plain `implemented(class)` that the class calls after defining itself.
--
-- The trait holds no state: the array belongs to the module, not to the trait.

const components = {}

const function implemented(class)
	table.insert(components, class)
end

const function Component(tag)
	for _, class in components do
		if class.tag() == tag then
			return class()
		end
	end
	error(`no component renders <{tag}>`, 2)
end

const Button = {}
Button.__index = Button

function Button.new()
	return setmetatable({}, Button)
end

function Button.tag()
	return "button"
end

function Button:render()
	return "<button>Save</button>"
end

setmetatable(Button, {
	__call = function()
		return Button.new()
	end,
})

implemented(Button)

const Spinner = {}
Spinner.__index = Spinner

function Spinner.new()
	return setmetatable({}, Spinner)
end

function Spinner.tag()
	return "progress"
end

function Spinner:render()
	return "<progress></progress>"
end

setmetatable(Spinner, {
	__call = function()
		return Spinner.new()
	end,
})

implemented(Spinner)

for _, tag in { "progress", "button" } do
	print(Component(tag):render())
end
