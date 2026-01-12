# memory_cli

A small helper for interacting with the manual observation API routes using the 3-layer workflow.

## Build

```
cargo run --bin memory_cli -- --help
```

## Usage

```
memory_cli \
  --base-url http://127.0.0.1:3010 \
  --admin-token $ADMIN_API_TOKEN \
  create --entry-type incident --title "Prometheus incident" --narrative @/tmp/narrative.txt
```

Subcommands:

- `create` – create a manual observation.
- `list` – list summaries.
- `search` – layer 1 index search.
- `timeline` – layer 2 timeline around anchor.
- `fetch` – layer 3 full details by IDs.

Run `memory_cli <subcommand> --help` for parameters.
