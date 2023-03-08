use std::collections::HashMap;
use std::fs;
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;

use tftp::packet::{Packet, READ_OPCODE, WRITE_OPCODE};

fn main() -> io::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:69")?;
    let socket = Arc::new(socket);
    let mut connections: HashMap<SocketAddr, Sender<Packet>> = HashMap::new();

    let mut buf = [0; 1024];
    loop {
        let (len, addr) = socket.recv_from(&mut buf)?;
        println!("Received {} bytes", len);

        let packet = Packet::deserialize(&buf).expect("valid packet");

        match packet {
            // Create processes for these:
            Packet::Request {
                op_code,
                file,
                mode: _,
            } => {
                let (tx, rx) = mpsc::channel();
                connections.insert(addr, tx);

                let socket = socket.clone();
                let file = file.to_owned();

                if op_code == READ_OPCODE {
                    thread::spawn(move || read_process(socket, addr, rx, file));
                } else if op_code == WRITE_OPCODE {
                    thread::spawn(move || write_process(socket, addr, rx, file));
                } else {
                    panic!("Request op_code is neither 1 or 2");
                }
            }

            // These will be sent to processes:
            // Packet::Data { block, data, len } => todo!(),
            // Packet::Ack { block } => todo!(),
            // Packet::Error {
            //     error_code,
            //     error_msg,
            // } => todo!(),
            packet => {
                if let Some(tx) = connections.get(&addr) {
                    tx.send(packet).expect("send packet to process");
                }
            }
        }
    }
}

// RRQ and DATA packets are awknowledged by ACK and ERROR packets
fn read_process(socket: Arc<UdpSocket>, dst: SocketAddr, rx: Receiver<Packet>, file: String) {
    let res = Packet::Data {
        block: todo!(),
        data: todo!(),
        len: todo!(),
    };
    let res = res.serialize();

    // TODO: handle error
    socket.send_to(&res, dst).expect("send first ack");

    while let Ok(e) = rx.recv() {
        //
    }
}

// WRQ and ACK packets are awknowledged by DATA and ERROR packets
fn write_process(socket: Arc<UdpSocket>, dst: SocketAddr, rx: Receiver<Packet>, file: String) {
    // Send first ack
    let res = Packet::Ack { block: 0 };
    let res = res.serialize();

    // TODO: handle error
    let len = socket.send_to(&res, dst).expect("send first ack");
    println!("Sent {} bytes", len);

    todo!()
}
