//! # Request/Response Network Messages
//!
//! This documentation assumes you have an app setup correctly that can send and receive normal messages. Please refer to the repository readme
//! and the [library documentation](https://docs.rs/bevy_eventwork/latest/bevy_eventwork/index.html) for help with that.
//!
//! Note that in bevy_eventwork clients and servers use the same plugin architecture but one (the server) will listen for connections but can't connect while listening and the other (the client) will connect to other apps but can't listen when connected.
//! This is important to understand because both apps can request and receive from each other as long as the message type is setup to do so in that app.
//! In this example the client is making requests to the server but if you flipped or duplicated all the message setup the server could make requests to the client.
//!
//! ## Overview
//!
//! The request/response messages in Eventwork work as follows.
//!
//! **Client**
//!
//! - Client sends a request to the Server using the [`Requester`](self::network_request::Requester) system param.
//! - On successful sends, a [`Response`](self::network_request::Response) object is returned that will eventually return the actual response.
//! - Client continues to poll the response in order to access the actual response.
//! - When the client gets the response it consumes the response object and can read the response.
//!
//! **Server**
//!
//! - Server listens for requests of the given type.
//! - When it receives a given request, it is also given a channel to send a response back in.
//! - Server does whatever it needs to handle the request and then uses the [`Request`](self::network_request::Request) object to send a response
//!
//! ## Shared definitions
//!
//! First you need to create the messages that both the server and the client will use.
//!
//! ```rust
//! use bevy::prelude::*;
//! use bevy_eventwork::{NetworkMessage, managers::network_request::RequestMessage};
//! use serde::{Serialize, Deserialize};
//!
//! /// A request sent from an eventwork app, in this case the client,
//! /// to another eventwork app, in this case the server.
//! /// It needs to implement [`RequestMessage`]
//! #[derive(Debug, Serialize, Deserialize, Clone)]
//! struct RequestStatus;
//!
//!
//! impl RequestMessage for RequestStatus {
//!     /// The type of message that the server will send back to the client.
//!     /// It must implement [`NetworkMessage`]
//!    type ResponseMessage = StatusResponse;
//!    
//!     /// A unique identifying name for the request message.
//!    const REQUEST_NAME: &'static str = "client_request_status";
//! }
//!
//! /// The response that the server will eventually return to the client
//! #[derive(Debug, Serialize, Deserialize, Clone)]
//! struct StatusResponse{
//!     pub response: bool
//! }
//!
//! impl NetworkMessage for StatusResponse {
//!     const NAME: &'static str = "client_request_status_response";
//! }
//!
//! ```
//!
//! ## Example client app
//!
//! Setting up our client is simple, we just need to register to listen for the *Responses* of our given request.
//! This example has a basic implementation of how you can do a single request and poll for it
//!
//! ```rust
//! use bevy::prelude::*;
//! use bevy_eventwork::{
//!     NetworkMessage,
//!     ConnectionId,
//!     tcp::TcpProvider,
//!     managers::network_request::{
//!     Response,
//!     Requester,
//!     RequestMessage,
//!     AppNetworkResponseMessage},
//! };
//! use serde::{Serialize, Deserialize};
//!
//! # #[derive(Debug, Serialize, Deserialize, Clone)]
//! # struct RequestStatus;
//! # impl RequestMessage for RequestStatus {
//! #   type ResponseMessage = StatusResponse;
//! #   const REQUEST_NAME: &'static str = "client_request_status";
//! # }
//! # #[derive(Debug, Serialize, Deserialize, Clone)]
//! # struct StatusResponse{
//! #    pub response: bool
//! # }
//! # impl NetworkMessage for StatusResponse {
//! #    const NAME: &'static str = "client_request_status_response";
//! # }
//!
//! struct ClientPlugin;
//!
//! impl Plugin for ClientPlugin{
//!     fn build(&self, app: &mut App){
//!         app.listen_for_response_message::<RequestStatus, TcpProvider>();
//!         app.add_systems(Startup, client_send_status_request);
//!         app.add_systems(Update, poll_responses);
//!     }
//!
//! }        
//!
//! /// A resource that will hold our response object so we can poll it every frame
//! #[derive(Resource)]
//! struct StatusRequest(Option<Response<StatusResponse>>);
//!
//! /// A system that will send the status request and then store the response object in a resource
//! fn client_send_status_request(
//!     net: Requester<RequestStatus, TcpProvider>,
//!     status_request: Option<ResMut<StatusRequest>>,
//!     mut commands: Commands,
//! ) {
//!     if let None = status_request {
//!         let request_response = net.send_request(
//!             ConnectionId { id: 0 },
//!             RequestStatus,
//!         );
//!
//!         if let Ok(response) = request_response {
//!             commands.insert_resource(StatusRequest(Some(response)));
//!         }
//!     }
//! }
//!
//! /// A system that will poll responses every frame.
//! fn poll_responses(
//!     status_request: Option<ResMut<StatusRequest>>,
//!     mut commands: Commands,
//! ) {
//!     if let Some(mut res) = status_request {
//!        if let Some(response) = res.0.take() {
//!            let result = response.try_recv();
//!            match result {
//!                Ok(status) => {
//!                   commands.remove_resource::<StatusRequest>();
//!                     println!("status: {}", status.response);
//!               }
//!               Err(response) => res.0 = Some(response),
//!            }
//!        }
//!     }
//! }
//! ```
//!
//! ## Example Server app
//!
//! Setting up our server is simple. We just need to register to listen for the *Requests* of our given request
//!  and then add a system to receive those events.
//!
//! ```rust
//! use bevy::prelude::*;
//! use bevy_eventwork::{
//!     NetworkMessage,
//!     tcp::TcpProvider,
//!     managers::network_request::{
//!     Request,
//!     RequestMessage,
//!     AppNetworkRequestMessage},
//! };
//! use serde::{Serialize, Deserialize};
//!
//! # #[derive(Debug, Serialize, Deserialize, Clone)]
//! # struct RequestStatus;
//! # impl RequestMessage for RequestStatus {
//! #   type ResponseMessage = StatusResponse;
//! #   const REQUEST_NAME: &'static str = "client_request_status";
//! # }
//! # #[derive(Debug, Serialize, Deserialize, Clone)]
//! # struct StatusResponse{
//! #    pub response: bool
//! # }
//! # impl NetworkMessage for StatusResponse {
//! #    const NAME: &'static str = "client_request_status_response";
//! # }
//!
//! struct ServerPlugin;
//!
//! impl Plugin for ServerPlugin{
//!     fn build(&self, app: &mut App){
//!         app.listen_for_request_message::<RequestStatus, TcpProvider>();
//!         app.add_systems(Update, handle_request_status);
//!     }
//!
//! }        
//!
//! /// A system that will read status requests and return the current status of the app.
//! fn handle_request_status(
//!     mut network_events: EventReader<Request<RequestStatus>>,
//! ){
//!     for event in network_events.read() {
//!         let _ = event.clone().respond(StatusResponse{
//!             response: true    
//!         });
//!         
//!     }
//! }
//! ```

