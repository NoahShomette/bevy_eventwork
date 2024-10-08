use std::{
    ops::Deref,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};

use async_channel::unbounded;
use bevy::{
    app::{App, PreUpdate},
    ecs::system::SystemParam,
    log::{debug, error, trace, warn},
    prelude::{Commands, Event, EventWriter, Mut, Res, ResMut, Trigger},
};
use dashmap::DashMap;
use futures_lite::StreamExt;

use crate::{
    error::NetworkError,
    network_message::NetworkMessage,
    runtime::{run_async, EventworkRuntime},
    serialize::{
        bincode_de, bincode_network_packet_de, bincode_network_packet_ser, bincode_ser,
        MessageSerdeFns,
    },
    AsyncChannel, Connection, ConnectionId, NetworkData, NetworkDataTypes, NetworkEvent,
    NetworkPacket, NetworkPacketSerdeFns, NetworkSerializedData, Runtime,
};

use super::{NetworkInstance, NetworkProvider};

impl<NP: NetworkProvider> std::fmt::Debug for NetworkInstance<NP> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Network [{} Connected Clients]",
            self.established_connections.len()
        )
    }
}

impl<NP: NetworkProvider> NetworkInstance<NP> {
    pub(crate) fn new(_provider: NP) -> Self {
        Self {
            recv_message_map: Arc::new(DashMap::new()),
            established_connections: Arc::new(DashMap::new()),
            new_connections: AsyncChannel::new(),
            disconnected_connections: AsyncChannel::new(),
            error_channel: AsyncChannel::new(),
            server_handle: None,
            connection_tasks: Arc::new(DashMap::new()),
            connection_task_counts: AtomicU32::new(0),
            connection_count: 0,
        }
    }
    /// Returns true if there are any active connections
    #[inline(always)]
    pub fn has_connections(&self) -> bool {
        self.established_connections.len() > 0
    }

    /// Start listening for new clients
    ///
    /// ## Note
    /// If you are already listening for new connections, this will cancel the original listen
    pub fn listen<RT: Runtime>(
        &mut self,
        accept_info: NP::AcceptInfo,
        runtime: &RT,
        network_settings: &NP::NetworkSettings,
    ) -> Result<(), NetworkError> {
        self.stop();

        let new_connections = self.new_connections.sender.clone();
        let error_sender = self.error_channel.sender.clone();
        let settings = network_settings.clone();

        trace!("Started listening");

        self.server_handle = Some(Box::new(run_async(
            async move {
                let accept = NP::accept_loop(accept_info, settings).await;
                match accept {
                    Ok(mut listen_stream) => {
                        while let Some(connection) = listen_stream.next().await {
                            new_connections
                                .send(connection)
                                .await
                                .expect("Connection channel has closed");
                        }
                    }
                    Err(e) => error_sender
                        .send(e)
                        .await
                        .expect("Error channel has closed."),
                }
            },
            runtime,
        )));

        Ok(())
    }

    /// Start async connecting to a remote server.
    pub fn connect<RT: Runtime>(
        &self,
        connect_info: NP::ConnectInfo,
        runtime: &RT,
        network_settings: &NP::NetworkSettings,
    ) {
        debug!("Starting connection");

        let network_error_sender = self.error_channel.sender.clone();
        let connection_event_sender = self.new_connections.sender.clone();
        let settings = network_settings.clone();

        let connection_task_weak = Arc::downgrade(&self.connection_tasks);
        let task_count = self.connection_task_counts.fetch_add(1, Ordering::SeqCst);

        self.connection_tasks.insert(
            task_count,
            Box::new(run_async(
                async move {
                    match NP::connect_task(connect_info, settings).await {
                        Ok(connection) => connection_event_sender
                            .send(connection)
                            .await
                            .expect("Connection channel has closed"),
                        Err(e) => network_error_sender
                            .send(e)
                            .await
                            .expect("Error channel has closed."),
                    };

                    // Remove the connection task from our dictionary of connection tasks
                    connection_task_weak
                        .upgrade()
                        .expect("Network dropped")
                        .remove(&task_count);
                },
                runtime,
            )),
        );
    }

