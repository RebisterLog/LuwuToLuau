-- Luwu -> Luau: enum variants that share their methods.
--
-- Every variant has the same behaviour and differs only in data, so the trait
-- carries `go`, `stop` and `label`, and each class only declares its two
-- const fields. Implementing the trait copies those three functions into each
-- class table, so the methods exist once per class and not once per object.

const Light = {}

function Light:go()
	return self.name == "green"
end

function Light:stop()
	return self.name == "red"
end

function Light:label()
	return `{self.name} -> {self.next_name}`
end

const Red = {}
Red.__index = Red
Red.name = "red"
Red.next_name = "green"
Red.go = Light.go
Red.stop = Light.stop
Red.label = Light.label

function Red.new()
	return setmetatable({}, Red)
end

setmetatable(Red, {
	__call = function()
		return Red.new()
	end,
})

const Yellow = {}
Yellow.__index = Yellow
Yellow.name = "yellow"
Yellow.next_name = "red"
Yellow.go = Light.go
Yellow.stop = Light.stop
Yellow.label = Light.label

function Yellow.new()
	return setmetatable({}, Yellow)
end

setmetatable(Yellow, {
	__call = function()
		return Yellow.new()
	end,
})

const Green = {}
Green.__index = Green
Green.name = "green"
Green.next_name = "yellow"
Green.go = Light.go
Green.stop = Light.stop
Green.label = Light.label

function Green.new()
	return setmetatable({}, Green)
end

setmetatable(Green, {
	__call = function()
		return Green.new()
	end,
})

const lights = { Red(), Yellow(), Green() }

for _, light in lights do
	print(light:label(), light:go(), light:stop())
end
