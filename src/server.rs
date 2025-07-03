use std::collections::HashMap;
use tokio::{io::AsyncReadExt, io::AsyncWriteExt, net::TcpListener};

struct ChatRoom {
    users: Vec<String>,
    password: Option<String>,
    messages: Vec<String>,
}

pub struct ChatServer {
    port: i32,
    tokens: HashMap<String, String>, // token to username
    chats: HashMap<i32, ChatRoom>,   // ChatId to Chat
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

        loop {
            let (mut socket, _) = listener.accept().await?;

            tokio::spawn(async move {
                let mut buf = [0; 1024];

                // In a loop, read data from the socket and write the data back.
                loop {
                    let n = match socket.read(&mut buf).await {
                        // socket closed
                        Ok(0) => return,
                        Ok(n) => n,
                        Err(e) => {
                            eprintln!("failed to read from socket; err = {:?}", e);
                            return;
                        }
                    };

                    // Write the data back
                    if let Err(e) = socket.write_all(&buf[0..n]).await {
                        eprintln!("failed to write to socket; err = {:?}", e);
                        return;
                    }
                }
            });
        }
    }
}
