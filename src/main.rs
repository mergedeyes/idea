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
    println!("  idea edit <id> [<id> ...]          Rename a topic, or edit an idea's text (see Ids below)");
    println!("  idea add-topic <name> [<name> ...] Create/walk a chain of (sub-)topics, see below");
    println!("  idea delete <id> [<id> ...]        Delete a topic or an idea (see Ids below)");
    println!("  idea defrag                        Renumber all ids to close gaps (1, 2, 3, ...), order preserved");
    println!();
    println!("Search forms:");
    println!("  idea search 1 \"foo\"       Search for \"foo\" only within top-level topic id 1 (and its sub-topics)");
    println!("  idea search 0 \"foo\"       Search for \"foo\" across all topics");
    println!("  idea search \"work\" \"foo\"  Search for \"foo\" within topics whose name contains \"work\"");
    println!("  idea search \"work\"        Search topics AND idea text for \"work\" at the same time");
    println!();
    println!("Ids:");
    println!("  A topic id is only unique among its own siblings - the same way an idea id is only unique");
    println!("  within its own topic. The first sub-topic you create under any topic gets id 1, no matter");
    println!("  how many topics exist elsewhere; the same is true one level further down, and so on.");
    println!("  Reaching anything below the top level means giving the whole chain of ids down to it, one");
    println!("  per level: 'idea edit 2 4' means \"the topic (or idea) numbered 4, directly inside top-level");
    println!("  topic 2\"; 'idea edit 2 4 6' goes one level deeper (id 6 inside topic 4, inside topic 2). A");
    println!("  single id on its own always means a top-level topic. 'idea list' prints every topic's full");
    println!("  id chain, so you can read off exactly what to type instead of counting levels by hand.");
    println!("  The last id in a chain is tried as an idea living directly in the topic reached by the ids");
    println!("  before it; if there's no such idea, it's tried as a direct sub-topic instead. That's how");
    println!("  'idea edit 2 4' can mean either \"idea 4 in topic 2\" or \"rename sub-topic 4 of topic 2\",");
    println!("  and how 'idea delete ...' picks between deleting an idea or an entire sub-topic.");
    println!("  'idea edit' opens the current name/text pre-filled on the line: edit it and press Enter to");
    println!("  save, or clear the line (or press Ctrl-C/Ctrl-D) to abort without changing anything.");
    println!("  If every <topic> in 'idea \"<topic>\" \"<idea>\"' is a number, it's treated as an existing");
    println!("  topic's id chain; otherwise it's matched/created by name (see below).");
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
    println!("Ids are permanent: a new one gets (current highest id among its own siblings) + 1, and existing");
    println!("ids never shift when something else is added or removed elsewhere. Deleting leaves gaps in the");
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

// Total idea count in a topic, including all ideas in every sub-topic beneath it.
fn count_ideas_recursive(topic: &Topic) -> usize {
    topic.ideas.len() + topic.sub_topics.iter().map(count_ideas_recursive).sum::<usize>()
}

// Total number of sub-topics beneath a topic (not counting the topic itself), at any depth.
fn count_sub_topics_recursive(topic: &Topic) -> usize {
    topic.sub_topics.iter().map(|t| 1 + count_sub_topics_recursive(t)).sum()
}

fn id_path_string(path: &[u64]) -> String {
    path.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(" ")
}

// Walks a chain of topic ids one level at a time: ids[0] must match a top-level topic (a
// sibling among `store` itself, where ids are unique); each id after that must match one of the
// *direct* sub-topics of the topic reached by the ids before it (again, unique only among those
// siblings). Unlike a flat "search anywhere" lookup, this can never land on a topic at the wrong
// depth, which matters now that the same id can occur more than once across different branches.
fn resolve_path<'a>(store: &'a [Topic], ids: &[u64]) -> Result<&'a Topic, String> {
    let (first, rest) = match ids.split_first() {
        Some(parts) => parts,
        None => return Err("Empty id path.".to_string()),
    };
    let topic = store
        .iter()
        .find(|t| t.id == *first)
        .ok_or_else(|| format!("No top-level topic found with id {}.", first))?;
    resolve_path_within(topic, rest)
}

fn resolve_path_within<'a>(topic: &'a Topic, ids: &[u64]) -> Result<&'a Topic, String> {
    let (id, rest) = match ids.split_first() {
        Some(parts) => parts,
        None => return Ok(topic),
    };
    let child = topic.sub_topics.iter().find(|t| t.id == *id).ok_or_else(|| {
        format!(
            "Topic \"{}\" has no sub-topic with id {} ({} sub-topic(s)).",
            topic.name,
            id,
            topic.sub_topics.len()
        )
    })?;
    resolve_path_within(child, rest)
}

