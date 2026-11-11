-- Luwu -> Luau: library extensions.
--
-- Luwu adds a few functions to the standard library. `table.drop(t, value)`
-- removes every item equal to value from an array, and does nothing when the
-- value is absent.
--
-- Luau has no `table.drop`, and `table.remove(t, table.find(t, x))` is not the
-- same thing: `table.find` returns nil for a missing value, and `table.remove`
-- then removes the *last* item. The translation keeps the find-and-remove pair
-- but guards it, which is exactly the bug `table.drop` exists to avoid.
--
-- Removing several matches is a loop, since `table.find` returns the first one.

const party = { "Ada", "Grace", "Alan" }

const found = table.find(party, "Grace")
if found ~= nil then
	table.remove(party, found)
end
print(#party, table.concat(party, ", "))

const missing = table.find(party, "missing")
if missing ~= nil then
	table.remove(party, missing)
end
print(#party, table.concat(party, ", "))

const numbers = { 10, 20, 30, 20 }
local index = table.find(numbers, 20)
while index ~= nil do
	table.remove(numbers, index)
	index = table.find(numbers, 20)
end
print(#numbers, table.concat(numbers, ", "))
