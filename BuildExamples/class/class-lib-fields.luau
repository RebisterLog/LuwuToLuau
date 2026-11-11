-- Luwu -> Luau: class.fields
--
-- Luwu returns a frozen list of ClassMembers carrying name, value, isconst and
-- position. Bare Luau keeps the same facts in ordinary tables on the class:
-- one entry per public field, in declaration order.
-- `private password` is not listed, so the loop runs three times, not four.
--
-- Reading the value of a field is just `object[field.name]`, and a nil field
-- (`nickname`) prints nil.

const Profile = {}
Profile.__index = Profile

Profile.fields = {
	{ name = "name", isconst = true, position = 1 },
	{ name = "nickname", isconst = false, position = 2 },
	{ name = "level", isconst = false, position = nil },
}

function Profile.new(name, nickname, password)
	const self = setmetatable({}, Profile)
	self.name = name
	self.nickname = nickname
	self.password = password
	self.level = 1
	return self
end

setmetatable(Profile, {
	__call = function(_, ...)
		return Profile.new(...)
	end,
})

const ada = Profile("Ada")

for _, field in Profile.fields do
	print(field.name, field.isconst, field.position, ada[field.name])
end

print(#Profile.fields)
