ALTER TABLE characters RENAME COLUMN experience TO renown;

-- Shop is consumables only; remove weapons
DELETE FROM shop_items WHERE item_id = (SELECT id FROM items WHERE name = 'Poisoned Dagger');
