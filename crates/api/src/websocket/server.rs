
use anyhow::Context;
use error::RtcResult;
use rand::{thread_rng, Rng};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::sync::{
    mpsc::{self, UnboundedReceiver, UnboundedSender},
    oneshot,
};
pub type ConnId = usize;

pub type RoomId = usize;

pub type Msg = String;

#[derive(Debug)]
pub enum Command {
    Connect {
        conn_tx: mpsc::UnboundedSender<Msg>,
        res_tx: oneshot::Sender<ConnId>,
    },
    Disconnect {
        conn: ConnId,
    },
    List {
        res_tx: oneshot::Sender<Vec<RoomId>>,
    },
    Join {
        conn: ConnId,
        room: RoomId,
        res_tx: oneshot::Sender<()>,
    },
    Message {
        msg: Msg,
        conn: ConnId,
        res_tx: oneshot::Sender<()>,
    },
}

#[derive(Debug)]
pub struct ChatServerHandle {
    cmd_tx: UnboundedSender<Command>,
}

impl ChatServerHandle {
    /// Register client message sender and obtain connection ID.
    pub async fn connect(&self, conn_tx: UnboundedSender<Msg>) -> RtcResult<ConnId> {
        let (res_tx, res_rx) = oneshot::channel();
        self.cmd_tx
            .send(Command::Connect { conn_tx, res_tx })
            .context("Send connection signal failed.")?;
        Ok(res_rx.await?)
    }

    ///  List all created rooms.
    pub async fn list_rooms(&self) -> RtcResult<Vec<RoomId>> {
        let (res_tx, res_rx) = oneshot::channel();
        self.cmd_tx
            .send(Command::List { res_tx })
            .context("Send list command signal failed.")?;
        Ok(res_rx.await?)
    }

    pub async fn join_room(&self, conn: ConnId, room: impl Into<RoomId>) {
        let (res_tx, res_rx) = oneshot::channel();
        self.cmd_tx
            .send(Command::Join {
                conn,
                room: room.into(),
                res_tx,
            })
            .unwrap();
        res_rx.await.unwrap()
    }

    pub async fn send_message(&self, conn: ConnId, msg: impl Into<Msg>) -> RtcResult<()> {
        let (res_tx, res_rx) = oneshot::channel();
        self.cmd_tx
            .send(Command::Message {
                msg: msg.into(),
                conn,
                res_tx,
            })
            .context("Send message singal failed.")?;
        res_rx.await?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct ChatServer {
    sessions: HashMap<ConnId, UnboundedSender<Msg>>,
    rooms: HashMap<RoomId, HashSet<ConnId>>,
    visitor_count: Arc<AtomicUsize>,
    cmd_rx: UnboundedReceiver<Command>,
}

impl ChatServer {
    pub fn new() -> (Self, ChatServerHandle) {
        let mut rooms = HashMap::with_capacity(4);
        rooms.insert(0, HashSet::new());
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        (
            Self {
                sessions: HashMap::new(),
                rooms,
                visitor_count: Arc::new(AtomicUsize::new(0)),
                cmd_rx,
            },
            ChatServerHandle { cmd_tx },
        )
    }

    async fn send_system_message(
        &self,
        room_id: RoomId,
        skip: ConnId,
        msg: impl Into<Msg>,
    ) -> RtcResult<()> {
        if let Some(conn_ids) = self.rooms.get(&room_id) {
            let msg = msg.into();
            for conn_id in conn_ids {
                if *conn_id != skip {
                    if let Some(tx) = self.sessions.get(conn_id) {
                        let _ = tx.send(msg.clone());
                    }
                }
            }
        }
        Ok(())
    }

    async fn send_message(&self, conn: ConnId, msg: impl Into<Msg>) -> RtcResult<()> {
        if let Some(room_id) = self
            .rooms
            .iter()
            .find_map(|(room_id, participants)| participants.contains(&conn).then_some(room_id))
        {
            self.send_system_message(*room_id, conn, msg).await?;
        };
        Ok(())
    }

    async fn connect(&mut self, tx: mpsc::UnboundedSender<Msg>) -> RtcResult<ConnId> {
        self.send_system_message(0, 0, "Someone joined").await?;

        let id = thread_rng().gen::<ConnId>();
        self.sessions.insert(id, tx);
        self.rooms.entry(0).or_default().insert(id);
        let count = self.visitor_count.fetch_add(1, Ordering::SeqCst);
        self.send_system_message(0, 0, format!("Total visitors {count}"))
            .await?;

        Ok(id)
    }

    async fn disconnect(&mut self, conn_id: ConnId) -> RtcResult<()> {
        println!("Someone disconnected");
        let mut rooms: Vec<RoomId> = Vec::new();

        if self.sessions.remove(&conn_id).is_some() {
            for (room_id, conn_ids) in &mut self.rooms {
                if conn_ids.remove(room_id) {
                    rooms.push(*room_id);
                }
            }
        }

        for room in rooms {
            self.send_system_message(room, 0, "Someone disconnected")
                .await?;
        }
        Ok(())
    }

    fn list_rooms(&mut self) -> Vec<RoomId> {
        self.rooms.keys().cloned().collect()
    }

    async fn join_room(&mut self, conn_id: ConnId, room: RoomId) -> RtcResult<()> {
        let mut rooms = Vec::new();
        for (room_id, conn_ids) in &mut self.rooms {
            if conn_ids.remove(room_id) {
                rooms.push(*room_id)
            }
        }

        for room in rooms {
            self.send_system_message(room, conn_id, "Someone disconnected")
                .await?;
        }
        self.rooms.entry(room).or_default().insert(conn_id);
        self.send_system_message(room, conn_id, "Someone connected")
            .await?;
        Ok(())
    }

    pub async fn run(mut self) -> RtcResult<()> {
        while let Some(cmd) = self.cmd_rx.recv().await {
            match cmd {
                Command::Connect { conn_tx, res_tx } => {
                    let conn_id = self.connect(conn_tx).await?;
                    let _ = res_tx.send(conn_id);
                }
                Command::Disconnect { conn } => {
                    self.disconnect(conn).await?;
                }
                Command::List { res_tx } => {
                    let _ = res_tx.send(self.list_rooms());
                }
                Command::Join { conn, room, res_tx } => {
                    self.join_room(conn, room).await?;
                    let _ = res_tx.send(());
                }
                Command::Message { msg, conn, res_tx } => {
                    self.send_message(conn, msg).await?;
                    let _ = res_tx.send(());
                }
            }
        }

        Ok(())
    }
}