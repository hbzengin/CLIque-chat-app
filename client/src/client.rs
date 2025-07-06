// client.rs

use colored::Colorize;
use protocol::{read_message, write_message, CreateChatRequest, JoinChatRequest, LeaveChatRequest, Packet, ProtocolMessage, SendMessageRequest};
use std::{error::Error, str::FromStr, sync::Arc};
use tokio::{
    io::{split, AsyncBufReadExt},
    net::TcpStream,
    sync::{mpsc, Mutex},
};

use uuid::Uuid;

macro_rules! y_println {
    ($($arg:tt)*) => {
        println!("{}", format!($($arg)*).yellow());
    };
}

macro_rules! debug_r_eprintln {
    ($($arg:tt)*) => {
        if cfg!(debug_assertions) {
            eprintln!("{}", format!($($arg)*).red());
        }
    };
}

macro_rules! debug_println {
    ($($arg:tt)*) => {
        if cfg!(debug_assertions) {
            println!("{}", format!($($arg)*).blue());
        }
    };
}

const HELP_TEXT: &str = r#"
Commands:
/create [password]           — create a new chat (optional arg password)
/join <chat_id> <user> [pw]  — join existing chat
/send <message>              — send to current chat
/leave                       — leave current chat
/exit                        — exit
"#;

pub enum Command {
    Create(Option<String>),
    Join { chat_id: Uuid, username: String, password: Option<String> },
    Send(String),
    Leave,
    Exit,
    Help,
    Invalid,
}

impl FromStr for Command {
    type Err = ();

    fn from_str(line: &str) -> Result<Self, Self::Err> {
        let mut iter = line.trim().split_whitespace();
        match iter.next() {
            Some("/create") => {
                let pw = iter.next().map(str::to_owned);
                Ok(Command::Create(pw))
            }
            Some("/join") => {
                if let (Some(chat_id), Some(username)) = (iter.next(), iter.next()) {
                    match Uuid::parse_str(chat_id) {
                        Ok(chat_id) => {
                            let password = iter.next().map(str::to_owned);
                            Ok(Command::Join { chat_id, username: username.into(), password })
                        }
                        Err(_) => Err(()),
                    }
                } else {
                    Err(())
                }
            }
            Some("/send") => {
                let msg = iter.collect::<Vec<_>>().join(" ");
                if msg.is_empty() {
                    Err(())
                } else {
                    Ok(Command::Send(msg))
                }
            }
            Some("/leave") => Ok(Command::Leave),
            Some("/exit") => Ok(Command::Exit),
            Some("/help") => Ok(Command::Help),
            _ => Err(()),
        }
    }
}

pub struct ChatClient {
    send_chan: mpsc::UnboundedSender<Packet>,
    chat_state: Arc<Mutex<Option<(Uuid, Uuid, String)>>>, // (chat_id, token, username)
}

impl ChatClient {
    pub async fn new(host: String, port: String) -> Result<Self, Box<dyn Error>> {
        let addr = format!("{}:{}", host, port);
        let stream = TcpStream::connect(&addr).await?;

        y_println!("Client connected to {}!", addr);

        let (read_stream, write_stream) = split(stream);

        let (send_chan, mut recv_chan) = mpsc::unbounded_channel::<Packet>();
        let chat_state = Arc::new(Mutex::new(None::<(Uuid, Uuid, String)>));
        // writer task that communicates with the server
        tokio::spawn(async move {
            let mut writer = write_stream;

            while let Some(pkt) = recv_chan.recv().await {
                if let Err(e) = write_message(&mut writer, &pkt).await {
                    debug_r_eprintln!("Write failed: {}", e);
                    break;
                }
            }
        });

        // reader task that gets server responses and prints new messages
        let reader_copy = chat_state.clone();
        tokio::spawn(async move {
            let mut reader = read_stream;
            let chat_state = reader_copy;

            loop {
                match read_message(&mut reader).await {
                    Ok(Packet { message, .. }) => match message {
                        ProtocolMessage::MessageBroadcast(chat) => {
                            if let Some((_, _, ref my_username)) = *chat_state.lock().await {
                                if &chat.username == my_username {
                                    continue;
                                }
                            }
                            println!("{}: {}", chat.username.blue(), chat.message);
                        }
                        ProtocolMessage::CreateChatResponse(resp) => {
                            y_println!("Created new chat with chat_id = {}", resp.chat_id);
                        }
                        ProtocolMessage::JoinChatResponse(resp) => {
                            y_println!("Joined chat");
                            debug_println!("token = {}", resp.token);
                            let mut guard = chat_state.lock().await;
                            *guard = Some((resp.chat_id, resp.token, resp.username.clone()));
                        }
                        ProtocolMessage::LeaveChatResponse(_) => {
                            y_println!("Left chat");
                        }
                        other => {
                            if let ProtocolMessage::ErrorResponse(err) = other {
                                y_println!("[Server] {:?} | {:?}", err.code, err.message);
                            }
                        }
                    },
                    Err(e) => {
                        debug_r_eprintln!("Read error: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(ChatClient { send_chan, chat_state })
    }

    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
        let stdin = tokio::io::BufReader::new(tokio::io::stdin());
        let mut lines_stream = stdin.lines();

        while let Some(line) = lines_stream.next_line().await? {
            // straight up magic. I didn't know you could do this with ANSI codes.
            // basically deletes user input
            print!("\x1B[1A\x1B[2K");

            // typing /send every time is annoying. if command doesnt start with '/' implicit send mode.
            let cmd = if line.starts_with('/') { line.parse().unwrap_or(Command::Invalid) } else { Command::Send(line.clone()) };
            match cmd {
                Command::Help => {
                    y_println!("{}", HELP_TEXT);
                }
                Command::Create(password) => {
                    let req = ProtocolMessage::CreateChatRequest(CreateChatRequest { password });
                    self.send_chan.send(Packet { version: 1, message: req })?;
                }
                Command::Join { chat_id, username, password } => {
                    let req = ProtocolMessage::JoinChatRequest(JoinChatRequest { chat_id, username, password });
                    self.send_chan.send(Packet { version: 1, message: req })?;
                }
                Command::Send(msg) => {
                    let guard = self.chat_state.lock().await;
                    if let Some((chat_id, token, ref username)) = *guard {
                        println!("{}: {}", username.blue(), msg);
                        let req = ProtocolMessage::SendMessageRequest(SendMessageRequest { chat_id, token, message: msg });
                        self.send_chan.send(Packet { version: 1, message: req })?;
                    } else {
                        y_println!("You must /join a chat before sending");
                    }
                }
                Command::Leave => {
                    let mut guard = self.chat_state.lock().await;
                    if let Some((chat_id, token, _)) = *guard {
                        *guard = None;
                        let req = ProtocolMessage::LeaveChatRequest(LeaveChatRequest { chat_id, token });
                        self.send_chan.send(Packet { version: 1, message: req })?;
                    } else {
                        y_println!("You are not in a chat");
                    }
                }
                Command::Exit => {
                    break;
                }
                Command::Invalid => {
                    y_println!("Invalid command. Type /help to see correct syntax");
                }
            }
        }

        Ok(())
    }
}
