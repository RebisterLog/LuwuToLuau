-- Luwu -> Luau: table comprehensions.
--
-- A comprehension is a loop, a filter and an append in one expression. The
-- Luau output is exactly those three parts spelled out: an empty array, a
-- `for` over the source, an `if` for the `when`, and `table.insert` for `give`.
--
-- The result is a fresh array in both languages, so nothing else changes.

const numbers = { 1, 2, 3, 4, 5, 6 }

const even_squares = {}
for _, n in numbers do
	if n % 2 == 0 then
		table.insert(even_squares, n * n)
	end
end
print(#even_squares, even_squares[1], even_squares[3])

const fruits = { "apple", "kiwi", "pear", "banana" }

const long_names = {}
for _, fruit in fruits do
	if #fruit > 3 then
		table.insert(long_names, string.upper(fruit))
	end
end
print(table.concat(long_names, ", "))

const people = {
	{ name = "Ada", age = 36 },
	{ name = "Grace", age = 17 },
	{ name = "Alan", age = 41 },
}

const adults = {}
for _, person in people do
	if person.age >= 18 then
		table.insert(adults, person.name)
	end
end
print(table.concat(adults, ", "))
