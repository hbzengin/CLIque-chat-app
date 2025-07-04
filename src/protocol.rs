use serde::{Deserialize, Serialize};
use serde_json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug)]
pub struct ErrorResponse {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    WrongPassword,
    ChatNotFound,
    InvalidFormat,
    Unauthorized,
    InternalError,
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

pub struct Packet {
    pub version: u8,
    pub message: Message,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case", content = "body")]
pub enum Message {
    CreateChatRequest(CreateChatRequest),
    CreateChatResponse(CreateChatResponse),
    JoinChatRequest(JoinChatRequest),
    JoinChatResponse(JoinChatResponse),
    SendMessageRequest(SendMessageRequest),
    SendMessageResponse(SendMessageResponse),
    LeaveChatRequest(LeaveChatRequest),
    LeaveChatResponse(LeaveChatResponse),
    ErrorResponse(ErrorResponse),
}

/* These are the actual bodies */

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateChatRequest {
    pub password: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateChatResponse {
    pub chat_id: Uuid,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JoinChatRequest {
    pub chat_id: Uuid,
    pub username: String,
    pub password: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JoinChatResponse {
    pub token: Uuid,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SendMessageRequest {
    pub token: Uuid,
    pub chat_id: Uuid,
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SendMessageResponse {}

#[derive(Serialize, Deserialize, Debug)]
pub struct LeaveChatRequest {
    pub token: Uuid,
    pub chat_id: Uuid,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LeaveChatResponse {}

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
