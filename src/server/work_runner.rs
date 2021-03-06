use anyhow::{Context, Result};
use async_trait::async_trait;
use bb8_redis::{
    bb8::{Pool, PooledConnection},
    redis::AsyncCommands,
    RedisConnectionManager,
};
use workers::Work;

use crate::protocol::{Message, Queue};

use super::{HandlerInput, HandlerOutput, Module};

pub struct WorkRunner<D> {
    pool: Pool<RedisConnectionManager>,
    root: Module<D>,
    data: D,
}

impl<D> WorkRunner<D> {
    pub fn new(pool: Pool<RedisConnectionManager>, data: D, root: Module<D>) -> Self {
        WorkRunner {
            pool,
            data,
            root: root,
        }
    }

    #[inline]
    async fn get_connection(&self) -> Result<PooledConnection<'_, RedisConnectionManager>> {
        let conn = self
            .pool
            .get()
            .await
            .context("unable to retrieve a redis connection from the pool")?;

        Ok(conn)
    }

    async fn prepare(msg: &mut Message, result: Result<HandlerOutput>) {
        match result {
            Ok(result) => {
                msg.data = base64::encode(result.data);
                msg.error = None;
                msg.schema = result.schema;
            }
            Err(err) => {
                msg.error = Some(format!("{}", err));
                msg.data = String::default();
            }
        }

        let src = msg.source;
        if msg.destination.len() > 0 {
            msg.source = msg.destination[0];
        }
        msg.destination = vec![src];
    }

    async fn send(&self, msg: Message) -> Result<()> {
        let mut conn = self.get_connection().await?;
        conn.rpush(Queue::Reply.as_ref(), msg)
            .await
            .context("unable to send your reply message")?;

        Ok(())
    }
}

#[async_trait]
impl<D> Work for WorkRunner<D>
where
    D: Clone + Send + Sync + 'static,
{
    type Input = (String, Message);
    type Output = ();
    async fn run(&self, input: Self::Input) -> Self::Output {
        let (command, mut msg) = input;
        let data = base64::decode(&msg.data).unwrap(); // <- not safe
        let handler = self
            .root
            .lookup(command)
            .context("handler not found this should never happen")
            .unwrap();

        let state = self.data.clone();
        let out = handler
            .call(
                state,
                HandlerInput {
                    source: msg.source,
                    data: data,
                    schema: msg.schema.clone(),
                },
            )
            .await;

        Self::prepare(&mut msg, out).await;

        if let Err(err) = self.send(msg).await {
            log::debug!("{}", err);
        }
    }
}
