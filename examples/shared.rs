use bevy::prelude::*;
use bevy_eventwork::{tcp::TcpProvider, NetworkMessage};
use serde::{Deserialize, Serialize};

/////////////////////////////////////////////////////////////////////
// In this example the client sends `UserChatMessage`s to the server,
// the server then broadcasts to all connected clients.
//
// We use two different types here, because only the server should
// decide the identity of a given connection and thus also sends a
// name.
//
// You can have a single message be sent both ways, it simply needs
// to implement both `NetworkMessage" and both client and server can
// send and recieve
/////////////////////////////////////////////////////////////////////

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UserChatMessage {
    pub message: String,
}

impl NetworkMessage for UserChatMessage {
    const NAME: &'static str = "example:UserChatMessage";
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NewChatMessage {
    pub name: String,
    pub message: String,
}

impl NetworkMessage for NewChatMessage {
    const NAME: &'static str = "example:NewChatMessage";
}

#[allow(unused)]
pub fn register_network_messages(app: &mut App) {
    use bevy_eventwork::AppNetworkMessage;

    // Both the client and the server need to register all messages that they want to receive or send
    app.register_message::<NewChatMessage, TcpProvider>();
    app.register_message::<UserChatMessage, TcpProvider>();
}
