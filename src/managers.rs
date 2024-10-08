use std::sync::{atomic::AtomicU32, Arc};

use async_channel::{Receiver, Sender};
use async_trait::async_trait;
use bevy::prelude::Resource;
use dashmap::DashMap;
use futures_lite::Stream;

use crate::serialize::{bincode_network_packet_de, bincode_network_packet_ser};
use crate::{error::NetworkError, runtime::JoinHandle, AsyncChannel, ConnectionId, NetworkPacket};
use crate::{Connection, NetworkSerializedData};
/// Contains logic for using [`Network`]
pub mod network;
/// Contains logic for making requests with expected responses
pub mod network_request;

/// An instance of a Network that uses the provided [`NetworkProvider`] to drive itself.
///
/// You can use this resource to interact with the network in Bevy systems.
///
/// - Listen for new client connections using [`Network::listen`]
/// - Connect to a server using [`Network::connect`]
/// - Send new messages using [`Network::send_message`]
/// - Send broadcasts to all connected clients using [`Network::broadcast`]
#[derive(Resource)]
pub struct NetworkInstance<NP: NetworkProvider> {
    recv_message_map: Arc<DashMap<&'static str, Vec<(ConnectionId, NetworkSerializedData)>>>,
    established_connections: Arc<DashMap<ConnectionId, Connection>>,
    new_connections: AsyncChannel<NP::Socket>,
    disconnected_connections: AsyncChannel<ConnectionId>,
    error_channel: AsyncChannel<NetworkError>,
    server_handle: Option<Box<dyn JoinHandle>>,
    connection_tasks: Arc<DashMap<u32, Box<dyn JoinHandle>>>,
    connection_task_counts: AtomicU32,
    connection_count: u32,
}

/// Resource which holds functions used to deserialize and serialize packets going into and out of the network.
///
/// To override the default bincode based functions insert this resource into your app with your own functions.
#[derive(Resource)]
pub struct NetworkPacketSerdeFn {
    /// Deserializes [`NetworkSerializedData`] into [`NetworkPacket`]s to send into the app
    pub network_packet_de: fn(data: NetworkSerializedData) -> Result<NetworkPacket, String>,
    /// Deserializes [`NetworkPacket`] into [`NetworkSerializedData`]s to send into the network to another app
    pub network_packet_ser: fn(data: NetworkPacket) -> Result<NetworkSerializedData, String>,
}

impl Default for NetworkPacketSerdeFn {
    fn default() -> Self {
        Self {
            network_packet_de: bincode_network_packet_de,
            network_packet_ser: bincode_network_packet_ser,
        }
    }
}

/// A trait used to drive the network. This is responsible
/// for generating the futures that carryout the underlying app network logic.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait NetworkProvider: 'static + Send + Sync {
    /// This is to configure particular protocols
    type NetworkSettings: Resource + Clone;

    /// The type that acts as a combined sender and reciever for the network.
    /// This type needs to be able to be split.
    type Socket: Send + Sync + Clone;

    /// The read half of the given socket type.
    type ReadHalf: Send + Sync + Clone;

    /// The write half of the given socket type.
    type WriteHalf: Send + Sync + Clone;

    /// Info necessary to start a connection, an [`std::net::SocketAddr`] for instance
    type ConnectInfo: Send + Sync;

    /// Info necessary to start a connection, an [`std::net::SocketAddr`] for instance
    type AcceptInfo: Send + Sync;

    /// The output type of [`Self::accept_loop`]
    type AcceptStream: Stream<Item = Self::Socket> + Unpin + Send;

    /// This will be spawned as a background operation to continuously add new connections.
    async fn accept_loop(
        accept_info: Self::AcceptInfo,
        network_settings: Self::NetworkSettings,
    ) -> Result<Self::AcceptStream, NetworkError>;

    /// Attempts to connect to a remote
    async fn connect_task(
        connect_info: Self::ConnectInfo,
        network_settings: Self::NetworkSettings,
    ) -> Result<Self::Socket, NetworkError>;

    /// Recieves messages over the network, forwards them to Eventwork via a sender.
    async fn recv_loop(
        read_half: Self::ReadHalf,
        messages: Sender<NetworkPacket>,
        settings: Self::NetworkSettings,
        network_packet_de: fn(data: NetworkSerializedData) -> Result<NetworkPacket, String>,
    );

    /// Sends messages over the network, receives packages from Eventwork via receiver.
    async fn send_loop(
        write_half: Self::WriteHalf,
        messages: Receiver<NetworkPacket>,
        settings: Self::NetworkSettings,
        network_packet_ser: fn(data: NetworkPacket) -> Result<NetworkSerializedData, String>,
    );

    /// Split the socket into a read and write half, so that the two actions
    /// can be handled concurrently.
    fn split(combined: Self::Socket) -> (Self::ReadHalf, Self::WriteHalf);
}