fn resolve_path_mut<'a>(store: &'a mut Store, ids: &[u64]) -> Result<&'a mut Topic, String> {
    let (first, rest) = match ids.split_first() {
        Some(parts) => parts,
        None => return Err("Empty id path.".to_string()),
    };
    let topic = store
        .iter_mut()
        .find(|t| t.id == *first)
        .ok_or_else(|| format!("No top-level topic found with id {}.", first))?;
    resolve_path_within_mut(topic, rest)
}

fn resolve_path_within_mut<'a>(topic: &'a mut Topic, ids: &[u64]) -> Result<&'a mut Topic, String> {
    let (id, rest) = match ids.split_first() {
        Some(parts) => parts,
        None => return Ok(topic),
    };
    // Find the position first (immutable borrow, ends here) rather than iter_mut().find(...),
    // so the error branch can still read topic.name/sub_topics.len() without a borrow conflict.
    let pos = match topic.sub_topics.iter().position(|t| t.id == *id) {
        Some(p) => p,
        None => {
            return Err(format!(
                "Topic \"{}\" has no sub-topic with id {} ({} sub-topic(s)).",
                topic.name,
                id,
                topic.sub_topics.len()
            ));
        }
    };
    resolve_path_within_mut(&mut topic.sub_topics[pos], rest)
}

// Removes and returns the topic at `ids` (see `resolve_path`): taken directly out of the
// top-level `store` vec if `ids` is a single id, or out of its parent's `sub_topics` otherwise.
fn remove_topic_at_path(store: &mut Store, ids: &[u64]) -> Result<Topic, String> {
    let (last, parent_path) = match ids.split_last() {
        Some(parts) => parts,
        None => return Err("Empty id path.".to_string()),
    };
    if parent_path.is_empty() {
        let pos = store
            .iter()
            .position(|t| t.id == *last)
            .ok_or_else(|| format!("No top-level topic found with id {}.", last))?;
        return Ok(store.remove(pos));
    }
    let parent = resolve_path_mut(store, parent_path)?;
    let pos = parent.sub_topics.iter().position(|t| t.id == *last).ok_or_else(|| {
        format!(
            "Topic \"{}\" has no sub-topic with id {} ({} sub-topic(s)).",
            parent.name,
            last,
            parent.sub_topics.len()
        )
    })?;
    Ok(parent.sub_topics.remove(pos))
}

// Walks a chain of topic names left to right, starting at `topics` (the top level, on the
// first call). A name that already exists among the current level's own children becomes the
// parent context for the next name; a name that doesn't exist yet is created as a new
// sub-topic of the previous one in the chain (or at the top level, for the very first name),
// with the next free id among *that level's own siblings* - ids restart at 1 under every
// parent, see the Ids section of the help text. Returns a mutable reference to the topic named
// by the *last* element of `names`.
fn walk_topic_chain<'a>(topics: &'a mut Vec<Topic>, names: &[String], depth: usize) -> &'a mut Topic {
    let name = &names[0];
    let pos = match topics.iter().position(|t| &t.name == name) {
        Some(p) => {
            println!("Found existing topic \"{}\" (id {}).", topics[p].name, topics[p].id);
            p
        }
        None => {
            let new_id = next_id(topics, |t| t.id);
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
        walk_topic_chain(&mut topics[pos].sub_topics, &names[1..], depth + 1)
    }
}

fn add_topic_chain(store: &mut Store, names: &[String]) {
    walk_topic_chain(store, names, 0);
}

// Parses every element of `strs` as a positive-integer id; `None` if any element isn't one.
fn parse_all_ids(strs: &[String]) -> Option<Vec<u64>> {
    strs.iter().map(|s| s.parse::<u64>().ok().filter(|&n| n > 0)).collect()
}