use std::{fmt::Debug, marker::PhantomData, sync::atomic::AtomicU64};

use async_channel::{Receiver, Sender};
use bevy::{
    ecs::system::SystemParam,
    prelude::{debug, App, Event, EventReader, EventWriter, Mut, PreUpdate, Res, ResMut, Resource},
};
use dashmap::DashMap;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{
    error::NetworkError,
    managers::network::{send_message, SendMessage},
    serialize::{
        bincode_de, bincode_network_packet_de, bincode_network_packet_ser, bincode_ser,
        MessageSerdeFns,
    },
    ConnectionId, NetworkData, NetworkDataTypes, NetworkMessage, NetworkPacket,
    NetworkPacketSerdeFns, NetworkSerializedData,
};

use super::{
    network::{register_message, Network},
    NetworkInstance, NetworkProvider,
};

#[derive(SystemParam)]
/// A wrapper around [`Network`] that allows for the sending of [`RequestMessage`]'s.
pub struct Requester<'w, 's, T: RequestMessage, NP: NetworkProvider> {
    server: Network<'w, 's, NP>,
    response_map: Res<'w, ResponseMap<T>>,
    #[system_param(ignore)]
    marker: PhantomData<&'s usize>,
}

impl<'w, 's, T: RequestMessage, NP: NetworkProvider> Requester<'w, 's, T, NP> {
    /// Sends a request and returns an object that will eventually return the response
    pub fn send_request(
        &mut self,
        client_id: ConnectionId,
        request: T,
    ) -> Result<Response<T::ResponseMessage>, NetworkError> {
        let (id, response) = self.response_map.get_responder();
        self.server
            .send_message(client_id, RequestInternal { id, request });
        Ok(response)
    }
}

