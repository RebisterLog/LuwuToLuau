-- Luwu -> Luau: bundles and restructuring.
--
-- A bundle `.(3, 4)` groups a fixed number of values as one immutable value.
-- Luau has no such primitive, so the grouping becomes an ordinary array and
-- the values are read by index: `point.1` is `point[1]`.
--
-- Two bundle features have no direct equivalent and are simplified:
--   * value equality and value hashing -- a table compares by identity, so a
--     bundle used as a table key is translated to a string key built from its
--     parts ("ada,1") instead of a hashed tuple;
--   * immutability -- a plain table can be changed, and only convention keeps
--     it fixed.
--
-- Restructuring, `.{ name, age }`, is the same shorthand table as in the
-- destructuring example: `{ name = name, age = age }`.

const point = { 3, 4 }
const x, y = point[1], point[2]
print(point[1], x + y)

const name, age = "Ada", 36
const person = { name = name, age = age }
print(person.name, person.age)

const scores = {}
scores["ada,1"] = "gold"
print(scores["ada,1"])
