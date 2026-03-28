INSERT INTO items (name, description, effect_type, effect_value, charges)
VALUES ('Vitality Concoction', 'A murky bottle of something that smells faintly of copper and herbs. Restores 20 health.', 'heal', 20, 1);

INSERT INTO shop_items (item_id, cost, stock)
SELECT id, 5, NULL FROM items WHERE name = 'Vitality Concoction';
