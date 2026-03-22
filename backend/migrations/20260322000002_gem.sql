ALTER TABLE items DROP COLUMN effect_type;
ALTER TABLE items ADD COLUMN effect_type TEXT NOT NULL DEFAULT 'damage';

INSERT INTO items (name, description, effect_type, effect_value, consumable)
VALUES ('Gem of Resurrection', 'A glowing gem pulsing with life. Restores you to full health.', 'full_heal', 0, 1);