// `topic_path` is one or more topic names forming a chain, e.g. ["Programming", "Rust"], OR (if
// every segment parses as a positive integer) a chain of topic ids: the first id addresses a
// top-level topic, and each id after that a *direct* sub-topic of the one before (see the Ids
// section of the help text). An id chain that doesn't resolve to an existing topic falls through
// and is treated as literal topic name(s) instead, same as the historical single-id shorthand.
fn add_idea(store: &mut Store, topic_path: &[String], idea_text: &str) {
    if let Some(ids) = parse_all_ids(topic_path) {
        if let Ok(topic) = resolve_path_mut(store, &ids) {
            let new_idea_id = next_id(&topic.ideas, |i| i.id);
            topic.ideas.push(Idea { id: new_idea_id, text: idea_text.to_string() });
            return;
        }
        // No topic has that id chain (yet) - fall through and treat it as literal topic name(s).
    }

    let topic = walk_topic_chain(store, topic_path, 0);
    let new_idea_id = next_id(&topic.ideas, |i| i.id);
    topic.ideas.push(Idea { id: new_idea_id, text: idea_text.to_string() });
}

fn list_ideas(store: &Store) {
    if store.is_empty() {
        println!("You have no saved ideas yet.");
        print_help();
        return;
    }
    let mut path = Vec::new();
    for topic in store {
        print_topic(topic, 0, &mut path);
    }
}

// Prints a topic and everything under it, indented by depth. The header shows the topic's full
// id chain (e.g. "2 4" for the 4th direct sub-topic of top-level topic 2) - exactly what to pass
// to `edit`/`delete`/etc. to reach it - not just its own local id.
fn print_topic(topic: &Topic, depth: usize, path: &mut Vec<u64>) {
    let indent = "  ".repeat(depth);
    path.push(topic.id);
    println!("{}=== TOPIC {}: {} ===", indent, id_path_string(path), topic.name);
    for idea in &topic.ideas {
        println!("{} [{}] {}", indent, idea.id, idea.text);
    }
    println!();
    for sub in &topic.sub_topics {
        print_topic(sub, depth + 1, path);
    }
    path.pop();
}

// Decides whether a topic matches a search selector:
// - "0"      -> matches every topic ("search across everything")
// - a number -> matches any topic with that local id (topic ids are only unique among their own
//                siblings - see the Ids section of the help text - so this can match more than
//                one topic if they happen to share an id under different parents; each match is
//                printed with its own full id chain so they're easy to tell apart)
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
    let mut path = Vec::new();
    search_ideas_recursive(store, topic_selector, idea_query, false, 0, &mut path, &mut found_any);
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
    path: &mut Vec<u64>,
    found_any: &mut bool,
) {
    let indent = "  ".repeat(depth);
    for topic in topics {
        path.push(topic.id);
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
                        println!("{}=== TOPIC {}: {} ===", indent, id_path_string(path), topic.name);
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
        search_ideas_recursive(&topic.sub_topics, topic_selector, idea_query, topic_matches, depth + 1, path, found_any);
        path.pop();
    }
}

// A single search argument is matched against both topics and ideas at once: a topic that
// matches the query (by id or name substring) has all of its ideas listed, and any idea whose
// text contains the query is listed regardless of whether its topic matched. Just like
// search_ideas_recursive, a match is inherited by descendants, and the walk always continues
// into sub-topics so a nested topic can match independently too.
fn search_all(store: &Store, query: &str) {
    let mut found_any = false;
    let mut path = Vec::new();
    search_all_recursive(store, query, false, 0, &mut path, &mut found_any);
    if !found_any {
        println!("No matching ideas found.");
    }
}

fn search_all_recursive(
    topics: &[Topic],
    query: &str,
    inherited_match: bool,
    depth: usize,
    path: &mut Vec<u64>,
    found_any: &mut bool,
) {
    let indent = "  ".repeat(depth);
    for topic in topics {
        path.push(topic.id);
        let topic_matches = inherited_match || matches_topic_selector(query, topic);
        let mut header_printed = false;
        for idea in &topic.ideas {
            let idea_matches = topic_matches || idea.text.to_lowercase().contains(query.to_lowercase().as_str());
            if idea_matches {
                if !header_printed {
                    println!("{}=== TOPIC {}: {} ===", indent, id_path_string(path), topic.name);
                    header_printed = true;
                }
                println!("{} [{}] {}", indent, idea.id, idea.text);
                *found_any = true;
            }
        }
        if header_printed {
            println!();
        }
        search_all_recursive(&topic.sub_topics, query, topic_matches, depth + 1, path, found_any);
        path.pop();
    }
}

fn rename_topic(topic: &mut Topic, new_name: &str) -> String {
    let old_name = topic.name.clone();
    topic.name = new_name.to_string();
    old_name
}

