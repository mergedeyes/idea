use std::{
    env::args, // Arguments
    process::exit, // Exiting prematurely
    fs::File, // File tools, like create
    path::PathBuf, // Paths.. obv
    io::{self, Write}, // For interactive y/N confirmation prompts
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Idea {
    id: u64,
    text: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Topic {
    id: u64,
    name: String,
    ideas: Vec<Idea>,
}

type Store = Vec<Topic>;

fn print_help() {
    println!("   HOW TO USE IDEA");
    println!("About: idea is a simple cli tool that lets you save important ideas, that you might forget about in 20s ...");
    println!("Usage:");
    println!("  idea \"<Topic>\" \"<Idea>\"           Save a new idea under a topic (default topic: \"no topic\")");
    println!("  idea list                          List all topics and ideas, with their ids");
    println!("  idea search <topic> [idea]         Search topics/ideas (see below)");
    println!("  idea delete <topic_id>              Delete an entire topic, including all its ideas");
    println!("  idea delete <topic_id> <idea_id>   Delete a specific idea by id");
    println!("  idea defrag                        Renumber all ids to close gaps (1, 2, 3, ...), order preserved");
    println!();
    println!("Search forms:");
    println!("  idea search 1 \"foo\"       Search for \"foo\" only within topic id 1");
    println!("  idea search 0 \"foo\"       Search for \"foo\" across all topics");
    println!("  idea search \"work\" \"foo\"  Search for \"foo\" within topics whose name contains \"work\"");
    println!("  idea search \"work\"        List all ideas in topics whose name contains \"work\"");
    println!();
    println!("Ideas are saved as json in '~/.config/idea/ideas.json'.");
    println!("Topic and idea ids are permanent: a new one gets (current highest id in its scope) + 1, and");
    println!("existing ids never shift when something else is added or removed. Deleting leaves gaps in the");
    println!("numbering; run 'idea defrag' to compact everything back to 1, 2, 3, ... (order is preserved).");
}

// Converts the pre-id file format ({ "topic name": ["idea", ...], ... }) into the new Store, assigning ids sequentially in the order the old map yields its entries.
fn migrate_old_format(old: serde_json::Map<String, serde_json::Value>) -> Store {
    let mut store: Store = Vec::new();
    let mut next_topic_id: u64 = 1;
    for (topic_name, ideas_value) in old.into_iter() {
        let mut ideas = Vec::new();
        let mut next_idea_id: u64 = 1;
        if let Some(arr) = ideas_value.as_array() {
            for idea_val in arr {
                if let Some(text) = idea_val.as_str() {
                    ideas.push(Idea { id: next_idea_id, text: text.to_string() });
                    next_idea_id += 1;
                }
            }
        }
        store.push(Topic { id: next_topic_id, name: topic_name, ideas });
        next_topic_id += 1;
    }
    store
}

fn save_store(path: &PathBuf, store: &Store) {
    let pretty_json_string = serde_json::to_string_pretty(store)
        .unwrap_or_else(|_| panic!("Failed to format JSON data into a string!"));
    std::fs::write(path, pretty_json_string)
        .unwrap_or_else(|_| panic!("Failed to write data to {}", path.display()));
}

fn load_store(path: &PathBuf) -> Store {
    if let Some(parent_dir) = path.parent() {
        std::fs::create_dir_all(parent_dir)
            .unwrap_or_else(|_| panic!("Failed to create configuration directory!"));
    }

    if !path.exists() {
        File::create(path).unwrap_or_else(|_| panic!("Failed to create file {}", path.display()));
        println!("Created a new ideas.json file!");
        return Vec::new();
    }

    let file_contents = std::fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("Failed to read file {}", path.display()));

    if file_contents.trim().is_empty() {
        return Vec::new();
    }

    match serde_json::from_str::<Store>(&file_contents) {
        Ok(store) => store,
        Err(_) => {
            // Not the new array-of-topics shape; check whether it's the old object-of-topic-name-to-ideas shape and migrate it if so.
            let old: serde_json::Value = serde_json::from_str(&file_contents).unwrap_or_else(|_| {
                panic!(
                    "Content of {} is not valid JSON. Please check for syntax errors.",
                    path.display()
                )
            });
            let old_map = match old.as_object() {
                Some(map) => map.clone(),
                None => panic!("Content of {} doesn't match a known ideas format.", path.display()),
            };
            let had_data = !old_map.is_empty();
            let migrated = migrate_old_format(old_map);
            save_store(path, &migrated);
            if had_data {
                println!("Migrated {} to the new id-based format.", path.display());
            }
            migrated
        }
    }
}

fn next_id<T>(items: &[T], id_of: impl Fn(&T) -> u64) -> u64 {
    items.iter().map(id_of).max().unwrap_or(0) + 1
}

fn add_idea(store: &mut Store, topic_name: &str, idea_text: &str) {
    if let Some(topic) = store.iter_mut().find(|t| t.name == topic_name) {
        let new_idea_id = next_id(&topic.ideas, |i| i.id);
        topic.ideas.push(Idea { id: new_idea_id, text: idea_text.to_string() });
    } else {
        let new_topic_id = next_id(store, |t| t.id);
        store.push(Topic {
            id: new_topic_id,
            name: topic_name.to_string(),
            ideas: vec![Idea { id: 1, text: idea_text.to_string() }],
        });
    }
}

fn list_ideas(store: &Store) {
    if store.is_empty() {
        println!("You have no saved ideas yet.");
        print_help();
        return;
    }
    for topic in store {
        println!("=== TOPIC {}: {} ===", topic.id, topic.name);
        for idea in &topic.ideas {
            println!(" [{}] {}", idea.id, idea.text);
        }
        println!();
    }
}

// Decides whether a topic matches a search selector:
// - "0"      -> matches every topic ("search across everything")
// - a number -> matches the topic with that permanent id
// - anything else -> case-insensitive substring match against the topic name
fn matches_topic_selector(selector: &str, topic: &Topic) -> bool {
    match selector.parse::<u64>() {
        Ok(0) => true,
        Ok(n) => n == topic.id,
        Err(_) => topic.name.to_lowercase().contains(selector.to_lowercase().as_str()),
    }
}

fn search_ideas(store: &Store, topic_selector: &str, idea_query: Option<&str>) {
    let mut found_any = false;
    for topic in store {
        if !matches_topic_selector(topic_selector, topic) {
            continue;
        }
        let mut header_printed = false;
        for idea in &topic.ideas {
            let idea_matches = match idea_query {
                Some(query) => idea.text.to_lowercase().contains(query.to_lowercase().as_str()),
                None => true,
            };
            if idea_matches {
                if !header_printed {
                    println!("=== TOPIC {}: {} ===", topic.id, topic.name);
                    header_printed = true;
                }
                println!(" [{}] {}", idea.id, idea.text);
                found_any = true;
            }
        }
        if header_printed {
            println!();
        }
    }
    if !found_any {
        println!("No matching ideas found.");
    }
}

fn delete_idea(store: &mut Store, topic_id: u64, idea_id: u64) -> Result<(String, String), String> {
    let topic = store
        .iter_mut()
        .find(|t| t.id == topic_id)
        .ok_or_else(|| format!("No topic found with id {}.", topic_id))?;
    let pos = topic
        .ideas
        .iter()
        .position(|i| i.id == idea_id)
        .ok_or_else(|| {
            format!(
                "No idea found with id {} in topic '{}' ({} idea(s)).",
                idea_id,
                topic.name,
                topic.ideas.len()
            )
        })?;
    let removed = topic.ideas.remove(pos);
    Ok((topic.name.clone(), removed.text))
}

fn delete_topic(store: &mut Store, topic_id: u64) -> Result<Topic, String> {
    let pos = store
        .iter()
        .position(|t| t.id == topic_id)
        .ok_or_else(|| format!("No topic found with id {}.", topic_id))?;
    Ok(store.remove(pos))
}

fn confirm(prompt: &str) -> bool {
    print!("{} [y/N]: ", prompt);
    io::stdout().flush().ok();
    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return false;
    }
    matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
}