    /// Disconnect all clients and stop listening for new ones
    ///
    /// ## Notes
    /// This operation is idempotent and will do nothing if you are not actively listening
    pub fn stop(&mut self) {
        if let Some(mut conn) = self.server_handle.take() {
            conn.abort();
            for conn in self.established_connections.iter() {
                match self.disconnected_connections.sender.try_send(*conn.key()) {
                    Ok(_) => (),
                    Err(err) => warn!("Could not send to client because: {}", err),
                }
            }
            self.established_connections.clear();
            self.recv_message_map.clear();

            while self.new_connections.receiver.try_recv().is_ok() {}
        }
    }
}

/// A system param used to interact with the network.
///
/// - Listen for new client connections using [`Network::listen`]
/// - Connect to a server using [`Network::connect`]
/// - Send new messages using [`Network::send_message`]
/// - Send broadcasts to all connected clients using [`Network::broadcast`]
#[derive(SystemParam)]
pub struct Network<'w, 's, NP: NetworkProvider> {
    network: ResMut<'w, NetworkInstance<NP>>,
    commands: Commands<'w, 's>,
}

impl<'w, 's, NP: NetworkProvider> Network<'w, 's, NP> {
    /// Send a message to a specific client
    pub fn send_message<T: NetworkMessage>(&mut self, client_id: ConnectionId, message: T) {
        self.commands.trigger(SendMessage {
            clients: vec![client_id],
            message,
        });
    }

    /// Broadcast a message to all connected clients
    pub fn broadcast<T: NetworkMessage + Clone>(&mut self, message: T) {
        let clients = self
            .network
            .established_connections
            .deref()
            .into_iter()
            .map(|connection| connection.key().clone())
            .collect();
        self.commands.trigger(SendMessage { clients, message });
    }

    /// Disconnect a specific client
    pub fn disconnect(&self, conn_id: ConnectionId) -> Result<(), NetworkError> {
        let connection = if let Some(conn) = self.network.established_connections.remove(&conn_id) {
            conn
        } else {
            return Err(NetworkError::ConnectionNotFound(conn_id));
        };

        connection.1.stop();

        Ok(())
    }

    /// Returns true if there are any active connections
    #[inline(always)]
    pub fn has_connections(&self) -> bool {
        self.network.has_connections()
    }

    /// Start listening for new clients
    ///
    /// ## Note
    /// If you are already listening for new connections, this will cancel the original listen
    pub fn listen<RT: Runtime>(
        &mut self,
        accept_info: NP::AcceptInfo,
        runtime: &RT,
        network_settings: &NP::NetworkSettings,
    ) -> Result<(), NetworkError> {
        self.network.listen(accept_info, runtime, network_settings)
    }

    /// Start async connecting to a remote server.
    pub fn connect<RT: Runtime>(
        &self,
        connect_info: NP::ConnectInfo,
        runtime: &RT,
        network_settings: &NP::NetworkSettings,
    ) {
        self.network
            .connect(connect_info, runtime, network_settings);
    }

    /// Disconnect all clients and stop listening for new ones
    ///
    /// ## Notes
    /// This operation is idempotent and will do nothing if you are not actively listening
    pub fn stop(&mut self) {
        self.network.stop();
    }
}

