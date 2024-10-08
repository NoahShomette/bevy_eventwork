use bevy::prelude::*;
use bevy_eventwork::{
    managers::network_request::{
        AppNetworkRequestMessage, AppNetworkResponseMessage, RequestMessage,
    },
    tcp::TcpProvider,
    NetworkMessage,
};
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RequestStatus;

impl RequestMessage for RequestStatus {
    /// The type of message that the server will send back to the client.
    /// It must implement [`NetworkMessage`]
    type ResponseMessage = StatusResponse;

    /// A unique identifying name for the request message.
    const REQUEST_NAME: &'static str = "client_request_status";
}

/// The response that the server will eventually return to the client
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StatusResponse {
    pub response: bool,
}

impl NetworkMessage for StatusResponse {
    const NAME: &'static str = "client_request_status_response";
}

#[allow(unused)]
pub fn register_network_messages(app: &mut App) {
    use bevy_eventwork::AppNetworkMessage;

    // Both the client and the server need to register all messages that they want to receive or send
    app.register_message::<NewChatMessage, TcpProvider>();
    app.register_message::<UserChatMessage, TcpProvider>();
}

/// Register request messages that we want the client to send. 
#[allow(unused)]
pub fn client_register_request_messages(app: &mut App) {
    use bevy_eventwork::AppNetworkMessage;

    app.register_send_request_message::<RequestStatus, TcpProvider>();
}

#[allow(unused)]
pub fn server_register_request_messages(app: &mut App) {
    use bevy_eventwork::AppNetworkMessage;
    /// For ease of use we just make both client and server capable of sending / recieving request status messages
    app.register_receive_request_message::<RequestStatus, TcpProvider>();
}
