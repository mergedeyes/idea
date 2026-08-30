use std::{
    env::args, // Arguments
    process::exit, // Exiting prematurely
    fs::File, // File tools, like create
    path::PathBuf, // Paths.. obv
    io::{self, Write}, // For interactive y/N confirmation prompts
};
use serde::{Deserialize, Serialize};
use rustyline::DefaultEditor;

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
    // Nested topics. Missing in files written before sub-topics existed; serde fills those in as empty.
    #[serde(default)]
    sub_topics: Vec<Topic>,
}

type Store = Vec<Topic>;

fn print_help() {
    println!("   HOW TO USE IDEA");
    println!("About: idea is a simple cli tool that lets you save important ideas, that you might forget about in 20s ...");
    println!("Usage:");
    println!("  idea \"<Topic>\" \"<Idea>\"           Save a new idea under a topic (default topic: \"no topic\")");
    println!("  idea <T1> <T2> ... \"<Idea>\"       Save under a chain of (sub-)topics, walked/created like add-topic");
    println!("  idea list                          List all topics and ideas, with their ids");
    println!("  idea search <topic> [idea]         Search topics/ideas (see below)");
    println!("  idea edit <topic_id>                Rename a topic (opens the current name to edit)");
    println!("  idea edit <topic_id> <idea_id>     Edit an idea's text (opens the current text to edit)");
    println!("  idea add-topic <name> [<name> ...] Create/walk a chain of (sub-)topics, see below");
    println!("  idea delete <topic_id>              Delete a topic (and everything nested in it)");
    println!("  idea delete <topic_id> <id>        Delete an idea by id, or (if no such idea) a nested");
    println!("                                      sub-topic by id, from inside that topic");
    println!("  idea defrag                        Renumber all ids to close gaps (1, 2, 3, ...), order preserved");
    println!();
    println!("Search forms:");
    println!("  idea search 1 \"foo\"       Search for \"foo\" only within topic id 1 (and its sub-topics)");
    println!("  idea search 0 \"foo\"       Search for \"foo\" across all topics");
    println!("  idea search \"work\" \"foo\"  Search for \"foo\" within topics whose name contains \"work\"");
    println!("  idea search \"work\"        Search topics AND idea text for \"work\" at the same time");
    println!();
    println!("Topic ids are unique across the whole tree, including sub-topics, so 'idea edit <id> ...',");
    println!("'idea delete <id> ...' and 'idea search <id> ...' all work on a topic at any depth by its id.");
    println!("'idea edit' opens the current name/text pre-filled on the line: edit it and press Enter to");
    println!("save, or clear the line (or press Ctrl-C/Ctrl-D) to abort without changing anything.");
    println!("If a single <topic> in 'idea \"<topic>\" \"<idea>\"' is a number, it's treated as an existing");
    println!("topic's id (found at any depth); otherwise it's matched/created by name (see below).");
    println!();
    println!("Sub-topics:");
    println!("  idea add-topic Programming Rust WebDev");
    println!("  idea Programming Rust WebDev \"look into async traits\"");
    println!("      Both walk a chain of names left to right the same way: a name that already exists");
    println!("      (checked among the current level's own children) becomes the parent for the next name;");
    println!("      a name that doesn't exist yet is created as a sub-topic of the previous one. So if");
    println!("      \"Programming\" already exists but \"Rust\" and \"WebDev\" don't, both commands add");
    println!("      Programming > Rust > WebDev as a new chain of sub-topics under \"Programming\" - the");
    println!("      first just creates the chain, the second also files an idea under the last topic in it.");
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
        store.push(Topic { id: next_topic_id, name: topic_name, ideas, sub_topics: Vec::new() });
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

// Highest topic id anywhere in the tree (including sub-topics, at any depth).
fn max_topic_id(topics: &[Topic]) -> u64 {
    topics
        .iter()
        .map(|t| t.id.max(max_topic_id(&t.sub_topics)))
        .max()
        .unwrap_or(0)
}

fn next_topic_id(topics: &[Topic]) -> u64 {
    max_topic_id(topics) + 1
}

// Finds a topic by id anywhere in the tree (a topic itself, or nested at any depth in its sub-topics).
fn find_topic<'a>(topics: &'a [Topic], topic_id: u64) -> Option<&'a Topic> {
    for topic in topics {
        if topic.id == topic_id {
            return Some(topic);
        }
        if let Some(found) = find_topic(&topic.sub_topics, topic_id) {
            return Some(found);
        }
    }
    None
}

