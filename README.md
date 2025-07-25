# CLI Chat Application in Rust

This repo contains a real-time chat application in Rust built with a client-server architecture using TCP sockets and async handling. The app supports multiple chat rooms with optional passwords, very basic user authentication, and real-time message broadcasting. It is built on a custom messaging protocol for message passing between clients and server.

The server handles multiple concurrent connections using Tokio's async runtime while maintaining chat rooms and broadcasting messages. Clients can create password-protected rooms, join existing rooms with usernames, send messages, and receive real-time broadcasts from other users in the same room.

## Features

- Custom protocol with JSON message bodies
- Async TCP networking with Tokio
- Real-time chat with multiple concurrent users
- Password-protected chat rooms with hashing
- Message broadcasting using tokio::sync::broadcast channels
- Command-line interface with colored output

## Usage

This app has three components: a shared protocol library, a server, and a client. You can run both server and client using Cargo. The server listens on port 8080 by default and clients can connect to create or join chat rooms.

```bash
# Start the server (runs on localhost:8080)
cargo run -p server

# Start a client (connects to localhost:8080)
cargo run -p client

# Connect client to custom host/port
cargo run -p client 192.168.1.100 8080
```

### Client Commands

The client has a CLI for using chat rooms:

```bash
# Create a new chat room (optionally password-protected)
/create
/create room_password

# Join an existing chat room with "chat_id" and "username"
/join 550e8400-e29b-41d4-a716-446655440000 alice
/join 550e8400-e29b-41d4-a716-446655440000 alice room_password

# Send a message to the current chat room
/send Hello everyone!
# Or for convenience, args without a '/' are implicitly '/send' commands
Hello everyone!

# Leave the current chat room
/leave

# Exit the application
/exit
```
