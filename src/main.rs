use std::io;
use std::net::UdpSocket;

fn main() -> io::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:69")?;

    let mut buf = [0; 1024];
    loop {
        let (len, addr) = socket.recv_from(&mut buf)?;
        println!("Received {} bytes", len);

        socket.send_to(&buf[..len], addr)?;
        println!("Sent {} bytes", len);
    }
}
