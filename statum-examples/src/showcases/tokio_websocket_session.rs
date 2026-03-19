use statum::{machine, state, transition};
use tokio::{sync::mpsc, task::JoinHandle};

#[state]
pub enum SessionState {
    Connected,
    Authenticated(SessionInfo),
    Subscribed(Subscription),
    Closed(CloseReason),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionInfo {
    pub user_id: String,
}

impl TryFrom<&str> for SessionInfo {
    type Error = &'static str;

    fn try_from(token: &str) -> Result<Self, Self::Error> {
        token
            .strip_prefix("token:")
            .filter(|user_id| !user_id.trim().is_empty())
            .map(|user_id| Self {
                user_id: user_id.to_owned(),
            })
            .ok_or("token must use token:<user>")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subscription {
    pub user_id: String,
    pub topic: String,
}

impl Subscription {
    fn new(user_id: String, topic: String) -> Result<Self, &'static str> {
        if topic.trim().is_empty() {
            return Err("topic is required");
        }

        Ok(Self { user_id, topic })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloseReason {
    pub reason: String,
}

#[machine]
pub struct SessionMachine<SessionState> {
    pub connection_id: u64,
    pub peer_label: String,
}

#[transition]
impl SessionMachine<Connected> {
    fn authenticate(self, session: SessionInfo) -> SessionMachine<Authenticated> {
        self.transition_with(session)
    }

    fn close(self, reason: String) -> SessionMachine<Closed> {
        self.transition_with(CloseReason { reason })
    }
}

#[transition]
impl SessionMachine<Authenticated> {
    fn subscribe(self, subscription: Subscription) -> SessionMachine<Subscribed> {
        self.transition_with(subscription)
    }

    fn close(self, reason: String) -> SessionMachine<Closed> {
        self.transition_with(CloseReason { reason })
    }
}

#[transition]
impl SessionMachine<Subscribed> {
    fn close(self, reason: String) -> SessionMachine<Closed> {
        self.transition_with(CloseReason { reason })
    }
}

impl SessionMachine<Subscribed> {
    fn publish(&self, topic: &str, body: &str) -> Result<ServerFrame, &'static str> {
        if topic.trim().is_empty() {
            return Err("topic is required");
        }
        if body.trim().is_empty() {
            return Err("body is required");
        }

        if topic != self.state_data.topic {
            return Err("publish topic does not match subscription");
        }

        Ok(ServerFrame::Delivered {
            user_id: self.state_data.user_id.clone(),
            topic: topic.to_string(),
            body: body.to_string(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientFrame {
    Authenticate { token: String },
    Subscribe { topic: String },
    Publish { topic: String, body: String },
    Close { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServerFrame {
    Hello {
        connection_id: u64,
        peer_label: String,
    },
    Authenticated {
        user_id: String,
    },
    Subscribed {
        topic: String,
    },
    Delivered {
        user_id: String,
        topic: String,
        body: String,
    },
    Error {
        message: String,
    },
    Bye {
        reason: String,
    },
}

impl ServerFrame {
    fn error(message: &'static str) -> Self {
        Self::Error {
            message: message.to_string(),
        }
    }
}

pub struct SessionHandle {
    client: mpsc::Sender<ClientFrame>,
    server: mpsc::Receiver<ServerFrame>,
    task: JoinHandle<Result<(), SessionError>>,
}

#[derive(Debug)]
pub enum SessionError {
    ChannelClosed,
    ClientDisconnected,
    TaskJoin(String),
}

impl core::fmt::Display for SessionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ChannelClosed => write!(f, "session channel closed"),
            Self::ClientDisconnected => write!(f, "client disconnected before closing the session"),
            Self::TaskJoin(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for SessionError {}

impl<T> From<mpsc::error::SendError<T>> for SessionError {
    fn from(_: mpsc::error::SendError<T>) -> Self {
        Self::ChannelClosed
    }
}

impl From<tokio::task::JoinError> for SessionError {
    fn from(error: tokio::task::JoinError) -> Self {
        Self::TaskJoin(error.to_string())
    }
}

impl SessionHandle {
    pub async fn send(&self, frame: ClientFrame) -> Result<(), SessionError> {
        self.client.send(frame).await?;
        Ok(())
    }

    pub async fn recv(&mut self) -> Option<ServerFrame> {
        self.server.recv().await
    }

    pub async fn finish(self) -> Result<(), SessionError> {
        drop(self.client);
        self.task.await.map_err(SessionError::from)?
    }
}

pub fn spawn_session(connection_id: u64, peer_label: impl Into<String>) -> SessionHandle {
    let (client_tx, client_rx) = mpsc::channel(8);
    let (server_tx, server_rx) = mpsc::channel(8);
    let peer_label = peer_label.into();

    let task = tokio::spawn(run_session(
        connection_id,
        peer_label.clone(),
        client_rx,
        server_tx,
    ));

    SessionHandle {
        client: client_tx,
        server: server_rx,
        task,
    }
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut session = spawn_session(7, "127.0.0.1:4000");

    if let Some(frame) = session.recv().await {
        println!("{frame:?}");
    }

    session
        .send(ClientFrame::Authenticate {
            token: "token:alice".to_string(),
        })
        .await?;
    if let Some(frame) = session.recv().await {
        println!("{frame:?}");
    }

    session
        .send(ClientFrame::Subscribe {
            topic: "deployments".to_string(),
        })
        .await?;
    if let Some(frame) = session.recv().await {
        println!("{frame:?}");
    }

    session
        .send(ClientFrame::Publish {
            topic: "deployments".to_string(),
            body: "rollout started".to_string(),
        })
        .await?;
    if let Some(frame) = session.recv().await {
        println!("{frame:?}");
    }

    session
        .send(ClientFrame::Close {
            reason: "demo complete".to_string(),
        })
        .await?;
    if let Some(frame) = session.recv().await {
        println!("{frame:?}");
    }

    session.finish().await?;
    Ok(())
}

async fn run_session(
    connection_id: u64,
    peer_label: String,
    mut client_rx: mpsc::Receiver<ClientFrame>,
    server_tx: mpsc::Sender<ServerFrame>,
) -> Result<(), SessionError> {
    send_server(
        &server_tx,
        ServerFrame::Hello {
            connection_id,
            peer_label: peer_label.clone(),
        },
    )
    .await?;

    let mut state = session_machine::SomeState::Connected(
        SessionMachine::<Connected>::builder()
            .connection_id(connection_id)
            .peer_label(peer_label)
            .build(),
    );

    loop {
        state = match state {
            session_machine::SomeState::Connected(machine) => {
                match recv_client(&mut client_rx).await? {
                    ClientFrame::Authenticate { token } => {
                        match SessionInfo::try_from(token.as_str()) {
                            Ok(session) => {
                                let user_id = session.user_id.clone();
                                let next = machine.authenticate(session);
                                send_server(&server_tx, ServerFrame::Authenticated { user_id })
                                    .await?;
                                session_machine::SomeState::Authenticated(next)
                            }
                            Err(message) => {
                                send_server(&server_tx, ServerFrame::error(message)).await?;
                                session_machine::SomeState::Connected(machine)
                            }
                        }
                    }
                    ClientFrame::Subscribe { .. } => {
                        send_server(
                            &server_tx,
                            ServerFrame::error("authenticate before subscribing"),
                        )
                        .await?;
                        session_machine::SomeState::Connected(machine)
                    }
                    ClientFrame::Publish { .. } => {
                        send_server(
                            &server_tx,
                            ServerFrame::error("authenticate before publishing"),
                        )
                        .await?;
                        session_machine::SomeState::Connected(machine)
                    }
                    ClientFrame::Close { reason } => {
                        let _closed = machine.close(reason.clone());
                        send_server(&server_tx, ServerFrame::Bye { reason }).await?;
                        return Ok(());
                    }
                }
            }
            session_machine::SomeState::Authenticated(machine) => {
                match recv_client(&mut client_rx).await? {
                    ClientFrame::Authenticate { .. } => {
                        send_server(
                            &server_tx,
                            ServerFrame::error("session already authenticated"),
                        )
                        .await?;
                        session_machine::SomeState::Authenticated(machine)
                    }
                    ClientFrame::Subscribe { topic } => {
                        match Subscription::new(machine.state_data.user_id.clone(), topic) {
                            Ok(subscription) => {
                                let topic = subscription.topic.clone();
                                let next = machine.subscribe(subscription);
                                send_server(&server_tx, ServerFrame::Subscribed { topic }).await?;
                                session_machine::SomeState::Subscribed(next)
                            }
                            Err(message) => {
                                send_server(&server_tx, ServerFrame::error(message)).await?;
                                session_machine::SomeState::Authenticated(machine)
                            }
                        }
                    }
                    ClientFrame::Publish { .. } => {
                        send_server(
                            &server_tx,
                            ServerFrame::error("subscribe before publishing"),
                        )
                        .await?;
                        session_machine::SomeState::Authenticated(machine)
                    }
                    ClientFrame::Close { reason } => {
                        let _closed = machine.close(reason.clone());
                        send_server(&server_tx, ServerFrame::Bye { reason }).await?;
                        return Ok(());
                    }
                }
            }
            session_machine::SomeState::Subscribed(machine) => {
                match recv_client(&mut client_rx).await? {
                    ClientFrame::Authenticate { .. } => {
                        send_server(
                            &server_tx,
                            ServerFrame::error("session already authenticated"),
                        )
                        .await?;
                        session_machine::SomeState::Subscribed(machine)
                    }
                    ClientFrame::Subscribe { .. } => {
                        send_server(&server_tx, ServerFrame::error("session already subscribed"))
                            .await?;
                        session_machine::SomeState::Subscribed(machine)
                    }
                    ClientFrame::Publish { topic, body } => match machine.publish(&topic, &body) {
                        Ok(frame) => {
                            send_server(&server_tx, frame).await?;
                            session_machine::SomeState::Subscribed(machine)
                        }
                        Err(message) => {
                            send_server(&server_tx, ServerFrame::error(message)).await?;
                            session_machine::SomeState::Subscribed(machine)
                        }
                    },
                    ClientFrame::Close { reason } => {
                        let _closed = machine.close(reason.clone());
                        send_server(&server_tx, ServerFrame::Bye { reason }).await?;
                        return Ok(());
                    }
                }
            }
            session_machine::SomeState::Closed(_) => return Ok(()),
        };
    }
}

async fn recv_client(
    client_rx: &mut mpsc::Receiver<ClientFrame>,
) -> Result<ClientFrame, SessionError> {
    client_rx
        .recv()
        .await
        .ok_or(SessionError::ClientDisconnected)
}

async fn send_server(
    server_tx: &mpsc::Sender<ServerFrame>,
    frame: ServerFrame,
) -> Result<(), SessionError> {
    server_tx.send(frame).await?;
    Ok(())
}
