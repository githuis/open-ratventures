-- Add migration script here

-- Enable foreign keys
PRAGMA foreign_keys = ON;

-- Users
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE
);

-- Stats
CREATE TABLE stats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    health INTEGER NOT NULL,
    energy INTEGER NOT NULL
);

-- Units
CREATE TABLE units (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    stats_id INTEGER NOT NULL,
    max_stats_id INTEGER NOT NULL,
    FOREIGN KEY (stats_id) REFERENCES stats(id),
    FOREIGN KEY (max_stats_id) REFERENCES stats(id)
);

-- Characters
CREATE TABLE characters (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    unit_id INTEGER NOT NULL,
    experience INTEGER NOT NULL DEFAULT 0,
    coins INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (unit_id) REFERENCES units(id)
);

-- Items
CREATE TABLE items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL
);