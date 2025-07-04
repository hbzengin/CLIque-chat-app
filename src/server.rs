use argon2::{
    password_hash::{PasswordHash, SaltString},
    Argon2, PasswordHasher, PasswordVerifier,
};

use std::{collections::HashMap, io::ErrorKind, sync::Arc};
use tokio::{net::TcpListener, sync::Mutex};

use crate::protocol::{read_message, write_message, CreateChatResponse, Message::*, Packet};

use rand::rngs::OsRng;

use uuid::Uuid;

fn gen_chat_id() -> Uuid {
    Uuid::new_v4()
}

fn hash_password(password: String) -> Result<String, Box<dyn std::error::Error>> {
    let salt = SaltString::try_from_rng(&mut OsRng)?;
    let hash = Argon2::default().hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}

fn verify_password(password: &str, stored: &str) -> Result<(), Box<dyn std::error::Error>> {
    let parsed = PasswordHash::new(stored)?;
    Argon2::default().verify_password(password.as_bytes(), &parsed)?;
    Ok(())
}

struct ChatRoom {
    users: Vec<String>,
    password: Option<String>,
    messages: Vec<String>,
}

pub struct ChatServer {
    port: i32,
    tokens: HashMap<String, String>, // token to username
    chats: HashMap<Uuid, ChatRoom>,  // ChatId to Chat
}

impl ChatServer {
    pub fn new(port: i32) -> Self {
        ChatServer {
            port,
            tokens: HashMap::new(),
            chats: HashMap::new(),
        }
    }

    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
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
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        let Packet {
            version: _,
            message,
        } = match read_message(&mut socket).await {
            Err(e) => return Err(e),
            Ok(pkt) => pkt,
        };

        let response = {
            let mut server = state.lock().await;

            match message {
                CreateChatRequest(r) => {
                    let chat_id = gen_chat_id();
                    let hashed_pw = match r.password {
                        Some(pw) => Some(hash_password(pw)?),
                        None => None,
                    };

                    server.chats.insert(
                        chat_id,
                        ChatRoom {
                            users: Vec::new(),
                            password: hashed_pw,
                            messages: Vec::new(),
                        },
                    );
                    CreateChatResponse(CreateChatResponse { chat_id })
                }
                JoinChatRequest(r) => {
                    // if !server.chats.contains_key(&r.chat_id) {}
                }
                SendMessageRequest(r) => todo!(),
                LeaveChatRequest(r) => todo!(),
                other => {
                    return Err(Box::new(std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("Unexpected request: {:?}", other),
                    )))
                }
            }
        };

        let reply = Packet {
            version: 1,
            message: response,
        };
        write_message(&mut socket, &reply).await?
    }
}
