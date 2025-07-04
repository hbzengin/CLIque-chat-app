use std::io;

use serde::{Deserialize, Serialize};
use serde_json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

#[repr(u8)]
enum MessageType {
    CreateChatRequest = 1,
    CreateChatResponse = 2,
    JoinChatRequest = 3,
    JoinChatResponse = 4,
    SendMessageRequest = 5,
    SendMessageResponse = 6,
    LeaveChatRequest = 7,
    LeaveChatResponse = 8,
}

impl TryFrom<u8> for MessageType {
    type Error = io::Error;

    fn try_from(n: u8) -> Result<Self, Self::Error> {
        use self::MessageType::*;
        match n {
            1 => Ok(CreateChatRequest),
            2 => Ok(CreateChatResponse),
            3 => Ok(JoinChatRequest),
            4 => Ok(JoinChatResponse),
            5 => Ok(SendMessageRequest),
            6 => Ok(SendMessageResponse),
            7 => Ok(LeaveChatRequest),
            8 => Ok(LeaveChatResponse),
            other => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Message type now known {other}"),
            )),
        }
    }
}

// Header shared by all

// 1+4 = 5 bytes total
struct Header {
    version: u8,
    length: u32,
}

impl From<[u8; 5]> for Header {
    fn from(buf: [u8; 5]) -> Self {
        let version = buf[0];
        let length = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]);
        Header { version, length }
    }
}

struct Packet {
    version: u8,
    message: Message,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "body")]
pub enum Message {
    CreateChatRequest(CreateChatRequest),
    CreateChatResponse(CreateChatResponse),
    JoinChatRequest(JoinChatRequest),
    JoinChatResponse(JoinChatResponse),
    SendMessageRequest(SendMessageRequest),
    SendMessageResponse(SendMessageResponse),
    LeaveChatRequest(LeaveChatRequest),
    LeaveChatRespon(LeaveChatResponse),
}

/* These are the actual bodies */

#[derive(Serialize, Deserialize)]
struct CreateChatRequest {
    password: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct CreateChatResponse {
    chat_id: i32,
}

#[derive(Serialize, Deserialize)]
struct JoinChatRequest {
    chat_id: i32,
    username: String,
    password: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct JoinChatResponse {
    token: Uuid,
}

#[derive(Serialize, Deserialize)]
struct SendMessageRequest {
    token: Uuid,
    chat_id: i32,
    message: String,
}

#[derive(Serialize, Deserialize)]
struct SendMessageResponse {}

#[derive(Serialize, Deserialize)]
struct LeaveChatRequest {
    token: Uuid,
    chat_id: i32,
}

#[derive(Serialize, Deserialize)]
struct LeaveChatResponse {}

pub async fn read_message<R: AsyncReadExt + Unpin>(
    src: &mut R,
) -> Result<Packet, Box<dyn std::error::Error>> {
    let mut header_bytes = [0u8; 5];
    src.read_exact(&mut header_bytes).await?;

    let header = Header::from(header_bytes);
    let mut message_bytes = vec![0u8; header.length as usize];
    src.read_exact(&mut message_bytes).await?;
    let message: Message = serde_json::from_slice(&message_bytes)?;

    Ok(Packet {
        version: header.version,
        message,
    })
}

pub async fn write_message<W: AsyncWriteExt + Unpin>(
    dst: &mut W,
    packet: &Packet,
) -> Result<(), Box<dyn std::error::Error>> {
    let body = serde_json::to_vec(&packet.message)?;

    let len: u32 = body.len() as u32;
    let mut header = [0u8; 5];
    header[0] = packet.version;
    header[1..5].copy_from_slice(&len.to_be_bytes());

    dst.write_all(&header).await?;
    dst.write_all(&body).await?;
    Ok(())
}