fn run_delete_topic(store: &mut Store, topic_id_str: &str) {
    let topic_id: u64 = match topic_id_str.parse() {
        Ok(n) if n > 0 => n,
        _ => {
            println!("Error: topic id must be a positive integer.");
            exit(1);
        }
    };

    let topic = match store.iter().find(|t| t.id == topic_id) {
        Some(t) => t,
        None => {
            println!("Error: No topic found with id {}.", topic_id);
            exit(1);
        }
    };

    if !topic.ideas.is_empty() {
        let proceed = confirm(&format!(
            "Topic \"{}\" (id {}) still has {} idea(s) in it. Delete it and all its ideas?",
            topic.name,
            topic.id,
            topic.ideas.len()
        ));
        if !proceed {
            println!("Aborted. Nothing was deleted.");
            exit(0);
        }
    }

    match delete_topic(store, topic_id) {
        Ok(topic) => println!(
            "Deleted topic \"{}\" (id {}) and {} idea(s) inside it.",
            topic.name,
            topic_id,
            topic.ideas.len()
        ),
        Err(e) => {
            println!("Error: {}", e);
            exit(1);
        }
    }
}

fn run_delete(store: &mut Store, topic_id_str: &str, idea_id_str: &str) {
    let topic_id: u64 = match topic_id_str.parse() {
        Ok(n) if n > 0 => n,
        _ => {
            println!("Error: topic id must be a positive integer.");
            exit(1);
        }
    };
    let idea_id: u64 = match idea_id_str.parse() {
        Ok(n) if n > 0 => n,
        _ => {
            println!("Error: idea id must be a positive integer.");
            exit(1);
        }
    };

    match delete_idea(store, topic_id, idea_id) {
        Ok((topic, idea)) => println!("Deleted idea [{}] \"{}\" from topic \"{}\".", idea_id, idea, topic),
        Err(e) => {
            println!("Error: {}", e);
            exit(1);
        }
    }
}

