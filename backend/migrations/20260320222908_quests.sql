PRAGMA foreign_keys = ON;

-- Active quests
-- status: 'active' = in progress, 'completed' = all encounters finished, 'failed' = party wiped
CREATE TABLE quests (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    current_encounter INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'completed', 'failed'))
);

-- Join table: which characters are in which quest (replaces Vec<i32> on Party)
CREATE TABLE quest_members (
    quest_id INTEGER NOT NULL,
    character_id INTEGER NOT NULL,
    slot_index INTEGER NOT NULL,
    PRIMARY KEY (quest_id, character_id),
    FOREIGN KEY (quest_id) REFERENCES quests(id) ON DELETE CASCADE,
    FOREIGN KEY (character_id) REFERENCES characters(id) ON DELETE CASCADE
);
