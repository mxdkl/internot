# Internot

Internot is a made-up world for testing AI agents. People, their families and,
later, their mail, calendars and tasks all come from one deterministic
generator, so what an agent sees in one place agrees with what it sees in
another. A different seed gives a different world. The same seed gives the
same world every time.

Agents work with the world through tools served over MCP. A trace tool reports
what the agent changed during a session, so runs can be scored.

## Status

This is early work. The old services (mail, calendar, chat, files, tasks and
money) were removed because they weren't good enough. They will be rebuilt on
top of a new population model.

What exists today:

- `internot_society` is the population model. It covers births, deaths,
  marriage, divorce and remarriage, and immigration, and gives everyone parents,
  children and siblings, from 1840 to 2100. Nothing is simulated ahead of time
  or stored per person. Each fact is worked out from the seed and the person's
  id when you ask for it.
- The `people` service in `internot` is older and has known problems (see
  `AGENTS.md`). The population model will replace it.

Households are next.

## Layout

| Crate | What it does |
|---|---|
| `procedural_core` | Building blocks: keyed hashes, permutations, samplers, bit layouts and search |
| `procedural_overlay` | Keeps an agent's changes for one session on top of the read-only world |
| `internot_society` | The population and family model |
| `internot` | The services and their tools |
| `internot_mcp` | The MCP server, which exposes every tool in `internot` |
| `internot_renderer` | Turns structured facts into text with an LLM (unused right now) |
| `internot_perf` | Benchmarks and the performance gate |

`mcp_harness/` is a Python harness that runs agent scenarios against the MCP
server. Most of its scenarios call tools that were removed, so read them as
examples for now.

## Building and running

You need a recent stable Rust toolchain.

```sh
cargo build --release
cargo test --workspace
```

Start the MCP server (it talks over stdio):

```sh
cargo run --release -p internot_mcp --bin internot-mcp
```

Build a sample world and compare it with US demographic data:

```sh
cargo run --release -p internot_society --example realism_report
```

That world has two regions and about 12 million people born over its history.
It builds in about a second and a half on a 16-thread laptop.

Run the tests and then the performance gate:

```sh
perf/check.sh
```

## More

`AGENTS.md` has the project notes: decisions, current status, known problems
and lessons learned. Designs are in `docs/superpowers/specs` and research notes
in `docs/superpowers/research`.

## License

MIT or Apache 2.0, at your option.
