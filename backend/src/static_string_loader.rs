use std::{collections::HashMap, sync::Arc};
use ratback::quest_data::{Dialogue, MissionDef, Monster};

pub fn load_enemies() -> Arc<Vec<Monster>> {
    let candidates = ["backend/data/enemies", "data/enemies"];
    let dir = candidates.iter().map(std::path::Path::new).find(|p| p.exists());

    let mut monsters = Vec::new();
    if let Some(dir) = dir {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if entry.path().extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                match std::fs::read_to_string(entry.path())
                    .ok()
                    .and_then(|s| serde_json::from_str::<Monster>(&s).ok())
                {
                    Some(m) => monsters.push(m),
                    None => eprintln!("Failed to parse enemy '{}'", entry.path().display()),
                }
            }
        }
    }
    println!("Loaded {} enemy templates", monsters.len());
    Arc::new(monsters)
}

pub fn load_dialogues() -> Arc<HashMap<String, Dialogue>> {
    let candidates = ["backend/data/dialogues", "data/dialogues"];
    let dir = candidates.iter().map(std::path::Path::new).find(|p| p.exists());

    let mut map = HashMap::new();
    if let Some(dir) = dir {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "json").unwrap_or(false) {
                    let id = path.file_stem().unwrap().to_string_lossy().to_string();
                    match std::fs::read_to_string(&path) {
                        Err(e) => println!("Failed to read {id}: {e}"),
                        Ok(content) => match serde_json::from_str::<Dialogue>(&content) {
                            Ok(dialogue) => { map.insert(id, dialogue); }
                            Err(e) => println!("Failed to parse dialogue '{id}': {e}"),
                        },
                    }
                }
            }
        }
    }
    println!("Loaded {} dialogues: {:?}", map.len(), map.keys().collect::<Vec<_>>());
    Arc::new(map)
}

pub fn load_missions() -> Arc<Vec<MissionDef>> {
    let candidates = ["backend/data/missions", "data/missions"];
    let dir = candidates.iter().map(std::path::Path::new).find(|p| p.exists());

    let mut missions = Vec::new();
    if let Some(dir) = dir {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "json").unwrap_or(false) {
                    match std::fs::read_to_string(&path)
                        .ok()
                        .and_then(|s| serde_json::from_str::<MissionDef>(&s).ok())
                    {
                        Some(m) => missions.push(m),
                        None => eprintln!("Failed to parse mission '{}'", path.display()),
                    }
                }
            }
        }
    }
    missions.sort_by(|a, b| a.id.cmp(&b.id));
    println!("Loaded {} missions", missions.len());
    Arc::new(missions)
}
