CREATE TABLE parties (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    leader_id INTEGER NOT NULL,
    FOREIGN KEY (leader_id) REFERENCES characters(id)
);

CREATE TABLE party_members (
    party_id INTEGER NOT NULL,
    character_id INTEGER NOT NULL,
    PRIMARY KEY (party_id, character_id),
    FOREIGN KEY (party_id) REFERENCES parties(id),
    FOREIGN KEY (character_id) REFERENCES characters(id)
);
