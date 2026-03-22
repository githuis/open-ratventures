ALTER TABLE items DROP COLUMN consumable;
ALTER TABLE items ADD COLUMN charges INTEGER NOT NULL DEFAULT 1;

UPDATE items SET charges = 1 WHERE name IN ('Poisoned Dagger', 'Gem of Resurrection');
UPDATE items SET charges = 3 WHERE name IN ('Rat Tooth Shiv', 'Sewer Shiv', 'Gnawed Bone Club', 'Rusty Nail Flail', 'Cheese Grater Blade');

ALTER TABLE character_items DROP COLUMN quantity;
ALTER TABLE character_items ADD COLUMN charges_remaining INTEGER NOT NULL DEFAULT 1;
