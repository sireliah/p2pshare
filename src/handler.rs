
// use libp2p::core::upgrade::{ReadOneError};
// use libp2p::swarm::ProtocolsHandler;

// pub struct TransferPayloadHandler {
//     events: Vec<String>,
// }

// impl TransferPayloadHandler {
//     fn new() -> Self {
//         TransferPayloadHandler{events: vec![]}
//     }
// }

// impl ProtocolsHandler for TransferPayloadHandler {
//     type InEvent = ();
//     type OutEvent = String;
//     type Error = ReadOneError;
//     type InboundProtocol = ();
//     type OutboundProtocol = ();
//     type OutboundOpenInfo = ();
// }