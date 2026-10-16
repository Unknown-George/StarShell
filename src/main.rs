use std::env;

use anyhow::{bail, Context as AnyhowContext, Result};
use rand::Rng;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use serenity::{
    all::{
        Command, CommandInteraction, Context, CreateCommand, CreateCommandOption,
        CreateInteractionResponse, CreateInteractionResponseMessage, EditInteractionResponse,
        EventHandler, GatewayIntents, GuildId, Interaction, Ready,
    },
    async_trait,
};
use tracing::{error, info};

const LINUX_ASSISTANT_PROMPT: &str = r#"
You are a careful Linux, Unix, macOS, BSD, and operating-systems help assistant inside Discord.
Answer the user's question directly and technically accurately.

Rules:
- Explain commands, flags, filesystems, permissions, processes, networking, shells, package managers,
  services, logs, containers, and operating-system concepts.
- Prefer portable POSIX solutions, then clearly label Linux-specific, macOS-specific, BSD-specific,
  or distribution-specific differences.
- Give copyable commands in fenced code blocks.
- Explain what each important command and flag does.
- For commands that can delete data, change permissions, alter disks, kill processes, expose secrets,
  or change networking, include a short WARNING and safer inspection/dry-run steps first.
- Never claim that you ran a command or inspected the user's machine. You cannot access their computer.
- Do not invent output. Tell the user what output would generally mean and ask them to paste their output
  when diagnosis requires it.
- If the request is ambiguous, state the assumption and give the safest useful answer.
- Keep Discord responses readable: use headings, short paragraphs, and concise examples.
"#;

#[derive(Clone)]
struct LinuxAssistant {
    http: HttpClient,
    base_url: String,
    api_key: String,
    model: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
}

#[derive(Clone, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

impl LinuxAssistant {
    fn from_environment() -> Result<Self> {
        let base_url = env::var("AI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_owned())
            .trim_end_matches('/')
            .to_owned();
        let api_key = env::var("AI_API_KEY").unwrap_or_default();
        let model = env::var("AI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_owned());

        if api_key.is_empty() && !base_url.contains("localhost") && !base_url.contains("127.0.0.1") {
            bail!("AI_API_KEY is missing; set it in .env, or configure a local Ollama endpoint")
        }

        Ok(Self {
            http: HttpClient::new(),
            base_url,
            api_key,
            model,
        })
    }

    async fn answer(&self, question: &str) -> Result<String> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_owned(),
                    content: LINUX_ASSISTANT_PROMPT.to_owned(),
                },
                ChatMessage {
                    role: "user".to_owned(),
                    content: question.to_owned(),
                },
            ],
            temperature: 0.2,
        };

        let mut request_builder = self
            .http
            .post(format!("{}/chat/completions", self.base_url))
            .json(&request);

        if !self.api_key.is_empty() {
            request_builder = request_builder.bearer_auth(&self.api_key);
        }

        let response = request_builder
            .send()
            .await
            .context("could not reach the AI provider")?;
        let status = response.status();
        let body = response
            .text()
            .await
            .context("could not read the AI provider response")?;

        if !status.is_success() {
            bail!("AI provider returned HTTP {status}: {body}")
        }

        let parsed: ChatResponse =
            serde_json::from_str(&body).context("AI provider returned invalid JSON")?;
        let answer = parsed
            .choices
            .first()
            .map(|choice| choice.message.content.trim().to_owned())
            .filter(|answer| !answer.is_empty())
            .context("AI provider returned no answer")?;

        Ok(answer)
    }
}

struct Handler {
    assistant: LinuxAssistant,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!(user = %ready.user.name, "Discord bot is connected");

        if let Err(error) = register_commands(&ctx.http).await {
            error!(%error, "Could not register slash commands");
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let Interaction::Command(command) = interaction else {
            return;
        };

        if let Err(error) = handle_command(&ctx, &command, &self.assistant).await {
            error!(command = %command.data.name, %error, "Command failed");

            let _ = command
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new()
                        .content("Something went wrong. Check the bot log for details."),
                )
                .await;
        }
    }
}

