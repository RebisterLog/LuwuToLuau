-- Luwu -> Luau: traits with parameters.
--
-- A parameterized trait supplies values, not just methods: `count` is a field
-- default and `max_stack` comes from the argument. So the trait becomes a
-- function that builds a table of members, and each class calls it with the
-- arguments it wants -- the default 64 for `Arrow`, 8 for `Potion`.
--
-- The factory's methods go on the class, its defaults go on the object, and
-- every object still gets its own `count` the first time `add` writes to it.

const function Stackable(max_stack)
	const max = max_stack or 64

	const trait = {
		max_stack = max,
		count = 1,
	}

	function trait:add(amount)
		const room = self.max_stack - self.count
		const added = math.min(room, amount)
		self.count += added
		return amount - added
	end

	return trait
end

const StackedArrow = Stackable()

const Arrow = {}
Arrow.__index = Arrow
Arrow.add = StackedArrow.add

function Arrow.new()
	return setmetatable({
		max_stack = StackedArrow.max_stack,
		count = StackedArrow.count,
	}, Arrow)
end

setmetatable(Arrow, {
	__call = function()
		return Arrow.new()
	end,
})

const StackedPotion = Stackable(8)

const Potion = {}
Potion.__index = Potion
Potion.add = StackedPotion.add

function Potion.new()
	return setmetatable({
		max_stack = StackedPotion.max_stack,
		count = StackedPotion.count,
	}, Potion)
end

setmetatable(Potion, {
	__call = function()
		return Potion.new()
	end,
})

print(Arrow():add(100), Potion():add(100))
