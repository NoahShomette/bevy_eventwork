use bevy::prelude::Component;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::NetworkPacket;

/// Component that holds serialization and deserialization functions for the given type T and DataType
#[derive(Component)]
pub struct ComponentSerdeFns<T> {
    /// Deserializes [`NetworkSerializedData`] into T
    pub de_fn: fn(data: &NetworkSerializedData) -> Result<T, String>,
    /// Deserializes T into [`NetworkSerializedData`]
    pub ser_fn: fn(data: &T) -> Result<NetworkSerializedData, String>,
}

/// The different core data types supported over the network
pub enum NetworkDataTypes {
    /// String data
    String,
    /// Binary data
    Binary,
}

/// An instance of untyped data that is either being sent or recieved over the network.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum NetworkSerializedData {
    /// String based data
    String(String),
    /// Binary based data
    Binary(Vec<u8>),
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
