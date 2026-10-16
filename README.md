# Linux Help Discord Bot

A Rust Discord bot that answers Linux, Unix, macOS, BSD, shell, and operating-system questions through an OpenAI-compatible AI API.

It has these slash commands:

- `/ask question:<question>` — ask about commands, flags, permissions, filesystems, processes, networking, package managers, services, logs, shells, or OS concepts
- `/ping` — health check
- `/hello` — greeting
- `/roll` — test command
- `/about` — bot information

The bot explains commands. It does **not** execute arbitrary commands on the bot host or access the user's computer.

## Requirements

- Rust from <https://rustup.rs/>
- A Discord application and bot token
- An OpenAI-compatible AI provider, or local Ollama

## Discord setup

1. Open the [Discord Developer Portal](https://discord.com/developers/applications).
2. Create an application.
3. Open **Bot**, click **Reset Token**, and copy the token.
4. Open **OAuth2 → URL Generator**.
5. Select scopes `bot` and `applications.commands`.
6. Select **Send Messages** under bot permissions.
7. Open the generated URL and authorize the bot in your server.

Only the `GUILDS` gateway intent is required. No privileged intent is needed.

## Configure

```bash
cp .env.example .env
```

Set at least:

```dotenv
DISCORD_TOKEN=your_discord_bot_token
AI_BASE_URL=https://api.openai.com/v1
AI_API_KEY=your_ai_api_key
AI_MODEL=gpt-4o-mini
```

For fast command updates during development, set `DISCORD_GUILD_ID` to your test server ID. Leave it blank for global commands; global registration can take up to an hour to appear.

## Provider examples

### OpenAI

```dotenv
AI_BASE_URL=https://api.openai.com/v1
AI_API_KEY=your_openai_key
AI_MODEL=gpt-4o-mini
```

### OpenRouter

```dotenv
AI_BASE_URL=https://openrouter.ai/api/v1
AI_API_KEY=your_openrouter_key
AI_MODEL=meta-llama/llama-3.1-8b-instruct:free
```

### Local Ollama

Start Ollama and pull a model:

```bash
ollama pull llama3.2
```

Then use:

```dotenv
AI_BASE_URL=http://localhost:11434/v1
AI_API_KEY=ollama
AI_MODEL=llama3.2
```

The Ollama model must be running on the same machine as the bot. If the bot is hosted elsewhere, use a reachable provider endpoint instead.

## Run

```bash
cargo run
```

For a release build:

```bash
cargo build --release
./target/release/discord-rust-bot
```

## Examples

```text
/ask question:How do I find which process is using port 8080?
/ask question:What is the difference between chmod 755 and chmod 644?
/ask question:Why does systemd say my service failed?
/ask question:How do I install a package on Fedora, Ubuntu, and Arch?
/ask question:Explain this command: find . -type f -name '*.log' -delete
```

The assistant is instructed to call out destructive commands, suggest inspection or dry-run steps, explain OS differences, and never pretend it ran a command.

