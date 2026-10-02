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
  marriage, divorce and remarriage, immigration, names, households and where
  people live (down to census tracts), from 1840 to 2100. Everyone has parents,
  children and siblings. Nothing is simulated ahead of time or stored per
  person. Each fact is worked out from the seed and the person's id when you
  ask for it.
- The `directory` service serves it over MCP: `read_person` and
  `read_household` give anyone's names, family, unions, household and address
  at any date.
- World definitions (rates, names, places) live in data files under `worlds/`,
  one folder per world. `us` is the default; `us-areas` tracks where people
  live in the population model itself and is much larger to build.

## Layout

| Crate | What it does |
|---|---|
| `procedural_core` | Building blocks: keyed hashes, permutations, samplers, fitting, bit layouts and search |
| `internot_def` | Loads and merges the world definitions in `worlds/` |
| `internot_society` | The population, family, household and residence model |
| `internot` | The services and their tools |
| `internot_mcp` | The MCP server, which exposes every tool in `internot` |
| `internot_perf` | Benchmarks and the performance gate |

## Building and running

You need a recent stable Rust toolchain.

```sh
cargo build --release
cargo test --workspace
```

Start the MCP server (it talks over stdio). It builds the world first, which
takes about 7 seconds for `us`:

```sh
cargo run --release -p internot_mcp --bin internot-mcp
```

`INTERNOT_PACK` picks the world (`us`, `us-areas`, `us-tiny`, ...) and
`INTERNOT_SEED` the seed (default 42).

Build a sample world and compare it with US demographic data:

```sh
cargo run --release -p internot_society --example realism_report
```

That world has about 17 million people born over its history and builds in
about 6 seconds on a 16-thread laptop.

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