fn find_topic_mut<'a>(topics: &'a mut [Topic], topic_id: u64) -> Option<&'a mut Topic> {
    for topic in topics.iter_mut() {
        if topic.id == topic_id {
            return Some(topic);
        }
        if let Some(found) = find_topic_mut(&mut topic.sub_topics, topic_id) {
            return Some(found);
        }
    }
    None
}

// Removes and returns a topic by id from wherever it lives in the tree (top level or nested).
fn remove_topic_by_id(topics: &mut Vec<Topic>, topic_id: u64) -> Option<Topic> {
    if let Some(pos) = topics.iter().position(|t| t.id == topic_id) {
        return Some(topics.remove(pos));
    }
    for topic in topics.iter_mut() {
        if let Some(found) = remove_topic_by_id(&mut topic.sub_topics, topic_id) {
            return Some(found);
        }
    }
    None
}

// Total idea count in a topic, including all ideas in every sub-topic beneath it.
fn count_ideas_recursive(topic: &Topic) -> usize {
    topic.ideas.len() + topic.sub_topics.iter().map(count_ideas_recursive).sum::<usize>()
}

// Total number of sub-topics beneath a topic (not counting the topic itself), at any depth.
fn count_sub_topics_recursive(topic: &Topic) -> usize {
    topic.sub_topics.iter().map(|t| 1 + count_sub_topics_recursive(t)).sum()
}

// Walks a chain of topic names left to right, starting at `topics` (the top level, on the
// first call). A name that already exists among the current level's own children becomes the
// parent context for the next name; a name that doesn't exist yet is created as a new
// sub-topic of the previous one in the chain (or at the top level, for the very first name).
// Returns a mutable reference to the topic named by the *last* element of `names`.
fn walk_topic_chain<'a>(
    topics: &'a mut Vec<Topic>,
    names: &[String],
    next_id_counter: &mut u64,
    depth: usize,
) -> &'a mut Topic {
    let name = &names[0];
    let pos = match topics.iter().position(|t| &t.name == name) {
        Some(p) => {
            println!("Found existing topic \"{}\" (id {}).", topics[p].name, topics[p].id);
            p
        }
        None => {
            let new_id = *next_id_counter;
            *next_id_counter += 1;
            topics.push(Topic {
                id: new_id,
                name: name.clone(),
                ideas: Vec::new(),
                sub_topics: Vec::new(),
            });
            if depth == 0 {
                println!("Created topic \"{}\" (id {}).", name, new_id);
            } else {
                println!("Created sub-topic \"{}\" (id {}).", name, new_id);
            }
            topics.len() - 1
        }
    };
    if names.len() == 1 {
        &mut topics[pos]
    } else {
        walk_topic_chain(&mut topics[pos].sub_topics, &names[1..], next_id_counter, depth + 1)
    }
}

fn add_topic_chain(store: &mut Store, names: &[String]) {
    let mut next_id_counter = next_topic_id(store);
    walk_topic_chain(store, names, &mut next_id_counter, 0);
}

// `topic_path` is one or more topic names forming a chain, e.g. ["Programming", "Rust"]. A
// single-segment path that parses as a number addresses an existing topic anywhere in the tree
// by id. Otherwise the path is walked/created exactly like `add-topic` (existing names are
// reused, missing ones are created as sub-topics of the previous segment), and the idea is
// added to the topic named by the last segment.
fn add_idea(store: &mut Store, topic_path: &[String], idea_text: &str) {
    if topic_path.len() == 1 {
        if let Ok(topic_id) = topic_path[0].parse::<u64>() {
            if let Some(topic) = find_topic_mut(store, topic_id) {
                let new_idea_id = next_id(&topic.ideas, |i| i.id);
                topic.ideas.push(Idea { id: new_idea_id, text: idea_text.to_string() });
                return;
            }
            // No topic has that id (yet) - fall through and treat it as a literal topic name.
        }
    }

    let mut next_id_counter = next_topic_id(store);
    let topic = walk_topic_chain(store, topic_path, &mut next_id_counter, 0);
    let new_idea_id = next_id(&topic.ideas, |i| i.id);
    topic.ideas.push(Idea { id: new_idea_id, text: idea_text.to_string() });
}

