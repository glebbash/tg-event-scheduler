use anyhow::Result;
use futures::stream::StreamExt;
use futures::stream::TryStreamExt;
use mongodb::error::ErrorKind;
use mongodb::error::WriteFailure;
use mongodb::options::UpdateOptions;
use mongodb::Database;
use mongodb::{
    bson::doc,
    bson::DateTime,
    change_stream::{
        event::{ChangeStreamEvent, OperationType},
        ChangeStream,
    },
    Client, Collection,
};
use serde::{Deserialize, Serialize};
use std::env;

const MONGO_DUPLICATE_KEY_ERROR_CODE: i32 = 11000;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Subscription {
    #[serde(rename = "chatId")]
    pub chat_id: i64,
    pub channel: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Event {
    #[serde(rename = "_id")]
    pub id: i32,
    pub channel: String,
    pub message: String,
    #[serde(rename = "notifyAt")]
    pub notify_at: DateTime,
    pub interval: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EventTrigger {
    #[serde(rename = "_id")]
    pub id: i32,
    #[serde(rename = "notifyAt")]
    pub notify_at: DateTime,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ChatTimezone {
    #[serde(rename = "_id")]
    pub chat_id: i32,
    #[serde(rename = "timezone")]
    pub timezone: String,
}

#[derive(Clone)]
pub struct EventsDB {
    mongo_client: Client,
}

pub struct EventChangeListener {
    change_stream: ChangeStream<ChangeStreamEvent<EventTrigger>>,
}

pub enum EventChange {
    Created(i32),
    Triggered(i32),
    Unknown,
}

impl EventChangeListener {
    pub async fn next(&mut self) -> Option<EventChange> {
        self.change_stream
            .next()
            .await
            .transpose()
            .unwrap()
            .map(|e| {
                let event_id = e.document_key.unwrap().get_i32("_id").unwrap();

                match e.operation_type {
                    OperationType::Insert => EventChange::Created(event_id),
                    OperationType::Delete => EventChange::Triggered(event_id),
                    _ => EventChange::Unknown,
                }
            })
    }
}

impl EventsDB {
    pub async fn add_event(&self, event: Event) -> Result<()> {
        self.get_event_triggers()
            .insert_one(
                EventTrigger {
                    id: event.id,
                    notify_at: event.notify_at,
                },
                None,
            )
            .await?;

        self.get_events().insert_one(event, None).await?;

        Ok(())
    }

    pub async fn get_event(&self, id: i32) -> Result<Option<Event>> {
        let event = self.get_events().find_one(doc! { "_id": id }, None).await?;

        Ok(event)
    }

    pub async fn delete_event(&self, id: i32) -> Result<()> {
        self.get_events()
            .delete_one(doc! { "_id": id }, None)
            .await?;

        Ok(())
    }

    pub async fn listen_for_changes(&self) -> Result<EventChangeListener> {
        Ok(EventChangeListener {
            change_stream: self.get_event_triggers().watch(None, None).await?,
        })
    }

    pub async fn subscribe(&self, chat_id: i64, channel: String) -> Result<()> {
        let res = self
            .get_subscriptions()
            .insert_one(Subscription { chat_id, channel }, None)
            .await;

        if let Err(err) = res {
            match *err.kind {
                ErrorKind::Write(WriteFailure::WriteError(err))
                    if err.code == MONGO_DUPLICATE_KEY_ERROR_CODE =>
                {
                    return Ok(())
                }
                _ => return Err(err.into()),
            }
        }

        Ok(())
    }

    pub async fn unsubscribe(&self, chat_id: i64, channel: String) -> Result<()> {
        self.get_subscriptions()
            .delete_one(doc! { "chatId": chat_id, "channel": channel }, None)
            .await?;

        Ok(())
    }

    pub async fn get_subscribers(&self, channel: &String) -> Result<Vec<i64>> {
        let mut cursor = self
            .get_subscriptions()
            .find(doc! { "channel": channel }, None)
            .await?;

        let mut chat_ids: Vec<i64> = vec![];

        while let Some(subscription) = cursor.try_next().await? {
            chat_ids.push(subscription.chat_id);
        }

        Ok(chat_ids)
    }

    pub async fn set_chat_timezone(&self, chat_id: i64, timezone: String) -> Result<()> {
        self.get_chat_timezones()
            .update_one(
                doc! { "_id": chat_id },
                doc! {
                    "$set": { "timezone": timezone },
                    "$setOnInsert": { "_id": chat_id },
                },
                UpdateOptions::builder().upsert(true).build(),
            )
            .await?;

        Ok(())
    }

    pub async fn get_chat_timezone(&self, chat_id: i64) -> Result<Option<ChatTimezone>> {
        let res = self
            .get_chat_timezones()
            .find_one(doc! { "_id": chat_id }, None)
            .await?;

        Ok(res)
    }

    fn get_db(&self) -> Database {
        self.mongo_client.database("tg-event-scheduler")
    }

    fn get_subscriptions(&self) -> Collection<Subscription> {
        self.get_db().collection("subscriptions")
    }

    fn get_events(&self) -> Collection<Event> {
        self.get_db().collection("event-info")
    }

    fn get_event_triggers(&self) -> Collection<EventTrigger> {
        self.get_db().collection("events")
    }

    fn get_chat_timezones(&self) -> Collection<ChatTimezone> {
        self.get_db().collection("chat-timezones")
    }
}

pub async fn connect() -> Result<EventsDB> {
    Ok(EventsDB {
        mongo_client: Client::with_uri_str(env::var("MONGO_URL")?).await?,
    })
}
