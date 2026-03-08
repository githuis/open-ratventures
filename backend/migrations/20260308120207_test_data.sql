-- Admin stats (current)
INSERT INTO stats (health, energy) VALUES (10, 10);
INSERT INTO stats (health, energy) VALUES (10, 10);

-- Create unit
INSERT INTO units (stats_id, max_stats_id)
VALUES (last_insert_rowid() - 1, last_insert_rowid());

-- Create user
INSERT INTO users (username) VALUES ('admin');

-- Create character
INSERT INTO characters (user_id, unit_id, experience, coins)
VALUES (
    last_insert_rowid(),
    (SELECT id FROM units ORDER BY id DESC LIMIT 1),
    0,
    0
);