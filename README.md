# idea

A tiny command-line tool for capturing ideas before you forget them, organized by topic, stored locally as JSON.

## What it's for

You're mid-task and a thought worth keeping shows up: a project idea, a thing to look up later, a line for a song. `idea` lets you get it down in one command, filed under a topic, without breaking your flow. Later you can list, search, or clean up what you've collected.

Every topic and every idea has a permanent numeric id. Ids never shift when you add or remove things elsewhere in your list, so a delete you queue up based on `idea list` output stays valid even if you add new ideas in between.

## Installation

Requires the Rust toolchain (`cargo`). Install it via [rustup](https://rustup.rs/) if you don't have it yet.

```sh
git clone https://github.com/mergedeyes/idea.git
cd idea
cargo build --release
```

This produces a binary at `target/release/idea`. Put it somewhere on your `PATH`, for example:

```sh
sudo cp target/release/idea /usr/local/bin/idea
```

## Usage

### Add an idea

```sh
idea "Rust" "look into io_uring for the async runtime"
```

If you skip the topic, it goes under the default topic `no topic`:

```sh
idea "remember to defrost the freezer"
```

You can also give a chain of topic names to file an idea under a sub-topic, walked/created the
same way as `add-topic` (see Sub-topics below):

```sh
idea Programming Rust WebDev "look into async traits"
```

### List everything

```sh
idea list
```

```
=== TOPIC 1: Rust ===
 [1] look into io_uring for the async runtime
 [2] understand pin/unpin

=== TOPIC 2: Guitar ===
 [1] practice barre chords
```

### Search

```sh
idea search 1 "io_uring"      # only within topic id 1
idea search 0 "io_uring"      # across all topics
idea search "rust" "pin"      # topics whose name contains "rust", filtered by idea text
idea search "rust"            # topics AND ideas searched for "rust" at once
```

The topic selector is a number (an exact topic id, or `0` for "every topic") or a case-insensitive substring match on the topic name. The idea filter, when given, is a case-insensitive substring match on the idea text.

With a single search argument, it's matched against both topics and ideas at once: a topic whose name (or id) matches has all of its ideas listed, and any idea whose text matches is listed regardless of its topic.

### Edit

```sh
idea edit 1      # rename topic id 1
idea edit 1 2    # edit the text of idea id 2 in topic id 1
```

One id after `edit` renames a topic; two ids edit a specific idea's text. Either way, the
current name/text opens pre-filled on the line, ready to edit: change what you want and press
Enter to save, or clear the line (or press Ctrl-C/Ctrl-D) to abort without changing anything.
Since topic ids are unique across the whole tree (see Sub-topics below), this works on a topic
at any depth.

### Sub-topics

Topics can nest inside other topics:

```sh
idea add-topic Programming Rust WebDev
```

This walks the names left to right. A name that already exists (checked among the current
level's own children) becomes the parent for the next name; a name that doesn't exist yet is
created as a sub-topic of the previous one. So if `Programming` already exists but `Rust` and
`WebDev` don't, this adds `Programming > Rust > WebDev` as a new chain of sub-topics under the
existing `Programming` topic. Running it again with names that all already exist just walks
down the chain without creating anything.

`idea list` shows sub-topics indented under their parent. Every id (topic or idea) still
behaves the way it does everywhere else in this tool: topic ids are unique across the entire
tree, so `search`, `edit`, and `delete` all reach a sub-topic just by its id, no matter how
deep it's nested.

Adding an idea works the same way: `idea "<topic>" "<idea>"` still matches/creates a single
top-level topic by name, exactly like before sub-topics existed - unless `<topic>` is a number,
in which case it addresses an existing topic's id at any depth. Give more than one topic name
before the idea text (`idea <T1> <T2> ... "<idea>"`) and it's walked/created as a chain just
like `add-topic`, then the idea is filed under the last topic in the chain.

### Delete

```sh
idea delete 2 1        # delete idea id 1 inside topic id 2
idea delete 2 4        # topic 2 has no idea id 4, so this deletes sub-topic id 4 instead
idea delete 2          # delete topic id 2 entirely, including its ideas and any sub-topics
```

The two-id form tries the second id as an idea living directly in that topic first; if there's
no such idea, it tries it as a sub-topic nested anywhere inside that topic instead. Either way,
deleting a topic (directly, or found this way) that still contains ideas or sub-topics asks for
confirmation first (default: no):

```
Topic "Guitar" (id 2) still has 1 idea(s) in it (counting sub-topics). Delete it and everything inside? [y/N]:
```

An empty topic (no ideas, no sub-topics) deletes immediately without prompting.

### Defrag

Deleting things leaves gaps in the numbering (e.g. topic ids `1, 3, 4` after deleting topic `2`). This is intentional, ids are permanent, not positions, so nothing else shifts. If you'd rather have the ids compact again:

```sh
idea defrag
```

This renumbers all topics to `1, 2, 3, ...` and, within each topic, all its ideas to `1, 2, 3, ...`, preserving the existing relative order.

## Where data is stored

Ideas are saved as JSON in `~/.config/idea/ideas.json`. If the file doesn't exist yet, it's created automatically on first use.

PS: If you look through the code and are wondering:
If the file was created by an older version of this tool (the format before ids existed), it's migrated to the current format automatically the first time you run any command, and the migrated file is written back to disk immediately.
