-- Luwu -> Luau: try expressions.
--
-- `try f(x)` runs f and gives Ok(value) or Err(error) -- no closure, no
-- `local success, value = pcall(...)` boilerplate.
--
-- Luau has no Result type, so the builder stays with pcall and keeps both
-- return values. The `is Ok.(value)` / `is Err.(problem)` checks become plain
-- branches on the pcall flag, and the error value is just what pcall returned.

const function parse_number(text)
	const value = tonumber(text)
	if value == nil then
		error(`not a number: {text}`, 2)
	end
	return value
end

const ok_ok, ok_value = pcall(parse_number, "42")
if ok_ok then
	print(`parsed {ok_value}`)
end

const bad_ok, bad_problem = pcall(parse_number, "nope")
if not bad_ok then
	print(`failed: {bad_problem}`)
end

print(ok_ok, ok_value)
print(bad_ok, bad_problem)
