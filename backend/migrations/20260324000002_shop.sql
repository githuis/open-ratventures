CREATE TABLE shop_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id INTEGER NOT NULL UNIQUE REFERENCES items(id) ON DELETE CASCADE,
    cost INTEGER NOT NULL,
    stock INTEGER -- NULL = unlimited
);

INSERT INTO shop_items (item_id, cost, stock)
SELECT id, 5, NULL FROM items WHERE name = 'Gem of Resurrection';

INSERT INTO shop_items (item_id, cost, stock)
SELECT id, 8, NULL FROM items WHERE name = 'Poisoned Dagger';

INSERT INTO shop_items (item_id, cost, stock)
SELECT id, 1, NULL FROM items WHERE name = 'Brown Smelly Dart';
