use argon2::{
    password_hash::{PasswordHash, SaltString},
    Argon2, PasswordHasher, PasswordVerifier,
};

use std::{
    collections::{HashMap, HashSet},
    io::ErrorKind,
    sync::Arc,
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{broadcast, Mutex},
};

use crate::protocol::{
    read_message, write_message, ChatMessage, CreateChatResponse, ErrorCode, ErrorResponse,
    JoinChatResponse, LeaveChatResponse, Packet,
    ProtocolMessage::{self, *},
    SendMessageResponse,
};

use rand::rngs::OsRng;
use uuid::Uuid;

fn gen_chat_id() -> Uuid {
    Uuid::new_v4()
}

fn hash_password(password: String) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let salt = SaltString::try_from_rng(&mut OsRng)?;
    let hash = Argon2::default().hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}

fn verify_password(
    password: &str,
    stored: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let parsed = PasswordHash::new(stored)?;
    Argon2::default().verify_password(password.as_bytes(), &parsed)?;
    Ok(())
}

struct ChatRoom {
    tokens: HashMap<Uuid, String>, // token to username
    users: HashSet<String>,
    password: Option<String>,
    messages: Vec<ChatMessage>,
    broadcaster: broadcast::Sender<ChatMessage>,
}

impl ChatRoom {
    fn new(password: Option<String>) -> Self {
        let (broadcaster, _) = broadcast::channel(100);

        ChatRoom {
            tokens: HashMap::new(),
            users: HashSet::new(),
            password,
            messages: Vec::new(),
            broadcaster,
        }
    }

    fn join(
        &mut self,
        username: String,
        password: Option<String>,
    ) -> Result<(Uuid, broadcast::Receiver<ChatMessage>), ErrorResponse> {
        if self.users.contains(&username) {
            return Err(ErrorResponse {
                code: ErrorCode::UserAlreadyInRoom,
                message: "User already in room!".into(),
            });
        }

        if let Some(room_pw_hash) = &self.password {
            let pw = password.ok_or_else(|| ErrorResponse {
                code: ErrorCode::PasswordMissing,
                message: "Password missing".into(),
            })?;

            verify_password(&pw, room_pw_hash).map_err(|_| ErrorResponse {
                code: ErrorCode::WrongPassword,
                message: "Wrong password".into(),
            })?;
        }

        let token = Uuid::new_v4();
        self.tokens.insert(token, username.clone());
        self.users.insert(username);
        let receiver = self.broadcaster.subscribe();

        Ok((token, receiver))
    }

    fn add_message(&mut self, token: Uuid, message: String) -> Result<(), ErrorResponse> {
        let username = self.tokens.get(&token).ok_or_else(|| ErrorResponse {
            code: ErrorCode::Unauthorized,
            message: "User does not exist in the room".into(),
        })?;
        self.messages.push(ChatMessage {
            username: username.into(),
            message: message.clone(),
        });

        let _ = self.broadcaster.send(ChatMessage {
            username: username.clone(),
            message,
        });

        Ok(())
    }

    fn leave(&mut self, token: Uuid) -> Result<(), ErrorResponse> {
        let username = self.tokens.get(&token).ok_or_else(|| ErrorResponse {
            code: ErrorCode::Unauthorized,
            message: "User does not exist in the room".into(),
        })?;
        self.users.remove(username);
        self.tokens.remove(&token);
        Ok(())
    }
}

pub struct ChatServer {
    port: i32,
    chats: HashMap<Uuid, ChatRoom>, // ChatId to Chat
}

impl ChatServer {
    pub fn new(port: i32) -> Self {
        ChatServer {
            port,
            chats: HashMap::new(),
        }
    }

    pub async fn run(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let addr = format!("0.0.0.0:{}", self.port);

        let listener = TcpListener::bind(addr).await?;
        let state = Arc::new(Mutex::new(self));

        loop {
            let (socket, _) = listener.accept().await?;
            let copy = Arc::clone(&state);

            tokio::spawn(async move {
                if let Err(e) = handle_connection(socket, copy).await {
                    eprintln!("an error occured:  {:?}", e);
                }
            });
        }
    }
}

