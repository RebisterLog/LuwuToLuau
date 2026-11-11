-- Luwu -> Luau: expressive control flow.
--
-- Each shorthand expands into the plain statements it stands for:
--
--   `x or return v`        -> read x, and `if x == nil then return v end`
--   `if const x = e when c`-> bind x, then `if x ~= nil and c then`
--   `do ... give value`    -> an immediately called function returning value
--   `for ... when c`       -> a loop with `if c then` around the body
--
-- Nothing is lost in the translation; the Luau side is just louder. Luwu's
-- `when` chains and `do`/`give` blocks are the reason the source reads better.

const prices = { apple = 1.25, pear = 0.8 }

const function describe(fruit)
	const price = prices[fruit]
	if price == nil then
		return `{fruit} isn't sold here`
	end
	return `{fruit} costs {price}`
end

print(describe("apple"))
print(describe("kiwi"))

const users = {
	ada = { name = "Ada", admin = true },
	grace = { name = "Grace", admin = false },
}

const user = users["ada"]
if user ~= nil and user.admin then
	print(`{user.name} is an admin`)
end

const other = users["grace"]
if other ~= nil and other.admin then
	print("Grace is an admin")
else
	print("Grace is not an admin")
end

const scores = { 72, 95, 88 }
const best = (function()
	local top = 0
	for _, score in scores do
		top = math.max(top, score)
	end
	return top
end)()
print(best)

for _, fruit in { "apple", "kiwi", "pear" } do
	const price = prices[fruit]
	if price ~= nil then
		print(`{fruit} costs {price}`)
	end
end
