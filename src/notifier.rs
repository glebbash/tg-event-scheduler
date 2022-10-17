use crate::events_db::{EventChange, EventsDB};

use teloxide::{prelude::Requester, types::ChatId, Bot};

pub async fn start(bot: Bot, db: EventsDB) {
    let mut changes = db.listen_for_changes().await.unwrap();

    while let Some(change) = changes.next().await {
        if let EventChange::Triggered(event_id) = change {
            let event = db.get_event(event_id).await.unwrap();
            if event.is_none() {
                continue; // triggered event does not exist, skipping
            }
            let event = event.unwrap();

            log::info!("Event triggered: {:?}", event);

            let chat_ids = db.get_subscribers(event.channel).await.unwrap();

            // TODO: send in parralel?
            for chat_id in chat_ids {
                bot.send_message(ChatId(chat_id), &event.message)
                    .await
                    .unwrap();
            }

            // TODO: handle repeating events
            db.delete_event(event_id).await.unwrap();
        }
    }
}