async fn handle_connection(
    mut socket: tokio::net::TcpStream,
    state: Arc<Mutex<ChatServer>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut message_receiver: Option<broadcast::Receiver<ChatMessage>> = None;
    let mut current_chat_id: Option<Uuid> = None;

    loop {
        tokio::select! {
            result = read_message(&mut socket) => {
                let packet = match result {
                    Ok(pkt) => pkt,
                    Err(_) => return Ok(()), // need to use OK instead of break
                };

                let mut server = state.lock().await;
                match packet.message {
                    CreateChatRequest(r) => {
                        let chat_id = gen_chat_id();
                        let hashed_pw = match r.password {
                            Some(pw) => Some(hash_password(pw)?),
                            None => None,
                        };
                        server.chats.insert(chat_id, ChatRoom::new(hashed_pw));
                        send_response(
                            &mut socket,
                            CreateChatResponse(CreateChatResponse { chat_id }),
                        )
                        .await?;
                    }
                    JoinChatRequest(r) => {
                        if current_chat_id.is_some() {
                            send_error(&mut socket, ErrorCode::UserAlreadyInAnotherRoom, "User already in another room!").await?;
                            continue;
                        }

                        let Some(chat) = server.chats.get_mut(&r.chat_id) else {
                            send_error(&mut socket, ErrorCode::ChatNotFound, "Chat not found").await?;
                            continue;
                        };

                        match chat.join(r.username, r.password) {
                            Ok((token, receiver)) => {
                                message_receiver = Some(receiver);
                                current_chat_id = Some(r.chat_id);
                                send_response(&mut socket, JoinChatResponse(JoinChatResponse { token }))
                                    .await?;
                            }
                            Err(err) => {
                                send_error(&mut socket, err.code, &err.message).await?;
                            }
                        }
                    }
                    SendMessageRequest(r) => {
                        let Some(chat) = server.chats.get_mut(&r.chat_id) else {
                            send_error(&mut socket, ErrorCode::ChatNotFound, "Chat not found").await?;
                            continue;
                        };
                        match chat.add_message(r.token, r.message) {
                            Ok(()) => {
                                send_response(&mut socket, SendMessageResponse(SendMessageResponse {}))
                                    .await?;
                            }
                            Err(err) => {
                                send_error(&mut socket, err.code, &err.message).await?;
                            }
                        }
                    }
                    LeaveChatRequest(r) => {
                        let Some(chat) = server.chats.get_mut(&r.chat_id) else {
                            send_error(&mut socket, ErrorCode::ChatNotFound, "Chat not found").await?;
                            continue;
                        };
                        match chat.leave(r.token) {
                            Ok(()) => {
                                message_receiver = None; // clear receiver when leaving?
                                send_response(&mut socket, LeaveChatResponse(LeaveChatResponse {})).await?;
                            }
                            Err(err) => {
                                send_error(&mut socket, err.code, &err.message).await?;
                            }
                        }
                    }
                    _ => {
                        return Err(Box::new(std::io::Error::new(
                            ErrorKind::InvalidData,
                            "Unexpected request",
                        )));
                    }
                }
            }

            msg = async {
                if let Some(ref mut receiver) = message_receiver {
                    receiver.recv().await
                } else {
                    std::future::pending().await
                }
            } => {
                match msg {
                    Ok(broadcast_msg) => {
                        send_response(&mut socket, MessageBroadcast(broadcast_msg)).await?;
                    }
                    Err(_) => {} // TODO: do I need to do anything here?
                }
            }
        }
    }
}

async fn send_error(
    sock: &mut TcpStream,
    code: ErrorCode,
    msg: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let pkt = Packet {
        version: 1,
        message: ErrorResponse(ErrorResponse {
            code,
            message: msg.into(),
        }),
    };
    write_message(sock, &pkt).await?;
    Ok(())
}

async fn send_response(
    sock: &mut TcpStream,
    m: ProtocolMessage,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let pkt = Packet {
        version: 1,
        message: m,
    };
    write_message(sock, &pkt).await?;
    Ok(())
}