/// The eventual response of a remote request.
#[derive(Debug)]
pub struct Response<T> {
    rx: Receiver<T>,
}

impl<T> Response<T> {
    /// Try to recieve the response, then drop the underlying machinery for handling the request.
    /// On err, we simply return the object to be checked again later.
    pub fn try_recv(self) -> Result<T, Response<T>> {
        if let Ok(res) = self.rx.try_recv() {
            Ok(res)
        } else {
            Err(self)
        }
    }
}

#[derive(Debug, Resource)]
/// Technically an internal type, public for use in system pram
pub struct ResponseMap<T: RequestMessage> {
    count: AtomicU64,
    map: DashMap<u64, Sender<T::ResponseMessage>>,
}

impl<T: RequestMessage> Default for ResponseMap<T> {
    fn default() -> Self {
        Self {
            count: Default::default(),
            map: DashMap::new(),
        }
    }
}

impl<T: RequestMessage> ResponseMap<T> {
    fn get_responder(&self) -> (u64, Response<T::ResponseMessage>) {
        let id = self
            .count
            .fetch_add(1, core::sync::atomic::Ordering::SeqCst);
        let (tx, rx) = async_channel::bounded(1);
        self.map.insert(id, tx);
        (id, Response { rx })
    }

    fn remove(&self, id: &u64) -> Option<Sender<T::ResponseMessage>> {
        self.map.remove(id).map(|inner| inner.1)
    }
}

/// Marks a type as a request type.
pub trait RequestMessage:
    Clone + Serialize + DeserializeOwned + Send + Sync + Debug + 'static
{
    /// The response type for the request.
    type ResponseMessage: NetworkMessage
        + Clone
        + Serialize
        + DeserializeOwned
        + Send
        + Sync
        + Debug
        + 'static;

    /// The label used for the request type, same rules as [`NetworkMessage`] in terms of naming.
    const REQUEST_NAME: &'static str;
}

/// Internal message sent and managed by the app.
///
/// Only exposed so that custom serde methods can be made. You should never touch this other than with [`AppNetworkResponseMessage::register_send_request_message_with`] or [`AppNetworkRequestMessage::register_receive_request_message_with`]
#[derive(Serialize, Deserialize)]
pub struct RequestInternal<T> {
    id: u64,
    request: T,
}

impl<T: RequestMessage> NetworkMessage for RequestInternal<T> {
    const NAME: &'static str = T::REQUEST_NAME;
}

/// A wrapper around a request that allows sending a response that will automatically be written
///  to eventwork for network transmission.
#[derive(Debug, Event, Clone)]
pub struct Request<T: RequestMessage> {
    request: T,
    source: ConnectionId,
    request_id: u64,
    response_tx: Sender<NetworkPacket>,
    ser_fn:
        fn(data: &ResponseInternal<T::ResponseMessage>) -> Result<NetworkSerializedData, String>,
}

impl<T: RequestMessage> Request<T> {
    /// Read the underlying request
    #[inline(always)]
    pub fn get_request(&self) -> &T {
        &self.request
    }

    /// Read the source of the underlying request
    #[inline(always)]
    pub fn source(&self) -> &ConnectionId {
        &self.source
    }

