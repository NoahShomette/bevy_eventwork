use bevy::{prelude::Resource, utils::hashbrown::HashMap};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::NetworkPacket;

/// Component that holds serialization and deserialization functions for the given type T and DataType
#[derive(Resource)]
pub struct MessageSerdeFns<T> {
    /// Deserializes [`NetworkSerializedData`] into T
    pub de_fn: fn(data: &NetworkSerializedData) -> Result<T, String>,
    /// Deserializes T into [`NetworkSerializedData`]
    pub ser_fn: fn(data: &T) -> Result<NetworkSerializedData, String>,
}

/// The different core data types supported over the network
#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum NetworkDataTypes {
    /// Text data
    Text,
    /// Binary data
    Binary,
}

/// An instance of untyped data that is either being sent or recieved over the network.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum NetworkSerializedData {
    /// Text based data
    Text(String),
    /// Binary based data
    Binary(Vec<u8>),
}

/// Resource which holds functions used to deserialize and serialize packets going into and out of the network.
///
/// To override the default bincode based functions insert this resource into your app and replace the [`NetworkDataTypes::Binary`] functions in the hashmap.
#[derive(Resource)]
pub struct NetworkPacketSerdeFns {
    /// Deserializes [`NetworkSerializedData`] into [`NetworkPacket`]s to send into the app
    pub network_packet_de:
        HashMap<NetworkDataTypes, fn(data: NetworkSerializedData) -> Result<NetworkPacket, String>>,
    /// Deserializes [`NetworkPacket`] into [`NetworkSerializedData`]s to send into the network to another app
    pub network_packet_ser:
        HashMap<String, fn(data: NetworkPacket) -> Result<NetworkSerializedData, String>>,
}

impl Default for NetworkPacketSerdeFns {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkPacketSerdeFns {
    /// Creates a new [`Self`] with the given set of serde fns for the given [`NetworkDataTypes`]
    pub fn new() -> Self {
        Self {
            network_packet_de: HashMap::default(),
            network_packet_ser: HashMap::default(),
        }
    }

    /// Insert a serialization method for the given [`NetworkDataTypes`]
    pub fn insert_serialization_method(
        &mut self,
        message_name: &'static str,
        data_type: NetworkDataTypes,
        de_fn: fn(data: NetworkSerializedData) -> Result<NetworkPacket, String>,
        ser_fn: fn(data: NetworkPacket) -> Result<NetworkSerializedData, String>,
    ) {
        self.network_packet_de.insert(data_type, de_fn);
        self.network_packet_ser
            .insert(message_name.to_string(), ser_fn);
    }
}

/// Default bincode based deserialization fn. Only supports binary data types.
pub fn bincode_de<T: DeserializeOwned>(data: &NetworkSerializedData) -> Result<T, String> {
    let NetworkSerializedData::Binary(binary) = data else {
        return Err("Expected Binary data found String data".to_string());
    };
    bincode::deserialize(binary).map_err(|err| err.to_string())
}

/// Default bincode based serialization fn. Only supports binary data types.
pub fn bincode_ser<T: Serialize>(data: &T) -> Result<NetworkSerializedData, String> {
    bincode::serialize(data)
        .map_err(|err| err.to_string())
        .map(|data| NetworkSerializedData::Binary(data))
}

/// Default bincode based [`NetworkPacket`] deserialization fn. Only supports binary data types.
pub fn bincode_network_packet_de(data: NetworkSerializedData) -> Result<NetworkPacket, String> {
    let NetworkSerializedData::Binary(binary) = data else {
        return Err("Expected Binary data found String data".to_string());
    };
    bincode::deserialize(&binary).map_err(|err| err.to_string())
}

/// Default bincode based [`NetworkPacket`] serialization fn. Only supports binary data types.
pub fn bincode_network_packet_ser(data: NetworkPacket) -> Result<NetworkSerializedData, String> {
    bincode::serialize(&data)
        .map_err(|err| err.to_string())
        .map(|data| NetworkSerializedData::Binary(data))
}
