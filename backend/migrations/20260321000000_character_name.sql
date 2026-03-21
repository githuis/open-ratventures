ALTER TABLE characters ADD COLUMN name TEXT NOT NULL DEFAULT 'Gorgrond';
UPDATE characters SET name = 'Gorgrond' WHERE name = '';
