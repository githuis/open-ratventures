ALTER TABLE quests ADD COLUMN mission_id TEXT;

CREATE TABLE character_clues (
    character_id INTEGER NOT NULL,
    clue_id      TEXT NOT NULL,
    PRIMARY KEY (character_id, clue_id),
    FOREIGN KEY (character_id) REFERENCES characters(id)
);

CREATE TABLE character_missions (
    character_id INTEGER NOT NULL,
    mission_id   TEXT NOT NULL,
    completed    INTEGER NOT NULL DEFAULT 0,
    quest_id     INTEGER,
    PRIMARY KEY (character_id, mission_id),
    FOREIGN KEY (character_id) REFERENCES characters(id),
    FOREIGN KEY (quest_id) REFERENCES quests(id)
);