fn command_definitions() -> Vec<CreateCommand> {
    vec![
        CreateCommand::new("ping").description("Check whether the bot is online."),
        CreateCommand::new("hello").description("Say hello to the bot."),
        CreateCommand::new("roll").description("Roll a six-sided die."),
        CreateCommand::new("about").description("Show information about this bot."),
        CreateCommand::new("ask")
            .description("Ask a Linux, Unix, macOS, BSD, or OS question.")
            .add_option(
                CreateCommandOption::new(
                    serenity::all::CommandOptionType::String,
                    "question",
                    "Your operating-system or command-line question.",
                )
                .required(true),
            ),
    ]
}

async fn register_commands(http: &serenity::http::Http) -> Result<()> {
    let commands = command_definitions();

    match env::var("DISCORD_GUILD_ID") {
        Ok(value) if !value.trim().is_empty() => {
            let guild_id: u64 = value
                .parse()
                .with_context(|| format!("DISCORD_GUILD_ID is not a valid server ID: {value}"))?;

            GuildId::new(guild_id)
                .set_commands(http, commands)
                .await
                .context("failed to register guild slash commands")?;

            info!(guild_id, "Registered slash commands for development server");
        }
        _ => {
            Command::set_global_commands(http, commands)
                .await
                .context("failed to register global slash commands")?;

            info!("Registered global slash commands");
        }
    }

    Ok(())
}

async fn handle_command(
    ctx: &Context,
    command: &CommandInteraction,
    assistant: &LinuxAssistant,
) -> Result<()> {
    if command.data.name == "ask" {
        let question = command
            .data
            .options
            .iter()
            .find(|option| option.name == "question")
            .and_then(|option| option.value.as_str())
            .context("the question option was missing")?;

        // AI requests can take longer than Discord's initial response window.
        command.defer(&ctx.http).await?;
        let answer = assistant.answer(question).await?;
        let answer = truncate_for_discord(answer);

        command
            .edit_response(&ctx.http, EditInteractionResponse::new().content(answer))
            .await
            .context("failed to send AI answer")?;
        return Ok(());
    }

    let response = match command.data.name.as_str() {
        "ping" => "Pong! 🏓".to_owned(),
        "hello" => format!("Hello, {}! 👋", command.user.name),
        "roll" => {
            let roll = rand::thread_rng().gen_range(1..=6);
            format!("You rolled **{roll}** 🎲")
        }
        "about" => "I explain Linux, Unix, macOS, BSD, shells, commands, permissions, networking, and operating-system concepts. Use `/ask question:<your question>`. I explain commands but never execute them on your machine.".to_owned(),
        _ => "Unknown command.".to_owned(),
    };

    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().content(response),
            ),
        )
        .await
        .context("failed to send interaction response")?;

    Ok(())
}

fn truncate_for_discord(text: String) -> String {
    const DISCORD_MESSAGE_LIMIT: usize = 2_000;
    const SUFFIX: &str = "\n\n[Answer shortened for Discord]";

    if text.chars().count() <= DISCORD_MESSAGE_LIMIT {
        return text;
    }

    let keep = DISCORD_MESSAGE_LIMIT - SUFFIX.chars().count();
    let mut shortened: String = text.chars().take(keep).collect();
    shortened.push_str(SUFFIX);
    shortened
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            env::var("RUST_LOG").unwrap_or_else(|_| "info,serenity=warn".to_owned()),
        )
        .init();

    let token = env::var("DISCORD_TOKEN")
        .context("DISCORD_TOKEN is missing; copy .env.example to .env and set it")?;
    let assistant = LinuxAssistant::from_environment()?;

    let intents = GatewayIntents::GUILDS;
    let mut client = serenity::Client::builder(&token, intents)
        .event_handler(Handler { assistant })
        .await
        .context("failed to create Discord client")?;

    client.start().await.context("Discord client stopped")?;
    Ok(())
}
