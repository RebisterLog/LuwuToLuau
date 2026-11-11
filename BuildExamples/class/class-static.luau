-- Luwu -> Luau: static functions.
--
-- A function without `self` belongs to the class, not to an object, and is
-- called with a dot. In Luau it is just a function stored on the class table,
-- so `Temperature.from_fahrenheit(212)` needs no change at all.

const Temperature = {}
Temperature.__index = Temperature

function Temperature.new(celsius)
	const self = setmetatable({}, Temperature)
	self.celsius = celsius
	return self
end

function Temperature.from_fahrenheit(fahrenheit)
	return Temperature((fahrenheit - 32) * 5 / 9)
end

function Temperature.from_kelvin(kelvin)
	return Temperature(kelvin - 273.15)
end

function Temperature:describe()
	return string.format("%.1f C", self.celsius)
end

setmetatable(Temperature, {
	__call = function(_, ...)
		return Temperature.new(...)
	end,
})

print(Temperature.from_fahrenheit(212):describe())
print(Temperature.from_kelvin(0):describe())
