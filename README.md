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
### Alternatively you can download the idea binary from the latest release page.

## Usage

### Add an idea

```sh
idea "Rust" "look into io_uring for the async runtime"
```

If you skip the topic, it goes under the default topic `no topic`:

```sh
idea "remember to defrost the freezer"
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
idea search "rust"            # all ideas in topics whose name contains "rust"
```

The topic selector is a number (an exact topic id, or `0` for "every topic") or a case-insensitive substring match on the topic name. The idea filter, when given, is a case-insensitive substring match on the idea text.

### Delete

```sh
idea delete 2 1        # delete idea id 1 inside topic id 2
idea delete 2          # delete topic id 2 entirely, including its ideas
```

Deleting a topic that still contains ideas asks for confirmation first (default: no):

```
Topic "Guitar" (id 2) still has 1 idea(s) in it. Delete it and all its ideas? [y/N]:
```

An empty topic deletes immediately without prompting.

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
