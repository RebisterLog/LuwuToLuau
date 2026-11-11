-- Luwu -> Luau: class.member
--
-- Luwu's class.member returns a ClassMember that knows whether the member is a
-- field or a method and prints itself as a declaration without its type:
--
--   class.member(ada, "name")      -> public const name = "Ada"
--   class.member(Profile, "greet") -> public function greet
--   class.member(Profile, "name")  -> public const name
--
-- Bare Luau has no such object, so the class describes its members with plain
-- tables and the declaration is written from them. Reading a member is just
-- indexing the class or the object.

const Profile = {}
Profile.__index = Profile

Profile.members = {
	name = { name = "name", kind = "field", isconst = true, visibility = "public", position = 1 },
	greet = { name = "greet", kind = "method", isconst = false, visibility = "public", position = nil },
}

function Profile.new(name)
	const self = setmetatable({}, Profile)
	self.name = name
	return self
end

function Profile:greet()
	return `hi {self.name}`
end

setmetatable(Profile, {
	__call = function(_, ...)
		return Profile.new(...)
	end,
})

const ada = Profile("Ada")

const name = Profile.members.name
print(`{name.visibility} const {name.name} = "{ada[name.name]}"`)

const greet = Profile.members.greet
print(`{greet.visibility} function {greet.name}`)

print(`{name.visibility} const {name.name}`)
print(ada[name.name])
