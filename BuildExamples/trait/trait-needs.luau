-- Luwu -> Luau: traits that need other traits.
--
-- `Named` only lists a requirement and has no methods, so it contributes
-- nothing to copy and emits nothing at all.
-- `Speaker needs Named` means "implementing Speaker also means implementing
-- Named" -- a fact Luwu checks, with no runtime form here.
--
-- Because the output only copies methods, `implements(Robot(), Named)` has no
-- equivalent either: what is left is the plain truth that Robot has `name`
-- and got `introduce` from Speaker.

const Speaker = {}

function Speaker:introduce()
	return `I'm {self.name}.`
end

const Robot = {}
Robot.__index = Robot

function Robot.new(init)
	const self = setmetatable({
		name = "R2",
	}, Robot)
	for key, value in init or {} do
		self[key] = value
	end
	return self
end

Robot.introduce = Speaker.introduce

setmetatable(Robot, {
	__call = function(_, init)
		return Robot.new(init)
	end,
})

print(Robot():introduce())
print(Robot().name ~= nil)
