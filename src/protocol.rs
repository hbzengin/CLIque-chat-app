use uuid::Uuid;

// Header shared by all
struct Header {
    version: u8,
    msg_type: u8,
    length: u32,
}

/* These are the actual bodies */
struct CreateChatRequest {
    password: Option<String>,
}

struct CreateChatResponse {
    chat_id: i32,
}

struct JoinChatRequest {
    chat_id: i32,
    username: String,
    password: Option<String>,
}

struct JoinChatResponse {
    token: Uuid,
}

struct SendMessageRequest {
    token: Uuid,
    chat_id: i32,
    message: String,
}

struct SendMessageResponse {}

struct LeaveChatRequest {
    token: Uuid,
    chat_id: i32,
}

struct LeaveChatResponse {}

pub async fn<R: AsyncReadExt + Unpin>(src: &mut R) -> Result<>