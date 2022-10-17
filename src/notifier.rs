use crate::events_db::{Event, EventChange, EventsDB};

use chrono::Duration;
use mongodb::bson::DateTime;
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

            let chat_ids = db.get_subscribers(&event.channel).await.unwrap();

            // TODO: send in parralel?
            for chat_id in chat_ids {
                bot.send_message(ChatId(chat_id), &event.message)
                    .await
                    .unwrap();
            }

            db.delete_event(event_id).await.unwrap();

            if let Some(interval) = &event.interval {
                let duration = parse_duration::parse(&interval).unwrap();
                let notify_at = event.notify_at.to_chrono() + Duration::from_std(duration).unwrap();

                db.add_event(Event {
                    id: event.id,
                    channel: event.channel.clone(),
                    message: event.message,
                    notify_at: DateTime::from_chrono(notify_at),
                    interval: event.interval,
                })
                .await
                .unwrap();
            }
        }
    }
}
