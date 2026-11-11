-- Luwu -> Luau: `const` and `private` members.
--
-- `const` on a binding already exists in Luau and is emitted as is:
-- `const ada = ...`, `const function greeting`.
--
-- `const` on a field and `private` are Luwu-only. Luau can't enforce either, so
-- the builder drops them: `name` and `password` stay ordinary fields and the
-- class simply doesn't expose them as metadata. The behaviour that matters
-- (`rename` changing `nickname`, `check` reading `password`) is unchanged.

const Profile = {}
Profile.__index = Profile

-- A plain table has no access modifiers: `name`, `nickname` and `password` are
-- all just fields, and only convention keeps `password` inside the class.

function Profile.new(name, nickname, password)
	const self = setmetatable({}, Profile)
	self.name = name
	self.nickname = nickname
	self.password = password
	self.level = 1
	return self
end

function Profile:rename(nickname)
	self.nickname = nickname
end

function Profile:check(attempt)
	return attempt == self.password
end

setmetatable(Profile, {
	__call = function(_, ...)
		return Profile.new(...)
	end,
})

const function greeting(person)
	return `hi {person.name}`
end

const ada = Profile("Ada", nil, "")
ada:rename("the countess")
print(greeting(ada), ada.nickname, ada.level, ada:check(""))
