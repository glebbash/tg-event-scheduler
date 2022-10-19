use dotenv::dotenv;
use teloxide::prelude::Bot;
use warp::Filter;

mod bot;
mod events_db;
mod notifier;

#[tokio::main]
async fn main() {
    dotenv().ok();
    pretty_env_logger::init();
    log::info!("Starting event scheduler bot...");

    let bot = Bot::from_env();
    let db = events_db::connect()
        .await
        .expect("Cannot connect to events db");
    let db_ = db.clone();
    let bot_ = bot.clone();

    let _ = tokio::join!(
        tokio::spawn(async move {
            bot::start(bot, db).await;
        }),
        tokio::spawn(async move {
            notifier::start(bot_, db_).await;
        }),
        tokio::spawn(async {
            warp::serve(warp::any().map(|| "OK"))
                .run(([0, 0, 0, 0], 8080))
                .await;
        }),
    );
}