    /// Consume the request and automatically send the response back to the client.
    pub fn respond(self, response: T::ResponseMessage) -> Result<(), NetworkError> {
        let packet = NetworkPacket {
            kind: String::from(T::ResponseMessage::NAME),
            data: (self.ser_fn)(&ResponseInternal {
                response_id: self.request_id,
                response,
            })
            .map_err(|_| NetworkError::Serialization)?,
        };

        self.response_tx
            .try_send(packet)
            .map_err(|_| NetworkError::SendError)
    }
}

/// A utility trait on [`App`] to easily register [`RequestMessage`]s for the app to recieve
pub trait AppNetworkRequestMessage {
    /// Registers the given [`RequestMessage::ResponseMessage`] to be sent and the [`RequestMessage`] to be received using Bincode
    fn register_receive_request_message<T: RequestMessage, NP: NetworkProvider>(
        &mut self,
    ) -> &mut Self;

    /// Registers the given [`RequestMessage::ResponseMessage`] to be sent and the [`RequestMessage`] to be received using custom serde functions
    fn register_receive_request_message_with<T: RequestMessage, NP: NetworkProvider>(
        &mut self,
        data_type: NetworkDataTypes,
        request_de_fn: fn(data: &NetworkSerializedData) -> Result<RequestInternal<T>, String>,
        request_ser_fn: fn(data: &RequestInternal<T>) -> Result<NetworkSerializedData, String>,
        network_packet_de_fn: fn(data: NetworkSerializedData) -> Result<NetworkPacket, String>,
        network_packet_ser_fn: fn(data: NetworkPacket) -> Result<NetworkSerializedData, String>,
        response_de_fn: fn(
            data: &NetworkSerializedData,
        ) -> Result<ResponseInternal<T::ResponseMessage>, String>,
        response_ser_fn: fn(
            data: &ResponseInternal<T::ResponseMessage>,
        ) -> Result<NetworkSerializedData, String>,
    ) -> &mut Self;
}

impl AppNetworkRequestMessage for App {
    fn register_receive_request_message_with<T: RequestMessage, NP: NetworkProvider>(
        &mut self,
        data_type: NetworkDataTypes,
        request_de_fn: fn(data: &NetworkSerializedData) -> Result<RequestInternal<T>, String>,
        request_ser_fn: fn(data: &RequestInternal<T>) -> Result<NetworkSerializedData, String>,
        network_packet_de_fn: fn(data: NetworkSerializedData) -> Result<NetworkPacket, String>,
        network_packet_ser_fn: fn(data: NetworkPacket) -> Result<NetworkSerializedData, String>,
        response_de_fn: fn(
            data: &NetworkSerializedData,
        ) -> Result<ResponseInternal<T::ResponseMessage>, String>,
        response_ser_fn: fn(
            data: &ResponseInternal<T::ResponseMessage>,
        ) -> Result<NetworkSerializedData, String>,
    ) -> &mut Self {
        let client = self.world().get_resource::<NetworkInstance<NP>>().expect("Could not find `Network`. Be sure to include the `EventworkPlugin` before listening for server messages.");
        debug!(
            "Registered a new ServerMessage: {}",
            RequestInternal::<T>::NAME
        );

        assert!(
            !client
                .recv_message_map
                .contains_key(RequestInternal::<T>::NAME),
            "Duplicate registration of RequestMessage: {}",
            RequestInternal::<T>::NAME
        );
        client
            .recv_message_map
            .insert(RequestInternal::<T>::NAME, Vec::new());
        self.world_mut()
            .insert_resource(MessageSerdeFns::<ResponseInternal<T::ResponseMessage>> {
                de_fn: response_de_fn,
                ser_fn: response_ser_fn,
            });
        self.world_mut()
            .insert_resource(MessageSerdeFns::<RequestInternal<T>> {
                de_fn: request_de_fn,
                ser_fn: request_ser_fn,
            });
        self.world_mut().resource_scope(
            |_world, mut network_packet_serde_fn: Mut<NetworkPacketSerdeFns>| {
                network_packet_serde_fn.insert_serialization_method(
                    ResponseInternal::<T::ResponseMessage>::NAME,
                    data_type,
                    network_packet_de_fn,
                    network_packet_ser_fn,
                );
            },
        );
        self.world_mut().resource_scope(
            |_world, mut network_packet_serde_fn: Mut<NetworkPacketSerdeFns>| {
                network_packet_serde_fn.insert_serialization_method(
                    T::REQUEST_NAME,
                    data_type,
                    network_packet_de_fn,
                    network_packet_ser_fn,
                );
            },
        );
        self.add_event::<NetworkData<RequestInternal<T>>>();
        self.add_event::<Request<T>>();
        self.observe(send_message::<ResponseInternal<T::ResponseMessage>, NP>);
        self.add_systems(
            PreUpdate,
            (
                create_request_handlers::<T, NP>,
                (register_message::<RequestInternal<T>, NP>,),
            ),
        )
    }