// Renumbers every topic id and every idea id (within its topic) to 1, 2, 3, ..., closing any gaps left by deletions. Relative order (by current id) is preserved.
fn defrag(store: &mut Store) {
    store.sort_by_key(|t| t.id);
    for (topic_index, topic) in store.iter_mut().enumerate() {
        topic.id = (topic_index + 1) as u64;
        topic.ideas.sort_by_key(|i| i.id);
        for (idea_index, idea) in topic.ideas.iter_mut().enumerate() {
            idea.id = (idea_index + 1) as u64;
        }
    }
}

fn main() {
    let arguments: Vec<String> = args().collect();
    if arguments.len() == 1 {
        println!("No arguments found, exiting.");
        print_help();
        exit(1)
    }

    let mut path: PathBuf = dirs::config_dir()
        .expect("Could not find the system configuration directory!");
    path.push("idea");
    path.push("ideas.json");

    let mut store = load_store(&path);

    if arguments.len() == 2 && arguments[1] == "list" {
        list_ideas(&store);
        exit(0);
    }

    if arguments[1] == "search" {
        match arguments.len() {
            3 => {
                search_ideas(&store, &arguments[2], None);
                exit(0);
            }
            4 => {
                search_ideas(&store, &arguments[2], Some(&arguments[3]));
                exit(0);
            }
            _ => {
                println!("Error: Invalid usage of search.");
                print_help();
                exit(1);
            }
        }
    }

    if arguments[1] == "delete" {
        match arguments.len() {
            3 => {
                run_delete_topic(&mut store, &arguments[2]);
                save_store(&path, &store);
                exit(0);
            }
            4 => {
                run_delete(&mut store, &arguments[2], &arguments[3]);
                save_store(&path, &store);
                exit(0);
            }
            _ => {
                println!("Error: Invalid usage of delete.");
                print_help();
                exit(1);
            }
        }
    }

    if arguments.len() == 2 && arguments[1] == "defrag" {
        defrag(&mut store);
        save_store(&path, &store);
        println!("Renumbered all topics and ideas.");
        exit(0);
    }

    let (topic, idea) = match arguments.len() {
        2 => {
            let default_topic = "no topic".to_string();
            let user_idea = arguments[1].clone();
            (default_topic, user_idea)
        }
        3 => {
            let user_topic = arguments[1].clone();
            let user_idea = arguments[2].clone();
            (user_topic, user_idea)
        }
        _ => {
            println!("Error: Invalid number of arguments.");
            print_help();
            exit(1);
        }
    };

    add_idea(&mut store, &topic, &idea);
    save_store(&path, &store);
    println!("Saved your idea!");
}
