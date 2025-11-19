# SideQuestGST — Discord Quest Bot

A small Rust-based Discord bot + worker that uses Kafka and Google Sheets to manage quests for a Discord guild.

This repository contains two services and a Kafka broker configured in `docker-compose.yml`:

- `bot-gateway` — Discord bot service that receives commands and posts messages.
- `sheet-worker` — background worker that reads/writes Google Sheets and communicates via Kafka.
- `kafka` — local Apache Kafka used for inter-service messaging (defined in `docker-compose.yml`).

## Key features

- Discord command handling (commands live in `bot-gateway/src/commands/`).
- Google Sheets-backed persistence for quests and stats.
- Asynchronous processing using Kafka topics.

## Quick start (recommended)

1. Copy the environment template and fill in your secrets:

```bash
cp .env.template .env
# edit .env and fill DISCORD_TOKEN, GOOGLE_SHEET_ID, TARGET_GUILD_ID, QUEST_GIVER_ID
```

2. Provide Google service account credentials:

- Create a Google Cloud service account with the Google Sheets API enabled.
- Download the JSON key and save it as `credentials.json` in the project root (this repo mounts `/app/credentials.json` into containers).
- Share the Google Sheet with the service account email (give Editor permissions).

3. Start services with Docker Compose (builds images and starts Kafka + services):

```bash
docker compose up --build -d
```

4. View logs:

```bash
docker compose logs -f bot-gateway
docker compose logs -f sheet-worker
docker compose logs -f kafka
```

5. To stop and remove containers:

```bash
docker compose down
```

Notes:
- `docker-compose.yml` mounts `./credentials.json` into `/app/credentials.json` and sets `GOOGLE_APPLICATION_CREDENTIALS=/app/credentials.json` for containers.
- Kafka in the compose file exposes the broker to the host on port `59092` and uses `kafka:9093` for inter-service communication inside the compose network.

## Environment variables

The repo contains `.env.template` with the required variables. Fill these in `.env` or export in your shell for local runs:

- `DISCORD_TOKEN` — your Discord bot token.
- `TARGET_GUILD_ID` — the ID of the guild/server the bot targets.
- `GOOGLE_SHEET_ID` — the Google Sheets spreadsheet ID that the worker uses.
- `QUEST_GIVER_ID` — role id or other id used by the bot for quest assignments.

Service-specific runtime environment (see `docker-compose.yml`):
- `KAFKA_BROKERS` — the Kafka broker(s). In compose it's `kafka:9093` for containers.
- `GOOGLE_APPLICATION_CREDENTIALS` — inside container `/app/credentials.json`.
- `RUST_LOG` — set logging level (e.g., `info`, `debug`).

## Local development (without Docker)

Prerequisites:
- Rust toolchain (rustup/cargo)
- A running Kafka broker (you can run the `kafka` service from the compose file)
- `credentials.json` placed at project root and `GOOGLE_SHEET_ID` set

Build and run each service directly:

```bash
# bot gateway
cd bot-gateway
export DISCORD_TOKEN=... # from Discord dev portal
export KAFKA_BROKERS=kafka:9093
export GOOGLE_SHEET_ID=...
export GOOGLE_APPLICATION_CREDENTIALS=../credentials.json
export RUST_LOG=debug
cargo run --release

# sheet worker (in a separate terminal)
cd ../sheet-worker
export KAFKA_BROKERS=kafka:9093
export GOOGLE_SHEET_ID=...
export GOOGLE_APPLICATION_CREDENTIALS=../credentials.json
export RUST_LOG=debug
cargo run --release
```

If you run locally without Docker, ensure `KAFKA_BROKERS` points to a reachable Kafka broker. The compose setup is the easiest way to get Kafka locally.

## Build Docker images (optional)

To build images yourself and push to a registry (the compose file references images hosted at GHCR):

```bash
# build locally
docker build -t gst-quest-bot:local ./bot-gateway
docker build -t gst-sheet-worker:local ./sheet-worker

# Tag & push to registry (example GHCR)
# docker tag gst-quest-bot:local ghcr.io/<user>/sidequestgst/gst-quest-bot:latest
# docker push ghcr.io/<user>/sidequestgst/gst-quest-bot:latest
```

## Project structure

Top-level files and folders:

- `docker-compose.yml` — orchestrates Kafka + services for local development.
- `.env.template` — example environment variables.
- `credentials.json` — (not in repo) Google service account key. Mounts into containers.
- `bot-gateway/` — Discord bot service (Cargo project).
  - `src/commands/` — various bot commands.