    fn register_receive_request_message<T: RequestMessage, NP: NetworkProvider>(
        &mut self,
    ) -> &mut Self {
        self.register_receive_request_message_with::<T, NP>(
            NetworkDataTypes::Binary,
            bincode_de::<RequestInternal<T>>,
            bincode_ser::<RequestInternal<T>>,
            bincode_network_packet_de,
            bincode_network_packet_ser,
            bincode_de::<ResponseInternal<T::ResponseMessage>>,
            bincode_ser::<ResponseInternal<T::ResponseMessage>>,
        )
    }
}

fn create_request_handlers<T: RequestMessage, NP: NetworkProvider>(
    mut requests: EventReader<NetworkData<RequestInternal<T>>>,
    mut requests_wrapped: EventWriter<Request<T>>,
    network: Res<NetworkInstance<NP>>,
    serde: Res<MessageSerdeFns<ResponseInternal<T::ResponseMessage>>>,
) {
    for request in requests.read() {
        if let Some(connection) = &network.established_connections.get(request.source()) {
            requests_wrapped.send(Request {
                request: request.request.clone(),
                request_id: request.id,
                response_tx: connection.send_message.clone(),
                source: request.source,
                ser_fn: serde.ser_fn.clone(),
            });
        }
    }
}

/// Internal message sent and managed by the app.
///
/// Only exposed so that custom serde methods can be made. You should never touch this other than with [`AppNetworkResponseMessage::register_send_request_message_with`] or [`AppNetworkRequestMessage::register_request_message_with`]
#[derive(Serialize, Deserialize)]
pub struct ResponseInternal<T> {
    response_id: u64,
    response: T,
}

impl<T: NetworkMessage> NetworkMessage for ResponseInternal<T> {
    const NAME: &'static str = T::NAME;
}

/// A utility trait on [`App`] to easily register [`RequestMessage::ResponseMessage`]s for clients to recieve
pub trait AppNetworkResponseMessage {
    /// Registers the given [`RequestMessage`] to be sent and the [`RequestMessage::ResponseMessage`] to be received using Bincode
    fn register_send_request_message<T: RequestMessage, NP: NetworkProvider>(
        &mut self,
    ) -> &mut Self;

    /// Registers the given [`RequestMessage`] to be sent and the [`RequestMessage::ResponseMessage`] to be received using custom serde functions
    fn register_send_request_message_with<T: RequestMessage, NP: NetworkProvider>(
        &mut self,
        data_type: NetworkDataTypes,
        request_de_fn: fn(data: &NetworkSerializedData) -> Result<RequestInternal<T>, String>,
        request_ser_fn: fn(data: &RequestInternal<T>) -> Result<NetworkSerializedData, String>,
        network_packet_de_fn: fn(data: NetworkSerializedData) -> Result<NetworkPacket, String>,
        network_packet_ser_fn: fn(data: NetworkPacket) -> Result<NetworkSerializedData, String>,
        response_de_fn: fn(
            data: &NetworkSerializedData,
        ) -> Result<ResponseInternal<T::ResponseMessage>, String>,
        response_ser_fn: fn(
            data: &ResponseInternal<T::ResponseMessage>,
        ) -> Result<NetworkSerializedData, String>,
    ) -> &mut Self;
}