pub(crate) fn handle_new_incoming_connections<NP: NetworkProvider, RT: Runtime>(
    mut server: ResMut<NetworkInstance<NP>>,
    network_serde: Res<NetworkPacketSerdeFns>,
    runtime: Res<EventworkRuntime<RT>>,
    network_settings: Res<NP::NetworkSettings>,
    mut network_events: EventWriter<NetworkEvent>,
) {
    while let Ok(new_conn) = server.new_connections.receiver.try_recv() {
        let id = server.connection_count;
        let conn_id = ConnectionId { id };
        server.connection_count += 1;

        let (read_half, write_half) = NP::split(new_conn);
        let recv_message_map = server.recv_message_map.clone();
        let read_network_settings = network_settings.clone();
        let write_network_settings = network_settings.clone();
        let disconnected_connections = server.disconnected_connections.sender.clone();

        let (outgoing_tx, outgoing_rx) = unbounded();
        let (incoming_tx, incoming_rx) = unbounded();

        let de = network_serde.network_packet_de.clone();
        let ser = network_serde.network_packet_ser.clone();
        server.established_connections.insert(
                conn_id,
                Connection {
                    receive_task: Box::new(run_async(async move {
                        trace!("Starting listen task for {}", id);
                        NP::recv_loop(read_half, incoming_tx, read_network_settings, de).await;

                        match disconnected_connections.send(conn_id).await {
                            Ok(_) => (),
                            Err(_) => {
                                error!("Could not send disconnected event, because channel is disconnected");
                            }
                        }
                    }, &runtime.0)),
                    map_receive_task: Box::new(run_async(async move{
                        while let Ok(packet) = incoming_rx.recv().await{
                            match recv_message_map.get_mut(&packet.kind[..]) {
                                Some(mut packets) => packets.push((conn_id, packet.data)),
                                None => {
                                    error!("Could not find existing entries for message kinds: {:?}", packet);
                                }
                            }
                        }
                    }, &runtime.0)),
                    send_task: Box::new(run_async(async move {
                        trace!("Starting send task for {}", id);
                        NP::send_loop(write_half, outgoing_rx, write_network_settings, ser).await;
                    }, &runtime.0)),
                    send_message: outgoing_tx,
                    //addr: new_conn.addr,
                },
            );

        network_events.send(NetworkEvent::Connected(conn_id));
    }

    while let Ok(disconnected_connection) = server.disconnected_connections.receiver.try_recv() {
        server
            .established_connections
            .remove(&disconnected_connection);
        network_events.send(NetworkEvent::Disconnected(disconnected_connection));
    }
}

/// A utility trait on [`App`] to easily register [`NetworkMessage`]s
pub trait AppNetworkMessage {
    /// Register a network message type
    ///
    /// ## Details
    /// This will:
    /// - Add a new event type of [`NetworkData<T>`]
    /// - Register the type for transformation over the wire
    /// - Internal bookkeeping
    fn register_message<T: NetworkMessage, NP: NetworkProvider>(&mut self) -> &mut Self;

    /// Register a network message type with a custom serialization format
    ///
    /// ## Note
    ///
    /// Don't forget to register your new [`NetworkSerialization`] format on the server using [`crate::serialize::register_serialization`] otherwise servers will silently fail.
    ///
    /// ## Details
    /// This will do everything that [`AppNetworkMessage::listen_for_message`] will do but with the given custom ser/de functions and serialization format:
    fn register_message_with<T: NetworkMessage, NP: NetworkProvider>(
        &mut self,
        data_type: NetworkDataTypes,
        de_fn: fn(data: &NetworkSerializedData) -> Result<T, String>,
        ser_fn: fn(data: &T) -> Result<NetworkSerializedData, String>,
        network_packet_de_fn: fn(data: NetworkSerializedData) -> Result<NetworkPacket, String>,
        network_packet_ser_fn: fn(data: NetworkPacket) -> Result<NetworkSerializedData, String>,
    ) -> &mut Self;
}

impl AppNetworkMessage for App {
    fn register_message<T: NetworkMessage, NP: NetworkProvider>(&mut self) -> &mut Self {
        self.register_message_with::<T, NP>(
            NetworkDataTypes::Binary,
            bincode_de::<T>,
            bincode_ser::<T>,
            bincode_network_packet_de,
            bincode_network_packet_ser,
        )
    }

