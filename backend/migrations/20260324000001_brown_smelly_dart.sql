INSERT INTO items (name, description, effect_type, effect_value, charges)
SELECT 'Brown Smelly Dart', 'Smells kinda funky, and seems to be a little soft too actually.', 'damage', 3, 1
WHERE NOT EXISTS (SELECT 1 FROM items WHERE name = 'Brown Smelly Dart');
