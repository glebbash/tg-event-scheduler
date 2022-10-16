use anyhow::Result;
use dotenv::dotenv;
use mongodb::{
    bson::doc,
    options::{ClientOptions, UpdateModifications, UpdateOptions},
    Client,
};
use serde::{Deserialize, Serialize};
use std::env;
use teloxide::{prelude::*, utils::command::BotCommands};
use warp::Filter;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Item {
    id: u32,
    message: String,
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    pretty_env_logger::init();
    log::info!("Starting command bot...");

    let mongo = mongo_connect().await.expect("Cannot connect to mongodb");

    let _ = tokio::join!(
        tokio::spawn(async move {
            Dispatcher::builder(
                Bot::from_env(),
                Update::filter_message()
                    .branch(dptree::entry().filter_command::<Command>().endpoint(answer)),
            )
            .dependencies(dptree::deps![mongo])
            .enable_ctrlc_handler()
            .build()
            .dispatch()
            .await;
        }),
        tokio::spawn(async move {
            warp::serve(warp::any().map(|| "OK"))
                .run(([0, 0, 0, 0], 8080))
                .await;
        })
    );
}

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "read value from db")]
    Read,
    #[command(description = "write value to db")]
    Write(String),
}

async fn answer(bot: Bot, mongo: Client, msg: Message, cmd: Command) -> ResponseResult<()> {
    let items = mongo
        .database("tg-event-scheduler")
        .collection::<Item>("test");

    match cmd {
        Command::Read => {
            let res = items
                .find_one(doc! { "id": 1 }, None)
                .await
                .unwrap()
                .unwrap();

            bot.send_message(msg.chat.id, format!("Value is: {}", res.message))
                .await?
        }
        Command::Write(username) => {
            items
                .update_one(
                    doc! { "id": 1 },
                    UpdateModifications::Document(doc! {
                        "$set": {
                            "id": 1, "message": username
                        }
                    }),
                    UpdateOptions::builder().upsert(true).build(),
                )
                .await
                .unwrap();

            bot.send_message(msg.chat.id, "written").await?
        }
    };

    Ok(())
}

async fn mongo_connect() -> Result<Client> {
    let username = env::var("MONGO_USER")?;
    let password = env::var("MONGO_PASS")?;

    let client_options = ClientOptions::parse(format!(
        "mongodb+srv://{username}:{password}@cluster0.xlyaogx.mongodb.net/?retryWrites=true&w=majority"
    ))
    .await?;

    let client = Client::with_options(client_options)?;

    Ok(client)
}
