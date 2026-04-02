INSERT INTO items (name, description, effect_type, effect_value, charges)
SELECT 'Ratweave Shard Net', 'Tightly woven from matted rat hair and stuffed with broken glass. Unpleasant to throw, worse to receive.', 'damage', 10, 1
WHERE NOT EXISTS (SELECT 1 FROM items WHERE name = 'Ratweave Shard Net');

INSERT INTO shop_items (item_id, cost, stock)
SELECT id, 4, NULL FROM items WHERE name = 'Ratweave Shard Net'
AND NOT EXISTS (SELECT 1 FROM shop_items WHERE item_id = (SELECT id FROM items WHERE name = 'Ratweave Shard Net'));
