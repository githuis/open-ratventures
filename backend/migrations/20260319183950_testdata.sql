-- Add migration script here
-- Add migration script here
PRAGMA foreign_keys = ON;

-- Admin test user
INSERT INTO users (username) VALUES ('admin');

-- Character for admin
INSERT INTO characters (user_id, experience, coins)
VALUES (1, 0, 0);

-- Unit for admin's character
INSERT INTO units (ref_id, health, energy, max_health, max_energy)
VALUES (1, 10, 15, 10, 15);