- `sheet-worker/` — Google Sheets worker (Cargo project).

Both `bot-gateway` and `sheet-worker` are self-contained Rust crates with `Cargo.toml` files.

## Google Sheets setup

1. Enable the Google Sheets API for a GCP project.
2. Create a service account and download the JSON key to `credentials.json`.
3. Add the service account email (found in the JSON) as an Editor on the target spreadsheet (Share → grant Editor access).

Without these steps the worker cannot read/write the spreadsheet.

## Discord setup

1. Create a Discord application and bot in the Developer Portal.
2. Invite the bot to your server with appropriate permissions (send messages, manage roles if needed).
3. Provide the bot token as `DISCORD_TOKEN`.

## Troubleshooting

- Kafka healthchecks failing:
  - Make sure nothing else is using the same host port (59092). Use `docker compose logs kafka` to inspect errors.
  - Compose healthcheck in `docker-compose.yml` waits on Kafka internal listener; allow extra start time if host is slow.

- Google Sheets permissions errors:
  - Ensure the service account email has Editor permission on the sheet.
  - Ensure `credentials.json` is valid and the `GOOGLE_APPLICATION_CREDENTIALS` path points to the file inside the container.

- Discord authentication errors:
  - Confirm `DISCORD_TOKEN` is correct and bot is invited to the right guild.
  - Check `RUST_LOG` for hints (`export RUST_LOG=debug`).

- If you see `permission denied` when reading credentials inside the container, ensure the file is readable by the container (compose mounts it read-only by default).

## Logs & debugging

- Use `docker compose logs -f <service>` to follow logs.
- The services use `RUST_LOG` for runtime logging. Set it to `debug` to see more verbose output.

## Contributing

Contributions are welcome. Suggested workflow:

1. Fork the repo.
2. Work on a branch with changes.
3. Run and test locally.
4. Submit a pull request with a concise description of your changes.

## Commands

All bot commands are implemented as slash commands. Times must use the format `YYYY-MM-DD HH:MM` and are interpreted as WIB (UTC+7). Quest IDs are UUIDs shown in the bot embeds and in `/list`.

- `/create` (Quest-role or admins)
  - Opens a modal to create a quest.
  - Slash options: `category` (select), `division` (select), `community_name` (optional if category is Community).
  - Modal fields: Quest Name, Description & Platform/Location (first line = platform), Participant Slots, Start Time, Deadline (optional).
  - The bot posts an embed with the generated quest ID. Footer: "Use /take <id> to take the quest".

- `/edit <quest_id>` (Quest-role or admins)
  - Opens a modal to edit an existing quest. Leave fields empty to keep current values.
  - Modal fields: New Title, Description & Platform/Location, Participant Slots, Start Time, Deadline.

- `/delete <quest_id>` (Quest-role or admins)
  - Sends a delete request for the quest. The bot verifies the quest exists before sending the request.

- `/take <quest_id>` (Guild members)
  - Register yourself as a participant for the quest.
  - Bot checks current participants and available slots; returns confirmation or error (already taken / full).

- `/drop <quest_id>` (Guild members)
  - Drop a quest you previously took. Only allowed when the participant status is `ON_PROGRESS` and before the quest start time.

- `/submit <quest_id> <attachment:image>` (Guild members)
  - Submit image proof for a taken quest. Only accepts image attachments (jpg/png/etc.). Produces a submit event with the attachment URL.

- `/list` (Guild members)
  - Shows the quest board in a paginated view with title, quest ID, slots status, organizer and start time.

- `/stats` (Guild members)
  - Sends a DM to the user with their active/completed/failed quest counts and active quest list.

- `/help` (Guild members)
  - Shows the help for all available commands.

- `/register_community <name> [leader]` (Admins only)
  - Admin command to register a new community. Produces a `REGISTER_COMMUNITY` event.

How to find a quest ID:
- The quest ID is shown in the quest embed created by `/create` and in entries printed by `/list`. Copy that UUID for use with `/take`, `/drop`, `/edit`, or `/delete`.

Common errors & tips:
- "This command doesnt work on DMs" — run the command in the target guild/server.
- "Access Denied" for staff commands — ensure you have the configured quest staff role or Administrator permission.
- Wrong time format — use exactly `YYYY-MM-DD HH:MM` (16 chars) and include minutes.
- For `/submit` attach an image file; other file types are rejected.