    fn register_message_with<T: NetworkMessage, NP: NetworkProvider>(
        &mut self,
        data_type: NetworkDataTypes,
        de_fn: fn(data: &NetworkSerializedData) -> Result<T, String>,
        ser_fn: fn(data: &T) -> Result<NetworkSerializedData, String>,
        network_packet_de_fn: fn(data: NetworkSerializedData) -> Result<NetworkPacket, String>,
        network_packet_ser_fn: fn(data: NetworkPacket) -> Result<NetworkSerializedData, String>,
    ) -> &mut Self {
        let server = self.world().get_resource::<NetworkInstance<NP>>().expect("Could not find `Network`. Be sure to include the `ServerPlugin` before listening for server messages.");

        debug!("Registered a new ServerMessage: {}", T::NAME);
        assert!(
            !server.recv_message_map.contains_key(T::NAME),
            "Duplicate registration of ServerMessage: {}",
            T::NAME
        );
        server.recv_message_map.insert(T::NAME, Vec::new());

        self.world_mut()
            .insert_resource(MessageSerdeFns::<T> { de_fn, ser_fn });

        self.world_mut().resource_scope(
            |_world, mut network_packet_serde_fn: Mut<NetworkPacketSerdeFns>| {
                network_packet_serde_fn.insert_serialization_method(
                    T::NAME,
                    data_type,
                    network_packet_de_fn,
                    network_packet_ser_fn,
                );
            },
        );
        self.observe(send_message::<T, NP>);

        self.add_event::<NetworkData<T>>();
        self.add_event::<SendMessage<T>>();
        self.add_systems(PreUpdate, register_message::<T, NP>)
    }
}

pub(crate) fn register_message<T, NP: NetworkProvider>(
    net_res: ResMut<NetworkInstance<NP>>,
    mut events: EventWriter<NetworkData<T>>,
    serde_query: Res<MessageSerdeFns<T>>,
) where
    T: NetworkMessage,
{
    let mut messages = match net_res.recv_message_map.get_mut(T::NAME) {
        Some(messages) => messages,
        None => return,
    };
    events.send_batch(messages.drain(..).filter_map(|(source, msg)| {
        (serde_query.de_fn)(&msg)
            .ok()
            .map(|inner| NetworkData { source, inner })
    }));
}

/// An event that will send a message over the network to the given clients with the given message
#[derive(Event)]
pub struct SendMessage<T: NetworkMessage> {
    clients: Vec<ConnectionId>,
    message: T,
}

impl<T: NetworkMessage> SendMessage<T> {
    /// Create a new instance of [`SendMessage`]
    pub fn new(clients: Vec<ConnectionId>, message: T) -> SendMessage<T> {
        Self { clients, message }
    }
}

pub(crate) fn send_message<T, NP: NetworkProvider>(
    send_event: Trigger<SendMessage<T>>,
    net_res: ResMut<NetworkInstance<NP>>,
    mut error_events: EventWriter<MessageError>,
    serde_query: Res<MessageSerdeFns<T>>,
) where
    T: NetworkMessage,
{
    for target_id in send_event.event().clients.iter() {
        let connection = match net_res.established_connections.get(&target_id) {
            Some(conn) => conn,
            None => {
                error_events.send(MessageError {
                    kind: T::NAME.to_string(),
                    error: format!("client not found: {}", target_id),
                });
                return;
            }
        };

        let Ok(serialized_message) = (serde_query.ser_fn)(&send_event.event().message) else {
            error_events.send(MessageError {
                kind: T::NAME.to_string(),
                error: format!("failed to serialize message"),
            });
            return;
        };

        let packet = NetworkPacket {
            kind: String::from(T::NAME),
            data: serialized_message,
        };

        match connection.send_message.try_send(packet) {
            Ok(_) => (),
            Err(err) => {
                error!("There was an error sending a packet: {}", err);
                error_events.send(MessageError {
                    kind: T::NAME.to_string(),
                    error: format!("There was an error sending a packet: {}", err),
                });
                return;
            }
        }
    }
}

/// An event that will send a message over the network to the given clients with the given message
#[derive(Event)]
pub struct MessageError {
    kind: String,
    error: String,
}

impl MessageError {
    /// Construct a new [`MessageError`]
    pub fn new(kind: String, error: String) -> MessageError {
        Self { kind, error }
    }

    /// Returns the [`NetworkMessage`] kind that the error occured in relation to
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Returns the actual error string
    pub fn error(&self) -> &str {
        &self.error
    }
}
