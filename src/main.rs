use std::collections::HashMap;
use std::fs;
use std::io::{self, BufWriter, Cursor, Read, Seek, SeekFrom, Write};
use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;

use tftp::packet::{Packet, FILE_NOT_FOUND, READ_OPCODE, SEE_MSG, WRITE_OPCODE};

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

                if op_code == READ_OPCODE {
                    thread::spawn(move || read_process(socket, addr, rx, file));
                } else if op_code == WRITE_OPCODE {
                    thread::spawn(move || write_process(socket, addr, rx, file));
                } else {
                    panic!("Request op_code is neither 1 or 2");
                }
            }

            // Sent to processes: Data, Ack, Error
            packet => {
                if let Some(tx) = connections.get(&addr) {
                    tx.send(packet).expect("send packet to process");
                } else {
                    // TODO: send error
                }
            }
        }
    }
}

/// Initial Connection Protocol for reading a file
/// 1. Host  A  sends  a  "RRQ"  to  host  B  with  source= A's TID,
///    destination= 69.
/// 2. Host B sends a "DATA" (with block number= 1) to host  A  with
///    source= B's TID, destination= A's TID.
///
/// RRQ and ACK packets are awknowledged by DATA and ERROR packets
fn read_process(socket: Arc<UdpSocket>, dst: SocketAddr, rx: Receiver<Packet>, file: String) {
    let file = match fs::File::open(file) {
        Ok(f) => f,
        Err(e) => {
            // TODO: handle error
            eprintln!("Error: {}", e);
            socket
                .send_to(
                    Packet::new_error(FILE_NOT_FOUND, "").serialize().as_slice(),
                    dst,
                )
                .expect("send error");
            return;
        }
    };

    // TODO: handle error
    let mut cursor = Cursor::new(file);
    let mut start = cursor.position() as usize;
    let end = cursor.get_ref().seek(SeekFrom::End(0)).expect("end") as usize;

    let mut data = [0; 512];
    let mut current_block = 1;

    'transfer: while start < end {
        // TODO: handle error
        // Read file into buffer
        let len = cursor.get_ref().read(&mut data).expect("read to buf");

        // Send data
        let res = Packet::new_data(current_block, data, len).serialize();
        // TODO: handle error
        socket.send_to(&res, dst).expect("send data");

        // Wait for ACK (timeout?)
        'recv: while let Ok(e) = rx.recv() {
            match e {
                Packet::Data {
                    block: _,
                    data: _,
                    len: _,
                } => {
                    // Since this is a read request we're not expecting data packets
                    // from the client
                    continue;
                }
                Packet::Ack { block } => {
                    // Need to make sure this block matches what we sent
                    // Else keep waiting
                    if block == current_block {
                        current_block += 1;
                        break 'recv;
                    }
                }
                Packet::Error { code, msg } => {
                    eprintln!("Error {}: {}", code, msg);
                    // End transfer?
                    break 'transfer;
                }
                _ => unreachable!(),
            }
        }

        start += len;
        cursor.set_position(len as u64);
    }
}

/// Initial Connection Protocol for writing a file
/// 1. Host A sends  a  "WRQ"  to  host  B  with  source=  A's  TID,
///    destination= 69.
/// 2. Host  B  sends  a "ACK" (with block number= 0) to host A with
///    source= B's TID, destination= A's TID.
///
/// WRQ and DATA packets are awknowledged by ACK and ERROR packets
fn write_process(socket: Arc<UdpSocket>, dst: SocketAddr, rx: Receiver<Packet>, file: String) {
    let file = match fs::File::create(file) {
        Ok(f) => f,
        Err(e) => {
            // TODO: handle error
            eprintln!("Error: {}", e);
            socket
                .send_to(
                    Packet::new_error(SEE_MSG, "There was an error creating/accessing the file")
                        .serialize()
                        .as_slice(),
                    dst,
                )
                .expect("send error");
            return;
        }
    };

    let mut writer = BufWriter::new(file);

    // Send ack
    let mut current_block = 0;
    let res = Packet::new_ack(current_block).serialize();

    // TODO: handle error
    socket.send_to(&res, dst).expect("send first ack");
    current_block += 1;

    'recv: while let Ok(e) = rx.recv() {
        match e {
            Packet::Data { block, data, len } => {
                // Write to file
                if block != current_block {
                    continue;
                }

                // TODO: handle error
                writer.write_all(&data).expect("write file");

                socket
                    .send_to(Packet::new_ack(current_block).serialize().as_slice(), dst)
                    .expect("send ack");

                current_block += 1;

                if len < 512 {
                    break 'recv;
                }
            }
            Packet::Ack { block: _ } => {
                // Since this is a write request we're not expecting ack packets
                // from the client
                continue;
            }
            Packet::Error { code, msg } => {
                eprintln!("Error {}: {}", code, msg);
                break 'recv;
            }
            _ => unreachable!(),
        }
    }
}