fn edit_idea_in(topic: &mut Topic, idea_id: u64, new_text: &str) -> Result<(String, String), String> {
    let topic_name = topic.name.clone();
    let idea_count = topic.ideas.len();
    let idea = topic.ideas.iter_mut().find(|i| i.id == idea_id).ok_or_else(|| {
        format!(
            "No idea found with id {} in topic '{}' ({} idea(s)).",
            idea_id, topic_name, idea_count
        )
    })?;
    let old_text = idea.text.clone();
    idea.text = new_text.to_string();
    Ok((topic_name, old_text))
}

fn delete_idea_from(topic: &mut Topic, idea_id: u64) -> Result<(String, String), String> {
    let pos = topic.ideas.iter().position(|i| i.id == idea_id).ok_or_else(|| {
        format!(
            "No idea found with id {} in topic '{}' ({} idea(s)).",
            idea_id, topic.name, topic.ideas.len()
        )
    })?;
    let removed = topic.ideas.remove(pos);
    Ok((topic.name.clone(), removed.text))
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

fn run_delete_topic_at(store: &mut Store, path: &[u64]) {
    let (name, idea_count, sub_topic_count) = {
        let topic = match resolve_path(store, path) {
            Ok(t) => t,
            Err(e) => {
                println!("Error: {}", e);
                exit(1);
            }
        };
        (topic.name.clone(), count_ideas_recursive(topic), count_sub_topics_recursive(topic))
    };
    let path_str = id_path_string(path);

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
            name,
            path_str,
            parts.join(" and ")
        ));
        if !proceed {
            println!("Aborted. Nothing was deleted.");
            exit(0);
        }
    }

    match remove_topic_at_path(store, path) {
        Ok(removed) => {
            let idea_count = count_ideas_recursive(&removed);
            let sub_topic_count = count_sub_topics_recursive(&removed);
            println!(
                "Deleted topic \"{}\" (id {}), {} idea(s) and {} sub-topic(s) inside it.",
                removed.name, path_str, idea_count, sub_topic_count
            );
        }
        Err(e) => {
            println!("Error: {}", e);
            exit(1);
        }
    }
}

// `idea delete <id> [<id> ...]`. A single id deletes a top-level topic. Two or more ids walk a
// path of topic ids (see `resolve_path`) to the second-to-last topic, then try the last id as an
// idea living directly in it first; if there's no such idea, try it as a direct sub-topic
// instead and delete that whole sub-topic (with the usual confirmation).
fn run_delete(store: &mut Store, ids: &[u64]) {
    if ids.len() == 1 {
        run_delete_topic_at(store, ids);
        return;
    }

    let (parent_path, second_id) = ids.split_at(ids.len() - 1);
    let second_id = second_id[0];

    let deleted_idea = match resolve_path_mut(store, parent_path) {
        Ok(topic) => delete_idea_from(topic, second_id).ok(),
        Err(e) => {
            println!("Error: {}", e);
            exit(1);
        }
    };
    if let Some((topic_name, idea_text)) = deleted_idea {
        println!("Deleted idea [{}] \"{}\" from topic \"{}\".", second_id, idea_text, topic_name);
        return;
    }

    let (topic_name, idea_count, sub_topic_count, is_direct_sub_topic) = {
        let topic = match resolve_path(store, parent_path) {
            Ok(t) => t,
            Err(e) => {
                println!("Error: {}", e);
                exit(1);
            }
        };
        (
            topic.name.clone(),
            topic.ideas.len(),
            topic.sub_topics.len(),
            topic.sub_topics.iter().any(|t| t.id == second_id),
        )
    };

    if !is_direct_sub_topic {
        println!(
            "Error: No idea and no sub-topic found with id {} in topic '{}' ({} idea(s), {} sub-topic(s)).",
            second_id, topic_name, idea_count, sub_topic_count
        );
        exit(1);
    }

    run_delete_topic_at(store, ids);
}

// Opens a line pre-filled with `current`, ready for the user to edit in place; Enter accepts
// the (possibly changed) line, Ctrl-C/Ctrl-D or any editor error is treated as a cancel.
fn prompt_edit(prompt: &str, current: &str) -> Option<String> {
    let mut editor = DefaultEditor::new().ok()?;
    editor.readline_with_initial(prompt, (current, "")).ok()
}

