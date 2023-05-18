use crate::events_db::{Event, EventsDB};
use anyhow::Result;
use chrono::Utc;
use chrono_english::Dialect;
use chrono_tz::Tz;
use mongodb::bson::DateTime;
use teloxide::{
    prelude::*,
    utils::command::{BotCommands, ParseError},
};

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "snake_case",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "get help")]
    Help,
    #[command(description = "<topic-name> - subscribe current chat to specified topic")]
    Subscribe(String),
    #[command(description = "<topic-name> - unsubscribe current chat from specified topic")]
    Unsubscribe(String),
    #[command(
        description = "<topic-name>, <date> - schedule replied message to be sent at specified date",
        parse_with = parse_args,
    )]
    Schedule(String, String, Option<String>),
    #[command(
        description = "<timezone> - timezone to use for current chat. Default is `Europe/Kiev`"
    )]
    SetTimezone(String),
}

async fn handle_bot_commands(bot: Bot, db: EventsDB, msg: Message, cmd: Command) -> Result<()> {
    match cmd {
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
                .await?;

            return Ok(());
        }
        Command::Subscribe(channel) => {
            db.subscribe(msg.chat.id.0, channel).await?;
        }
        Command::Unsubscribe(channel) => {
            db.unsubscribe(msg.chat.id.0, channel).await?;
        }
        Command::Schedule(channel, notify_at_str, interval) => {
            let event_message = msg
                .reply_to_message()
                .and_then(|reply| reply.text())
                .map(|text| text.to_string());

            if event_message.is_none() {
                bot.send_message(
                    msg.chat.id,
                    "Err: You must reply to a message with event text",
                )
                .await?;
                return Ok(());
            }
            let event_message = event_message.unwrap();

            let timezone = db
                .get_chat_timezone(msg.chat.id.0)
                .await?
                .and_then(|t| t.timezone.parse::<Tz>().ok())
                .unwrap_or(Tz::Europe__Kiev);

            let notify_at = chrono_english::parse_date_string(
                &notify_at_str,
                Utc::now().with_timezone(&timezone),
                Dialect::Uk,
            );
            if notify_at.is_err() {
                bot.send_message(msg.chat.id, "Err: Invalid date").await?;
                return Ok(());
            }
            let notify_at = notify_at.unwrap();

            if let Some(interval_str) = &interval {
                if let Err(err) = parse_duration::parse(interval_str) {
                    bot.send_message(msg.chat.id, format!("Err(Invalid interval): {err}"))
                        .await?;
                    return Ok(());
                }
            }

            db.add_event(Event {
                id: msg.id.0,
                channel,
                message: event_message,
                notify_at: DateTime::from_chrono(notify_at),
                interval,
            })
            .await?;
        }
        Command::SetTimezone(timezone) => {
            if let Err(err_msg) = timezone.parse::<Tz>() {
                bot.send_message(
                    msg.chat.id,
                    format!(
                        "Err(Invalid timezone): {err_msg}.\n\
                        \n\
                        See: https://en.wikipedia.org/wiki/List_of_tz_database_time_zones"
                    ),
                )
                .await?;
                return Ok(());
            }

            db.set_chat_timezone(msg.chat.id.0, timezone).await?;
        }
    };

    bot.send_message(msg.chat.id, "ok").await?;

    Ok(())
}

pub async fn start(bot: Bot, db: EventsDB) {
    Dispatcher::builder(
        bot,
        Update::filter_message().branch(
            dptree::entry()
                .filter_command::<Command>()
                .endpoint(handle_bot_commands),
        ),
    )
    .dependencies(dptree::deps![db])
    .enable_ctrlc_handler()
    .build()
    .dispatch()
    .await;
}

fn parse_args(input: String) -> Result<(String, String, Option<String>), ParseError> {
    let parts = input.split(",").collect::<Vec<&str>>();

    match parts.len() {
        2 => Ok((parts[0].to_string(), parts[1].to_string(), None)),
        3 => Ok((
            parts[0].to_string(),
            parts[1].to_string(),
            Some(parts[2].to_string()),
        )),
        len => Err(ParseError::Custom(
            format!("2 or 3 arguments expected, not {}", len).into(),
        )),
    }
}
