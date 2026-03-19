-- Add migration script here
-- Add migration script here
PRAGMA foreign_keys = ON;

-- Users
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE
);

-- Characters
CREATE TABLE characters (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    experience INTEGER NOT NULL DEFAULT 0,
    coins INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Units
CREATE TABLE units (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ref_id INTEGER NOT NULL,
    health INTEGER NOT NULL,
    energy INTEGER NOT NULL,
    max_health INTEGER NOT NULL,
    max_energy INTEGER NOT NULL,
    FOREIGN KEY (ref_id) REFERENCES characters(id)
);

-- Items
CREATE TABLE items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL
);