-- Luwu -> Luau: a trait with expected members.
--
-- `expect const name` and `expect function details` are obligations: the class
-- must provide them. Luwu checks that statically, so the Luau output only needs
-- the class's own definitions plus the trait's default `describe`.
--
-- `details` is not overwritten by the trait -- the class already has it, and
-- `describe` calls it through `self:details()`.

const Describable = {}

function Describable:describe()
	return `{self.name}: {self:details()}`
end

const Sword = {}
Sword.__index = Sword

function Sword.new(init)
	const self = setmetatable({
		name = "Iron sword",
		damage = 7,
	}, Sword)
	for key, value in init or {} do
		self[key] = value
	end
	return self
end

function Sword:details()
	return `deals {self.damage} damage`
end

Sword.describe = Describable.describe

setmetatable(Sword, {
	__call = function(_, init)
		return Sword.new(init)
	end,
})

print(Sword():describe())