fn list_ideas(store: &Store) {
    if store.is_empty() {
        println!("You have no saved ideas yet.");
        print_help();
        return;
    }
    for topic in store {
        print_topic(topic, 0);
    }
}

fn print_topic(topic: &Topic, depth: usize) {
    let indent = "  ".repeat(depth);
    println!("{}=== TOPIC {}: {} ===", indent, topic.id, topic.name);
    for idea in &topic.ideas {
        println!("{} [{}] {}", indent, idea.id, idea.text);
    }
    println!();
    for sub in &topic.sub_topics {
        print_topic(sub, depth + 1);
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
    search_ideas_recursive(store, topic_selector, idea_query, false, 0, &mut found_any);
    if !found_any {
        println!("No matching ideas found.");
    }
}

// A topic matches on its own (via matches_topic_selector) or by inheriting a match from an
// ancestor that already matched; either way every idea under it is then a candidate, filtered
// further by idea_query. The walk still recurses into every topic's sub-topics regardless of
// whether that topic matched, so a sub-topic can match the selector on its own too.
fn search_ideas_recursive(
    topics: &[Topic],
    topic_selector: &str,
    idea_query: Option<&str>,
    inherited_match: bool,
    depth: usize,
    found_any: &mut bool,
) {
    let indent = "  ".repeat(depth);
    for topic in topics {
        let topic_matches = inherited_match || matches_topic_selector(topic_selector, topic);
        let mut header_printed = false;
        if topic_matches {
            for idea in &topic.ideas {
                let idea_matches = match idea_query {
                    Some(query) => idea.text.to_lowercase().contains(query.to_lowercase().as_str()),
                    None => true,
                };
                if idea_matches {
                    if !header_printed {
                        println!("{}=== TOPIC {}: {} ===", indent, topic.id, topic.name);
                        header_printed = true;
                    }
                    println!("{} [{}] {}", indent, idea.id, idea.text);
                    *found_any = true;
                }
            }
        }
        if header_printed {
            println!();
        }
        search_ideas_recursive(&topic.sub_topics, topic_selector, idea_query, topic_matches, depth + 1, found_any);
    }
}

// A single search argument is matched against both topics and ideas at once: a topic that
// matches the query (by id or name substring) has all of its ideas listed, and any idea whose
// text contains the query is listed regardless of whether its topic matched. Just like
// search_ideas_recursive, a match is inherited by descendants, and the walk always continues
// into sub-topics so a nested topic can match independently too.
fn search_all(store: &Store, query: &str) {
    let mut found_any = false;
    search_all_recursive(store, query, false, 0, &mut found_any);
    if !found_any {
        println!("No matching ideas found.");
    }
}

fn search_all_recursive(topics: &[Topic], query: &str, inherited_match: bool, depth: usize, found_any: &mut bool) {
    let indent = "  ".repeat(depth);
    for topic in topics {
        let topic_matches = inherited_match || matches_topic_selector(query, topic);
        let mut header_printed = false;
        for idea in &topic.ideas {
            let idea_matches = topic_matches || idea.text.to_lowercase().contains(query.to_lowercase().as_str());
            if idea_matches {
                if !header_printed {
                    println!("{}=== TOPIC {}: {} ===", indent, topic.id, topic.name);
                    header_printed = true;
                }
                println!("{} [{}] {}", indent, idea.id, idea.text);
                *found_any = true;
            }
        }
        if header_printed {
            println!();
        }
        search_all_recursive(&topic.sub_topics, query, topic_matches, depth + 1, found_any);
    }
}

fn edit_topic(store: &mut Store, topic_id: u64, new_name: &str) -> Result<String, String> {
    let topic = find_topic_mut(store, topic_id).ok_or_else(|| format!("No topic found with id {}.", topic_id))?;
    let old_name = topic.name.clone();
    topic.name = new_name.to_string();
    Ok(old_name)
}

fn edit_idea(store: &mut Store, topic_id: u64, idea_id: u64, new_text: &str) -> Result<(String, String), String> {
    let topic = find_topic_mut(store, topic_id).ok_or_else(|| format!("No topic found with id {}.", topic_id))?;
    let topic_name = topic.name.clone();
    let idea_count = topic.ideas.len();
    let idea = topic
        .ideas
        .iter_mut()
        .find(|i| i.id == idea_id)
        .ok_or_else(|| {
            format!(
                "No idea found with id {} in topic '{}' ({} idea(s)).",
                idea_id,
                topic_name,
                idea_count
            )
        })?;
    let old_text = idea.text.clone();
    idea.text = new_text.to_string();
    Ok((topic_name, old_text))
}

fn delete_idea(store: &mut Store, topic_id: u64, idea_id: u64) -> Result<(String, String), String> {
    let topic = find_topic_mut(store, topic_id).ok_or_else(|| format!("No topic found with id {}.", topic_id))?;
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
    remove_topic_by_id(store, topic_id).ok_or_else(|| format!("No topic found with id {}.", topic_id))
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

    let topic = match find_topic(store, topic_id) {
        Some(t) => t,
        None => {
            println!("Error: No topic found with id {}.", topic_id);
            exit(1);
        }
    };

    let idea_count = count_ideas_recursive(topic);
    let sub_topic_count = count_sub_topics_recursive(topic);

    if idea_count > 0 || sub_topic_count > 0 {
        let mut parts = Vec::new();
        if idea_count > 0 {
            parts.push(format!("{} idea(s)", idea_count));
        }
        if sub_topic_count > 0 {
            parts.push(format!("{} sub-topic(s)", sub_topic_count));
        }
        let proceed = confirm(&format!(
            "Topic \"{}\" (id {}) still has {} in it (counting sub-topics). Delete it and everything inside?",
            topic.name,
            topic.id,
            parts.join(" and ")
        ));
        if !proceed {
            println!("Aborted. Nothing was deleted.");
            exit(0);
        }
    }

    match delete_topic(store, topic_id) {
        Ok(removed) => {
            let idea_count = count_ideas_recursive(&removed);
            let sub_topic_count = count_sub_topics_recursive(&removed);
            println!(
                "Deleted topic \"{}\" (id {}), {} idea(s) and {} sub-topic(s) inside it.",
                removed.name, topic_id, idea_count, sub_topic_count
            );
        }
        Err(e) => {
            println!("Error: {}", e);
            exit(1);
        }
    }
}

// `idea delete <topic_id> <second_id>` first tries `second_id` as an idea id living directly
// in `topic_id`'s own ideas (the historical behavior). If there's no such idea, it tries
// `second_id` as a topic id nested anywhere inside `topic_id`'s subtree instead, and if that
// matches, deletes that whole sub-topic (with the same confirmation flow as a normal topic
// delete). This lets a chain like `idea delete <parent> <child>` reach a sub-topic directly.
fn run_delete(store: &mut Store, topic_id_str: &str, second_id_str: &str) {
    let topic_id: u64 = match topic_id_str.parse() {
        Ok(n) if n > 0 => n,
        _ => {
            println!("Error: topic id must be a positive integer.");
            exit(1);
        }
    };
    let second_id: u64 = match second_id_str.parse() {
        Ok(n) if n > 0 => n,
        _ => {
            println!("Error: id must be a positive integer.");
            exit(1);
        }
    };

    if let Ok((topic, idea)) = delete_idea(store, topic_id, second_id) {
        println!("Deleted idea [{}] \"{}\" from topic \"{}\".", second_id, idea, topic);
        return;
    }

    let topic = match find_topic(store, topic_id) {
        Some(t) => t,
        None => {
            println!("Error: No topic found with id {}.", topic_id);
            exit(1);
        }
    };
    let is_nested_sub_topic = find_topic(&topic.sub_topics, second_id).is_some();

    if !is_nested_sub_topic {
        println!(
            "Error: No idea and no sub-topic found with id {} in topic '{}' ({} idea(s), {} sub-topic(s)).",
            second_id,
            topic.name,
            topic.ideas.len(),
            count_sub_topics_recursive(topic)
        );
        exit(1);
    }

    run_delete_topic(store, second_id_str);
}

// Opens a line pre-filled with `current`, ready for the user to edit in place; Enter accepts
// the (possibly changed) line, Ctrl-C/Ctrl-D or any editor error is treated as a cancel.
fn prompt_edit(prompt: &str, current: &str) -> Option<String> {
    let mut editor = DefaultEditor::new().ok()?;
    editor.readline_with_initial(prompt, (current, "")).ok()
}

fn run_edit_topic(store: &mut Store, topic_id_str: &str) {
    let topic_id: u64 = match topic_id_str.parse() {
        Ok(n) if n > 0 => n,
        _ => {
            println!("Error: topic id must be a positive integer.");
            exit(1);
        }
    };

    let current_name = match find_topic(store, topic_id) {
        Some(t) => t.name.clone(),
        None => {
            println!("Error: No topic found with id {}.", topic_id);
            exit(1);
        }
    };

    let new_name = match prompt_edit("Topic name: ", &current_name) {
        Some(text) if !text.trim().is_empty() => text,
        _ => {
            println!("Aborted. Nothing was changed.");
            exit(0);
        }
    };

    match edit_topic(store, topic_id, &new_name) {
        Ok(old_name) => println!("Renamed topic \"{}\" (id {}) to \"{}\".", old_name, topic_id, new_name),
        Err(e) => {
            println!("Error: {}", e);
            exit(1);
        }
    }
}

fn run_edit(store: &mut Store, topic_id_str: &str, idea_id_str: &str) {
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

    let topic = match find_topic(store, topic_id) {
        Some(t) => t,
        None => {
            println!("Error: No topic found with id {}.", topic_id);
            exit(1);
        }
    };
    let current_text = match topic.ideas.iter().find(|i| i.id == idea_id) {
        Some(i) => i.text.clone(),
        None => {
            println!(
                "Error: No idea found with id {} in topic '{}' ({} idea(s)).",
                idea_id,
                topic.name,
                topic.ideas.len()
            );
            exit(1);
        }
    };

    let new_text = match prompt_edit("Idea: ", &current_text) {
        Some(text) if !text.trim().is_empty() => text,
        _ => {
            println!("Aborted. Nothing was changed.");
            exit(0);
        }
    };

    match edit_idea(store, topic_id, idea_id, &new_text) {
        Ok((topic, old_text)) => println!(
            "Edited idea [{}] in topic \"{}\": \"{}\" -> \"{}\".",
            idea_id, topic, old_text, new_text
        ),
        Err(e) => {
            println!("Error: {}", e);
            exit(1);
        }
    }
}

// Renumbers every topic id (globally unique across the whole tree, pre-order: a topic before
// its sub-topics, siblings in their current id order) and every idea id (within its own topic)
// to 1, 2, 3, ..., closing any gaps left by deletions.
fn defrag(store: &mut Store) {
    let mut next_id = 1u64;
    defrag_topics(store, &mut next_id);
}

fn defrag_topics(topics: &mut Vec<Topic>, next_id: &mut u64) {
    topics.sort_by_key(|t| t.id);
    for topic in topics.iter_mut() {
        topic.id = *next_id;
        *next_id += 1;
        topic.ideas.sort_by_key(|i| i.id);
        for (idea_index, idea) in topic.ideas.iter_mut().enumerate() {
            idea.id = (idea_index + 1) as u64;
        }
        defrag_topics(&mut topic.sub_topics, next_id);
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
                search_all(&store, &arguments[2]);
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

    if arguments[1] == "edit" {
        match arguments.len() {
            3 => {
                run_edit_topic(&mut store, &arguments[2]);
                save_store(&path, &store);
                exit(0);
            }
            4 => {
                run_edit(&mut store, &arguments[2], &arguments[3]);
                save_store(&path, &store);
                exit(0);
            }
            _ => {
                println!("Error: Invalid usage of edit.");
                print_help();
                exit(1);
            }
        }
    }

    if arguments[1] == "add-topic" {
        if arguments.len() < 3 {
            println!("Error: Invalid usage of add-topic.");
            print_help();
            exit(1);
        }
        let names: Vec<String> = arguments[2..].to_vec();
        add_topic_chain(&mut store, &names);
        save_store(&path, &store);
        exit(0);
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

    // Everything but the last argument is a topic path (walked/created just like add-topic);
    // the last argument is the idea text. `idea "text"` alone falls back to "no topic".
    let (topic_path, idea_text): (Vec<String>, String) = if arguments.len() == 2 {
        (vec!["no topic".to_string()], arguments[1].clone())
    } else {
        let last = arguments.len() - 1;
        (arguments[1..last].to_vec(), arguments[last].clone())
    };

    add_idea(&mut store, &topic_path, &idea_text);
    save_store(&path, &store);
    println!("Saved your idea!");
}
