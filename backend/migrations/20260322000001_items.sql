DROP TABLE IF EXISTS character_items;
DROP TABLE IF EXISTS items;

CREATE TABLE items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    effect_type TEXT NOT NULL,
    effect_value INTEGER NOT NULL,
    consumable INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE character_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
    item_id INTEGER NOT NULL REFERENCES items(id),
    quantity INTEGER NOT NULL DEFAULT 1,
    UNIQUE(character_id, item_id)
);

INSERT INTO items (name, description, effect_type, effect_value, consumable)
VALUES ('Poisoned Dagger', 'A blade coated in rat poison. Deals 12 damage.', 'damage', 12, 1);
