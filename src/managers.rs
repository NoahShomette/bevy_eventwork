use std::sync::{atomic::AtomicU32, Arc};

use async_channel::{Receiver, Sender};
use async_trait::async_trait;
use dashmap::DashMap;
use futures_lite::Stream;

use crate::{
    error::NetworkError, runtime::JoinHandle, AsyncChannel, Connection, ConnectionId, NetworkPacket,
};

/// Contains logic for using [`Network`]
pub mod network;
/// Contains logic for making requests with expected responses to a server
pub mod network_request;

/// An instance of a [`NetworkServer`] is used to listen for new client connections
/// using [`NetworkServer::listen`]
pub struct Network<NP: NetworkProvider> {
    recv_message_map: Arc<DashMap<&'static str, Vec<(ConnectionId, Vec<u8>)>>>,
    established_connections: Arc<DashMap<ConnectionId, Connection>>,
    new_connections: AsyncChannel<NP::Socket>,
    disconnected_connections: AsyncChannel<ConnectionId>,
    error_channel: AsyncChannel<NetworkError>,
    server_handle: Option<Box<dyn JoinHandle>>,
    connection_tasks: Arc<DashMap<u32, Box<dyn JoinHandle>>>,
    connection_task_counts: AtomicU32,
    connection_count: u32,
}

/// A trait used by [`NetworkServer`] to drive a server, this is responsible
/// for generating the futures that carryout the underlying server logic.
#[async_trait]
pub trait NetworkProvider: 'static + Send + Sync {
    /// This is to configure particular protocols
    type NetworkSettings: Send + Sync + Clone;

    /// The type that acts as a combined sender and reciever for a client.
    /// This type needs to be able to be split.
    type Socket: Send;

    /// The read half of the given socket type.
    type ReadHalf: Send;

    /// The write half of the given socket type.
    type WriteHalf: Send;

    /// Info necessary to start a connection, an [`std::net::SocketAddr`] for instance
    type ConnectInfo: Send;

    /// Info necessary to start a connection, an [`std::net::SocketAddr`] for instance
    type AcceptInfo: Send;

    /// The output type of [`accept_loop`]
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

    /// Recieves messages from the client, forwards them to Spicy via a sender.
    async fn recv_loop(
        read_half: Self::ReadHalf,
        messages: Sender<NetworkPacket>,
        settings: Self::NetworkSettings,
    );

    /// Sends messages to the client, receives packages from Spicy via receiver.
    async fn send_loop(
        write_half: Self::WriteHalf,
        messages: Receiver<NetworkPacket>,
        settings: Self::NetworkSettings,
    );

    /// Split the socket into a read and write half, so that the two actions
    /// can be handled concurrently.
    fn split(combined: Self::Socket) -> (Self::ReadHalf, Self::WriteHalf);
}
