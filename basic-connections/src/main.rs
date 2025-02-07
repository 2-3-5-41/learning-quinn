use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use prost::Message;
use quinn::{ClientConfig, Endpoint, ServerConfig};
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};

const SERVER_ADDRESS: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 23541);
const CLIENT_ADDRESS: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 23542);

mod learning {
    pub mod plaintext {
        include!(concat!(env!("OUT_DIR"), "/learning.plaintext.rs"));
    }
}

#[tokio::main]
async fn main() {
    // server self-signed certificate
    let serve_cert = rcgen::generate_simple_self_signed(vec!["helloworld".to_string()]).unwrap();

    // .der
    let serve_der = CertificateDer::from(serve_cert.cert);

    // server private key
    let serve_key = PrivatePkcs8KeyDer::from(serve_cert.key_pair.serialize_der());

    // server config
    let serve_conf =
        ServerConfig::with_single_cert(vec![serve_der.clone()], serve_key.into()).unwrap();

    let serve_endpoint = Endpoint::server(serve_conf, SERVER_ADDRESS).unwrap();

    // Server Task
    let server_task = tokio::spawn(async move {
        // incoming server connection
        let incoming = serve_endpoint.accept().await.unwrap();

        // accepted connection
        let accepted = incoming.await.unwrap();

        println!("Server accepted connection: {}", accepted.remote_address());

        // Our encoded protobuf message.
        let encoded: &mut Vec<u8> = &mut Vec::new();

        // Our protobuf message to send.
        let proto = learning::plaintext::Text {
            message: String::from("This is a protobuf message!"),
        };

        match proto.encode(encoded) {
            Ok(_) => println!("Encoded protobuf message!"),
            Err(e) => println!("Failed to encode protobuf message: {:?}", e),
        }

        let mut interval = tokio::time::interval(Duration::from_millis(100));

        while let Ok(mut stream) = accepted.open_uni().await {
            let write = stream.write(encoded).await.unwrap();
            println!("Server wrote {} bytes to stream", write);
            interval.tick().await;
        }
    });

    // client trusted certs
    let mut trusted_certs = rustls::RootCertStore::empty();
    trusted_certs.add(serve_der).unwrap();

    // client config
    let client_conf = ClientConfig::with_root_certificates(Arc::new(trusted_certs)).unwrap();

    // client endpoint
    let mut client_endpoint = Endpoint::client(CLIENT_ADDRESS).unwrap();
    client_endpoint.set_default_client_config(client_conf);

    // client task
    let client_task = tokio::spawn(async move {
        match client_endpoint.connect(SERVER_ADDRESS, "helloworld") {
            Ok(connecting) => match connecting.await {
                Ok(connection) => {
                    println!("Client connected to: {}", connection.remote_address());

                    while let Ok(mut stream) = connection.accept_uni().await {
                        match stream.read_to_end(64).await {
                            Ok(read) => match learning::plaintext::Text::decode(read.as_slice()) {
                                Ok(decoded) => println!("Decoded protobuf message: {:?}", decoded),
                                Err(e) => println!("Failed to decode protobuf message: {:?}", e),
                            },
                            Err(e) => println!("Stream read error: {:?}", e),
                        };
                    }
                }
                Err(e) => return println!("Client Connection Error: {:?}", e),
            },
            Err(e) => return println!("Client Connection Error: {:?}", e),
        }

        client_endpoint.wait_idle().await;
    });

    let (server, client) = tokio::join!(server_task, client_task);

    match server {
        Ok(_) => (),
        Err(e) => println!("Server task join error: {:?}", e),
    }

    match client {
        Ok(_) => (),
        Err(e) => println!("Client task join error: {:?}", e),
    }
}