fn prompt_and_rename_topic(store: &mut Store, path: &[u64]) {
    let current_name = match resolve_path(store, path) {
        Ok(t) => t.name.clone(),
        Err(e) => {
            println!("Error: {}", e);
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

    let topic = resolve_path_mut(store, path).expect("topic vanished mid-edit");
    let old_name = rename_topic(topic, &new_name);
    println!("Renamed topic \"{}\" (id {}) to \"{}\".", old_name, id_path_string(path), new_name);
}

// `idea edit <id> [<id> ...]`. A single id renames a top-level topic. Two or more ids walk a
// path of topic ids (see `resolve_path`) to the second-to-last topic, then try the last id as an
// idea living directly in it first; if there's no such idea, try it as a direct sub-topic
// instead and rename that (same fallback `delete` uses to pick between an idea and a sub-topic).
fn run_edit(store: &mut Store, ids: &[u64]) {
    if ids.len() == 1 {
        prompt_and_rename_topic(store, ids);
        return;
    }

    let (parent_path, second_id) = ids.split_at(ids.len() - 1);
    let second_id = second_id[0];

    let (idea_text, is_direct_sub_topic, topic_name, idea_count, sub_topic_count) = {
        let topic = match resolve_path(store, parent_path) {
            Ok(t) => t,
            Err(e) => {
                println!("Error: {}", e);
                exit(1);
            }
        };
        (
            topic.ideas.iter().find(|i| i.id == second_id).map(|i| i.text.clone()),
            topic.sub_topics.iter().any(|t| t.id == second_id),
            topic.name.clone(),
            topic.ideas.len(),
            topic.sub_topics.len(),
        )
    };

    if let Some(current_text) = idea_text {
        let new_text = match prompt_edit("Idea: ", &current_text) {
            Some(text) if !text.trim().is_empty() => text,
            _ => {
                println!("Aborted. Nothing was changed.");
                exit(0);
            }
        };

        let topic = resolve_path_mut(store, parent_path).expect("topic vanished mid-edit");
        match edit_idea_in(topic, second_id, &new_text) {
            Ok((topic_name, old_text)) => println!(
                "Edited idea [{}] in topic \"{}\": \"{}\" -> \"{}\".",
                second_id, topic_name, old_text, new_text
            ),
            Err(e) => {
                println!("Error: {}", e);
                exit(1);
            }
        }
        return;
    }

    if !is_direct_sub_topic {
        println!(
            "Error: No idea and no sub-topic found with id {} in topic '{}' ({} idea(s), {} sub-topic(s)).",
            second_id, topic_name, idea_count, sub_topic_count
        );
        exit(1);
    }

    prompt_and_rename_topic(store, ids);
}

// Renumbers every topic id and every idea id back to 1, 2, 3, ..., closing any gaps left by
// deletions. Each level is renumbered independently - a topic's direct sub-topics get their own
// 1, 2, 3, ... and so does every topic's own ideas - since ids are only ever unique among their
// own siblings (see the Ids section of the help text). Order (by current id) is preserved at
// every level.
fn defrag(store: &mut Store) {
    defrag_topics(store);
}

fn defrag_topics(topics: &mut Vec<Topic>) {
    topics.sort_by_key(|t| t.id);
    for (index, topic) in topics.iter_mut().enumerate() {
        topic.id = (index + 1) as u64;
        topic.ideas.sort_by_key(|i| i.id);
        for (idea_index, idea) in topic.ideas.iter_mut().enumerate() {
            idea.id = (idea_index + 1) as u64;
        }
        defrag_topics(&mut topic.sub_topics);
    }
}

fn parse_id(s: &str) -> Result<u64, String> {
    match s.parse::<u64>() {
        Ok(n) if n > 0 => Ok(n),
        _ => Err(format!("id must be a positive integer (got '{}').", s)),
    }
}

fn parse_id_path_or_exit(strs: &[String]) -> Vec<u64> {
    strs.iter()
        .map(|s| match parse_id(s) {
            Ok(n) => n,
            Err(e) => {
                println!("Error: {}", e);
                exit(1);
            }
        })
        .collect()
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
        if arguments.len() < 3 {
            println!("Error: Invalid usage of edit.");
            print_help();
            exit(1);
        }
        let ids = parse_id_path_or_exit(&arguments[2..]);
        run_edit(&mut store, &ids);
        save_store(&path, &store);
        exit(0);
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
        if arguments.len() < 3 {
            println!("Error: Invalid usage of delete.");
            print_help();
            exit(1);
        }
        let ids = parse_id_path_or_exit(&arguments[2..]);
        run_delete(&mut store, &ids);
        save_store(&path, &store);
        exit(0);
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
