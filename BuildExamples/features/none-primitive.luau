-- Luwu -> Luau: the none primitive.
--
-- `none` means "intentionally empty" and, unlike nil, it stays in an array:
-- `{ "first", none, "third" }` has three items, not two.
--
-- Luau has no such value, so the builder emits one shared sentinel table named
-- `none`. That keeps the array hole-free and makes `== none` a comparison
-- against a single value. Two adjustments follow from a table being truthy:
-- `if found then` must become `if found ~= none then`, and a lone `none` prints
-- as none only because the sentinel says so.

const none = setmetatable({}, {
	__tostring = function()
		return "none"
	end,
})

const list = { "first", none, "third" }
print(#list, list[2] == none, list[2] == nil)

const function find(items, wanted)
	for _, item in items do
		if item == wanted then
			return item
		end
	end
	return none
end

const found = find({ 10, 20 }, 20)
if found ~= none then
	print(`found {found}`)
end

const missing = find({ 10, 20 }, 99)
print(missing == none)
