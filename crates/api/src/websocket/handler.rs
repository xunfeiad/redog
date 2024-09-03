use actix_ws::AggregatedMessage;
use error::RtcResult;
use std::{
    pin::{self, pin},
    time::{Duration, Instant},
};
use tokio::{select, sync::mpsc, time::interval};

use crate::websocket::server::{ChatServerHandle, ConnId};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn chat_ws(
    chat_server: ChatServerHandle,
    mut session: actix_ws::Session,
    msg_stream: actix_ws::MessageStream
) -> RtcResult<()>{
    log::info!("connected");
    let mut name = None;
    let mut last_heartbeat = Instant::now();
    let mut interval = interval(HEARTBEAT_INTERVAL);

    let (conn_tx, mut conn_rx) = mpsc::unbounded_channel();

    let conn_id = chat_server.connect(conn_tx).await?;
    let msg_stream = msg_stream.max_frame_size(128 * 1024)
    .aggregate_continuations().max_continuation_size(2*1024*1024);

    let mut msg_stream = pin!(msg_stream);
    let closed_reason = loop {
        let tick = pin!(interval.tick());
        let msg_tx
    };
    Ok(())
}
