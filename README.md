# idea

A tiny command-line tool for capturing ideas before you forget them, organized by topic, stored locally as JSON.

## What it's for

You're mid-task and a thought worth keeping shows up: a project idea, a thing to look up later, a line for a song. `idea` lets you get it down in one command, filed under a topic, without breaking your flow. Later you can list, search, or clean up what you've collected.

Every topic and every idea has a permanent numeric id, but that id is only unique among its own siblings - a topic's direct sub-topics number their own 1, 2, 3, ..., and each topic's own ideas number their own 1, 2, 3, ..., independently of every other topic (see Ids below). Ids never shift when you add or remove things elsewhere in your list, so a delete you queue up based on `idea list` output stays valid even if you add new ideas in between.

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

You can also give a chain of topic names to file an idea under a sub-topic, walked/created the
same way as `add-topic` (see Sub-topics below):

```sh
idea Programming Rust WebDev "look into async traits"
```

If every topic argument is a number instead, it's treated as an existing topic's id chain (see
Ids below), not a name:

```sh
idea 2 1 "look into async traits"     # files under whatever topic is at id chain "2 1"
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

A nested topic's header shows its full id chain, not just its own local id - see Ids below.

### Search

```sh
idea search 1 "io_uring"      # only within top-level topic id 1 (and its sub-topics)
idea search 0 "io_uring"      # across all topics
idea search "rust" "pin"      # topics whose name contains "rust", filtered by idea text
idea search "rust"            # topics AND ideas searched for "rust" at once
```

The topic selector is a number (an exact topic id, or `0` for "every topic") or a case-insensitive substring match on the topic name. The idea filter, when given, is a case-insensitive substring match on the idea text.

With a single search argument, it's matched against both topics and ideas at once: a topic whose name (or id) matches has all of its ideas listed, and any idea whose text matches is listed regardless of its topic.

Unlike `edit`/`delete`, a numeric search selector is a single id, not a chain - so it matches that id wherever it occurs in the tree. Since ids are only unique among their own siblings, that can match more than one topic if two of them (under different parents) happen to share the same local id; each match is printed with its own full id chain so they're easy to tell apart.

### Edit

```sh
idea edit 1        # rename top-level topic id 1
idea edit 1 2       # edit idea id 2 in topic 1, or (if no such idea) rename its direct sub-topic id 2
idea edit 1 2 3     # one level deeper: id 3 inside topic 2, inside topic 1
```

One id renames a top-level topic. Two or more ids walk a chain of topic ids one level at a time down to the second-to-last one, then try the very last id as an idea living directly in it; if there's no such idea, it's tried as a direct sub-topic instead and renamed (see Ids below for how the chain works). Either way, the current name/text opens pre-filled on the line, ready to edit: change what you want and press Enter to save, or clear the line (or press Ctrl-C/Ctrl-D) to abort without changing anything.

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

`idea list` shows sub-topics indented under their parent, each labeled with its full id chain -
see Ids below.

Adding an idea works the same way: `idea "<topic>" "<idea>"` still matches/creates a single
top-level topic by name, exactly like before sub-topics existed - unless every `<topic>` argument
is a number, in which case it's treated as an existing topic's id chain (see Ids). Give more than
one topic name before the idea text (`idea <T1> <T2> ... "<idea>"`) and it's walked/created as a
chain just like `add-topic`, then the idea is filed under the last topic in the chain.

### Ids

A topic id is only unique among its own siblings - the same way an idea id is only unique within its own topic. The first sub-topic you create under any topic gets id 1, no matter how many topics or sub-topics already exist elsewhere in your list; the same is true one level further down, and so on. This keeps numbers small and meaningful (the 2nd sub-topic under "Work" is id 2, not some arbitrary number that also depends on everything else you've ever filed).

Because of that, reaching anything below the top level means giving the whole chain of ids down to it, one per level:

```sh
idea edit 2 4       # the topic (or idea) numbered 4, directly inside top-level topic 2
idea edit 2 4 6     # one level deeper: id 6, directly inside topic 4, which is inside topic 2
```

A single id on its own always means a top-level topic. `idea list` prints every topic's full id chain (e.g. `TOPIC 2 4: WebDev`), so you never have to count levels by hand - just read the chain straight off the listing and pass it to `edit`/`delete`.

The *last* id in a chain is tried as an idea living directly in the topic reached by the ids before it; if there's no such idea, it's tried as a direct sub-topic instead. That's how `idea edit 2 4` can mean either "idea 4 in topic 2" or "rename sub-topic 4 of topic 2", and how `idea delete ...` picks between deleting an idea or an entire sub-topic. If a topic happens to have *both* an idea and a direct sub-topic with the same id, the idea wins - address the sub-topic afterward (or after `defrag`) once that collision is gone.

`idea search`'s numeric selector doesn't take a chain (see Search above) - it's the one place a bare id can match more than one topic.

### Delete

```sh
idea delete 2 1        # delete idea id 1 inside topic id 2
idea delete 2 4        # topic 2 has no idea id 4, so this deletes its direct sub-topic id 4 instead
idea delete 2          # delete topic id 2 entirely, including its ideas and any sub-topics
idea delete 2 4 6      # one level deeper: delete idea/sub-topic 6 inside topic 4, inside topic 2
```

Same chain rules as `edit` (see Ids above). Either way, deleting a topic (directly, or found this way) that still contains ideas or sub-topics asks for confirmation first (default: no):

```
Topic "Guitar" (id 2) still has 1 idea(s) in it (counting sub-topics). Delete it and everything inside? [y/N]:
```

An empty topic (no ideas, no sub-topics) deletes immediately without prompting.

### Defrag

Deleting things leaves gaps in the numbering (e.g. topic ids `1, 3, 4` after deleting topic `2` from among its siblings). This is intentional, ids are permanent, not positions, so nothing else shifts. If you'd rather have the ids compact again:

```sh
idea defrag
```

This renumbers every topic's direct sub-topics back to `1, 2, 3, ...` and every topic's own ideas back to `1, 2, 3, ...`, independently at every level, preserving each level's existing relative order.

## Where data is stored

Ideas are saved as JSON in `~/.config/idea/ideas.json`. If the file doesn't exist yet, it's created automatically on first use.

PS: If you look through the code and are wondering:
If the file was created by an older version of this tool (the format before ids existed), it's migrated to the current format automatically the first time you run any command, and the migrated file is written back to disk immediately.
