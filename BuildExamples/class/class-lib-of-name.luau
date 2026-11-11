-- Luwu -> Luau: class.of and class.name
--
-- class.of(x) is the class that built x, which is exactly `getmetatable(x)`
-- for an object, and nil for a plain table.
-- class.name(C) is not a Luau concept, so the class keeps its name in a plain
-- field: `__name`. Reading an object's class name goes through its metatable.

const Account = {}
Account.__index = Account
Account.__name = "Account"

function Account.new(owner)
	const self = setmetatable({}, Account)
	self.owner = owner
	return self
end

setmetatable(Account, {
	__call = function(_, ...)
		return Account.new(...)
	end,
})

const account = Account("Ada")

print(getmetatable(account) == Account)
print(getmetatable({}))
print(Account.__name)
print(getmetatable(account).__name)
