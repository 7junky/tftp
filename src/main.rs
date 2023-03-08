use std::io;
use std::net::UdpSocket;

use tftp::packet::Packet;

fn main() -> io::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:69")?;

    let mut buf = [0; 1024];
    loop {
        let (len, addr) = socket.recv_from(&mut buf)?;
        println!("Received {} bytes", len);

        let packet = Packet::deserialize(&buf).expect("valid packet");

        match packet {
            Packet::Request {
                op_code,
                file_name,
                mode: _,
            } => {
                let res = Packet::Ack { block: 0 };
                let res = res.serialize();

                socket.send_to(&res, addr)?;
                println!("Sent {} bytes", len);
            }
            Packet::Data { block, data, len } => todo!(),
            Packet::Ack { block } => todo!(),
            Packet::Error {
                error_code,
                error_msg,
            } => todo!(),
        }
    }
}