impl AppNetworkResponseMessage for App {
    fn register_send_request_message_with<T: RequestMessage, NP: NetworkProvider>(
        &mut self,
        data_type: NetworkDataTypes,
        request_de_fn: fn(data: &NetworkSerializedData) -> Result<RequestInternal<T>, String>,
        request_ser_fn: fn(data: &RequestInternal<T>) -> Result<NetworkSerializedData, String>,
        network_packet_de_fn: fn(data: NetworkSerializedData) -> Result<NetworkPacket, String>,
        network_packet_ser_fn: fn(data: NetworkPacket) -> Result<NetworkSerializedData, String>,
        response_de_fn: fn(
            data: &NetworkSerializedData,
        ) -> Result<ResponseInternal<T::ResponseMessage>, String>,
        response_ser_fn: fn(
            data: &ResponseInternal<T::ResponseMessage>,
        ) -> Result<NetworkSerializedData, String>,
    ) -> &mut Self {
        self.insert_resource(ResponseMap::<T>::default());
        let client = self.world().get_resource::<NetworkInstance<NP>>().expect("Could not find `Network`. Be sure to include the `EventworkPlugin` before listening for server messages.");
        debug!(
            "Registered a new ServerMessage: {}",
            RequestInternal::<T>::NAME
        );

        assert!(
            !client
                .recv_message_map
                .contains_key(ResponseInternal::<T::ResponseMessage>::NAME),
            "Duplicate registration of ResponseMessage: {}",
            ResponseInternal::<T::ResponseMessage>::NAME
        );
        client
            .recv_message_map
            .insert(ResponseInternal::<T::ResponseMessage>::NAME, Vec::new());
        self.world_mut()
            .insert_resource(MessageSerdeFns::<ResponseInternal<T::ResponseMessage>> {
                de_fn: response_de_fn,
                ser_fn: response_ser_fn,
            });
        self.world_mut()
            .insert_resource(MessageSerdeFns::<RequestInternal<T>> {
                de_fn: request_de_fn,
                ser_fn: request_ser_fn,
            });

        self.world_mut().resource_scope(
            |_world, mut network_packet_serde_fn: Mut<NetworkPacketSerdeFns>| {
                network_packet_serde_fn.insert_serialization_method(
                    ResponseInternal::<T::ResponseMessage>::NAME,
                    data_type,
                    network_packet_de_fn,
                    network_packet_ser_fn,
                );
            },
        );
        self.world_mut().resource_scope(
            |_world, mut network_packet_serde_fn: Mut<NetworkPacketSerdeFns>| {
                network_packet_serde_fn.insert_serialization_method(
                    T::REQUEST_NAME,
                    data_type,
                    network_packet_de_fn,
                    network_packet_ser_fn,
                );
            },
        );
        self.observe(send_message::<RequestInternal<T>, NP>);

        self.add_event::<NetworkData<ResponseInternal<T::ResponseMessage>>>();
        self.add_event::<SendMessage<ResponseInternal<T::ResponseMessage>>>();
        self.add_systems(
            PreUpdate,
            (
                register_message::<ResponseInternal<T::ResponseMessage>, NP>,
                create_client_response_handlers::<T>,
            ),
        )
    }

    fn register_send_request_message<T: RequestMessage, NP: NetworkProvider>(
        &mut self,
    ) -> &mut Self {
        self.register_send_request_message_with::<T, NP>(
            NetworkDataTypes::Binary,
            bincode_de::<RequestInternal<T>>,
            bincode_ser::<RequestInternal<T>>,
            bincode_network_packet_de,
            bincode_network_packet_ser,
            bincode_de::<ResponseInternal<T::ResponseMessage>>,
            bincode_ser::<ResponseInternal<T::ResponseMessage>>,
        )
    }
}

fn create_client_response_handlers<T: RequestMessage>(
    mut responses: EventReader<NetworkData<ResponseInternal<T::ResponseMessage>>>,
    response_map: ResMut<ResponseMap<T>>,
) {
    for response in responses.read() {
        if let Some(sender) = response_map.remove(&response.response_id) {
            sender
                .try_send(response.response.clone())
                .expect("Internal channel closed!");
        }
    }
}
