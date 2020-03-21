use async_std::task;
use futures::{executor, future, prelude::*};
use libp2p::{
    build_development_transport, identity,
    mdns::{Mdns, MdnsEvent},
    mplex,
    secio::SecioConfig,
    swarm::NetworkBehaviourEventProcess,
    tcp::TcpConfig,
    NetworkBehaviour, PeerId, Swarm, Transport,
};

use libp2p::core::transport::upgrade::Version;
use std::{
    error::Error,
    task::{Context, Poll},
};

use p2pshare::behaviour::TransferBehaviour;
use p2pshare::protocol::{ProtocolEvent, TransferPayload};

#[derive(NetworkBehaviour)]
struct MyBehaviour {
    mdns: Mdns,
    transfer_behaviour: TransferBehaviour,
}

impl NetworkBehaviourEventProcess<MdnsEvent> for MyBehaviour {
    fn inject_event(&mut self, event: MdnsEvent) {
        match event {
            MdnsEvent::Discovered(list) => {
                for (peer, _addr) in list {
                    self.transfer_behaviour.peers.insert(peer);
                }
            }
            MdnsEvent::Expired(list) => {
                for (peer, _addr) in list {
                    println!("Expired: {:?}", peer);
                    self.transfer_behaviour.peers.remove(&peer);
                }
            }
        }
    }
}

impl NetworkBehaviourEventProcess<ProtocolEvent> for MyBehaviour {
    fn inject_event(&mut self, event: ProtocolEvent) {
        println!("Got event in peer: {:?}", event);
        match event {
            ProtocolEvent::Received { name, path } => println!("Data: {} {}", name, path),
            ProtocolEvent::Sent => println!("sent!"),
        }
    }
}

impl NetworkBehaviourEventProcess<String> for MyBehaviour {
    fn inject_event(&mut self, event: String) {
        println!("String event: {:?}", event);
    }
}

impl NetworkBehaviourEventProcess<TransferPayload> for MyBehaviour {
    fn inject_event(&mut self, event: TransferPayload) {
        println!("TransferPayload event: {:?}", event);
    }
}

async fn execute_swarm() {
    let local_keys = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_keys.public());
    println!("I am Peer: {:?}", local_peer_id);

    let mut swarm = {
        let mdns = Mdns::new().unwrap();
        let transfer_behaviour = TransferBehaviour::new();
        let mplex = mplex::MplexConfig::new();

        let behaviour = MyBehaviour {
            mdns,
            transfer_behaviour,
        };
        let transport = build_development_transport(local_keys.clone());
        let transport = TcpConfig::new()
            .upgrade(Version::V1)
            .authenticate(SecioConfig::new(local_keys.clone()))
            .multiplex(mplex);
        Swarm::new(transport, behaviour, local_peer_id)
    };

    Swarm::listen_on(&mut swarm, "/ip4/0.0.0.0/tcp/0".parse().unwrap()).unwrap();
    let mut listening = false;
    task::block_on(future::poll_fn(move |context: &mut Context| loop {
        match swarm.poll_next_unpin(context) {
            Poll::Ready(Some(event)) => {
                println!("Event, hello: {:?}", event);
            }
            Poll::Ready(None) => return Poll::Ready(()),
            Poll::Pending => {
                if !listening {
                    for addr in Swarm::listeners(&swarm) {
                        println!("{:?}", addr);
                        listening = true;
                    }
                }
                return Poll::Pending;
            }
        }
    }));
}

fn main() -> Result<(), Box<dyn Error>> {
    let future = execute_swarm();
    executor::block_on(future);
    Ok(())
}
