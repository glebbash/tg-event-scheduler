use crate::events_db::{Event, EventsDB};
use anyhow::Result;
use chrono::prelude::Local;
use chrono_english::Dialect;
use mongodb::bson::DateTime;
use teloxide::{
    prelude::*,
    utils::command::{BotCommands, ParseError},
};

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "subscribe current chat to specified channel")]
    Subscribe(String),
    #[command(description = "unsubscribe current chat from specified channel")]
    UnSubscribe(String),
    // /schedule 501, mon 1p, [1s]
    #[command(
        description = "schedule a message to be sent in 5 seconds",
        parse_with = accept_two_digits,
    )]
    Schedule(String, String, Option<String>),
}

async fn handle_bot_commands(bot: Bot, db: EventsDB, msg: Message, cmd: Command) -> Result<()> {
    match cmd {
        Command::Subscribe(channel) => {
            db.subscribe(msg.chat.id.0, channel).await?;
        }
        Command::UnSubscribe(channel) => {
            db.unsubscribe(msg.chat.id.0, channel).await?;
        }
        Command::Schedule(channel, notify_at_str, _interval_str) => {
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

            let notify_at =
                chrono_english::parse_date_string(&notify_at_str, Local::now(), Dialect::Uk);

            if notify_at.is_err() {
                bot.send_message(msg.chat.id, "Err: Invalid date").await?;
                return Ok(());
            }
            let notify_at = notify_at.unwrap();

            // TODO: handle intervals
            // let interval = chrono_english::parse_duration(&interval_str).unwrap();

            db.add_event(Event {
                id: msg.id.0,
                channel,
                message: event_message,
                notify_at: DateTime::from_millis(notify_at.timestamp_millis()),
                interval: None,
            })
            .await?;
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

fn accept_two_digits(input: String) -> Result<(String, String, Option<String>), ParseError> {
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
