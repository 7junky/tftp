use std::io::{BufReader, Cursor, Read};

#[derive(Debug, PartialEq)]
pub enum Mode {
    NetAscii,
    Octet,
    Mail,
}

impl Mode {
    pub fn encode(&self) -> &[u8] {
        todo!()
    }
}

#[derive(Debug)]
pub enum Error {
    InvalidOpcode,
    NoZeroByte,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::InvalidOpcode => write!(f, "invalid opcode"),
            Error::NoZeroByte => write!(f, "couldn't find zero byte"),
        }
    }
}

impl std::error::Error for Error {}

impl From<&str> for Mode {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "netascii" => Mode::NetAscii,
            "octet" => Mode::Octet,
            "mail" => Mode::Mail,
            _ => panic!(),
        }
    }
}

// Opcodes
pub const READ_OPCODE: u16 = 1;
pub const WRITE_OPCODE: u16 = 2;
pub const DATA_OPCODE: u16 = 3;
pub const ACK_OPCODE: u16 = 4;
pub const ERROR_OPCODE: u16 = 5;

// Errors
pub const SEE_MSG: u16 = 0;
pub const FILE_NOT_FOUND: u16 = 1;
pub const ACCESS_VIOLATION: u16 = 2;
pub const DISK_FULL: u16 = 3;
pub const ILLEGAL_OP: u16 = 4;
pub const UNKNOWN_TID: u16 = 5;
pub const FILE_EXISTS: u16 = 6;
pub const NO_USER: u16 = 7;

/// https://www.rfc-editor.org/rfc/rfc1350
pub enum Packet {
    /// RRQ/WRQ Packet
    ///  2 bytes     string    1 byte     string   1 byte
    ///  ------------------------------------------------
    /// | Opcode |  Filename  |   0  |    Mode    |   0  |
    ///  ------------------------------------------------
    /// Mode can be either "netascii", "octet" or "mail"
    Request {
        op_code: u16,
        file: String,
        mode: Mode,
    },
    /// DATA Packet
    ///  2 bytes     2 bytes      n bytes
    ///  ----------------------------------
    /// | Opcode |   Block #  |   Data     |
    ///  ----------------------------------
    /// The block numbers on data packets begin with one and increase by one for
    /// each new block of data.
    Data {
        block: u16,
        data: [u8; 512],

        // If its less than 512 bytes, it's the last data packet
        len: usize,
    },
    /// ACK Packet
    ///  2 bytes     2 bytes
    ///  ---------------------
    /// | Opcode |   Block #  |
    ///  ---------------------
    /// The  block  number  in an  ACK echoes the block number of the DATA packet being
    /// acknowledged.
    Ack { block: u16 },
    /// ERROR Packet
    ///  2 bytes     2 bytes      string    1 byte
    ///  -----------------------------------------
    /// | Opcode |  ErrorCode |   ErrMsg   |   0  |
    ///  -----------------------------------------
    ///  Error Codes:
    ///  0 Not defined, see error message (if any).
    ///  1 File not found.
    ///  2 Access violation.
    ///  3 Disk full or allocation exceeded.
    ///  4 Illegal TFTP operation.
    ///  5 Unknown transfer ID.
    ///  6 File already exists.
    ///  7 No such user.
    Error { code: u16, msg: String },
}

impl Packet {
    pub fn deserialize(bytes: &[u8]) -> Result<Packet, Error> {
        let op_code = u16::from_be_bytes([bytes[0], bytes[1]]);

        let packet = match op_code {
            READ_OPCODE => parse_rwrq(bytes, op_code)?,
            WRITE_OPCODE => parse_rwrq(bytes, op_code)?,
            DATA_OPCODE => parse_data(bytes)?,
            ACK_OPCODE => parse_ack(bytes)?,
            ERROR_OPCODE => parse_error(bytes)?,
            _ => Err(Error::InvalidOpcode)?,
        };

        Ok(packet)
    }

    pub fn serialize(&self) -> Vec<u8> {
        match self {
            Packet::Request {
                op_code,
                file,
                mode,
            } => {
                let mut res: Vec<u8> = Vec::with_capacity(30);

                let op_code = op_code.to_be_bytes();
                res.extend_from_slice(&op_code);

                let file_name = file.as_bytes();
                res.extend_from_slice(file_name);
                res.push(0);

                let mode = mode.encode();
                res.extend_from_slice(mode);
                res.push(0);

                res
            }
            Packet::Data {
                block,
                data,
                len: _,
            } => {
                let mut res: Vec<u8> = Vec::with_capacity(516);

                let op_code = DATA_OPCODE.to_be_bytes();
                res.extend_from_slice(&op_code);

                let block = block.to_be_bytes();
                res.extend_from_slice(&block);

                res.extend_from_slice(data);

                res
            }
            Packet::Ack { block } => {
                let mut res: Vec<u8> = Vec::with_capacity(4);

                let op_code = ACK_OPCODE.to_be_bytes();
                res.extend_from_slice(&op_code);

                let block = block.to_be_bytes();
                res.extend_from_slice(&block);

                res
            }
            Packet::Error { code, msg } => {
                let mut res: Vec<u8> = Vec::with_capacity(30);

                let op_code = ERROR_OPCODE.to_be_bytes();
                res.extend_from_slice(&op_code);

                // The tftp client I have expects the error code to be little endian
                // which isn't mentioned on the RFC
                let code = code.to_le_bytes();
                res.extend_from_slice(&code);

                let msg = msg.as_bytes();
                res.extend_from_slice(&msg);
                res.push(0);

                res
            }
        }
    }

    pub fn new_error(code: u16, msg: &str) -> Self {
        Self::Error {
            code,
            msg: msg.to_owned(),
        }
    }

    pub fn new_data(block: u16, data: [u8; 512], len: usize) -> Self {
        Self::Data { block, data, len }
    }

    pub fn new_ack(block: u16) -> Self {
        Self::Ack { block }
    }
}

fn parse_rwrq(bytes: &[u8], op_code: u16) -> Result<Packet, Error> {
    let mut cursor = Cursor::new(&bytes[2..]);

    let file = read_until_zero_byte(&mut cursor)?;
    let file = std::str::from_utf8(file).unwrap();

    let mode = read_until_zero_byte(&mut cursor)?;
    let mode = std::str::from_utf8(mode).unwrap();
    let mode: Mode = mode.into();

    Ok(Packet::Request {
        op_code,
        file: file.to_owned(),
        mode,
    })
}

fn parse_data(bytes: &[u8]) -> Result<Packet, Error> {
    let block = u16::from_be_bytes([bytes[2], bytes[3]]);

    let mut data = [0; 512];
    let mut reader = BufReader::new(&bytes[4..]);
    // TODO: handle error
    let len = reader.read(&mut data).expect("ok");

    Ok(Packet::Data { block, data, len })
}

fn parse_ack(bytes: &[u8]) -> Result<Packet, Error> {
    let block = u16::from_be_bytes([bytes[2], bytes[3]]);

    Ok(Packet::Ack { block })
}

fn parse_error(bytes: &[u8]) -> Result<Packet, Error> {
    let code = u16::from_le_bytes([bytes[2], bytes[3]]);

    let mut cursor = Cursor::new(&bytes[4..]);

    let msg = read_until_zero_byte(&mut cursor)?;
    let msg = std::str::from_utf8(msg).unwrap();

    Ok(Packet::Error {
        code,
        msg: msg.to_owned(),
    })
}

fn read_until_zero_byte<'a>(cursor: &mut Cursor<&'a [u8]>) -> Result<&'a [u8], Error> {
    let start = cursor.position() as usize;
    let end = cursor.get_ref().len() - 1;

    for i in start..end {
        if cursor.get_ref()[i] == b'\0' {
            cursor.set_position((i + 1) as u64);

            return Ok(&cursor.get_ref()[start..i]);
        }
    }

    Err(Error::NoZeroByte)
}

#[cfg(test)]
mod test {
    use super::{Mode, Packet, READ_OPCODE, WRITE_OPCODE};

    fn test_rwrq(rq: &[u8], exp_op_code: u16, exp_file: &str, exp_mode: Mode) {
        let packet = Packet::deserialize(rq).unwrap();

        match packet {
            Packet::Request {
                op_code,
                file,
                mode,
            } => {
                assert_eq!(
                    op_code, exp_op_code,
                    "Expected: {}\nGot: {}",
                    exp_op_code, op_code
                );
                assert_eq!(file, exp_file, "Expected: {}\nGot: {}", exp_file, file);
                assert_eq!(mode, exp_mode, "Expected: {:?}\nGot: {:?}", exp_mode, mode)
            }
            _ => panic!("did not get expected packet: Request"),
        }
    }

    #[test]
    fn test_parse_rrq() {
        // read, main.rs, netascii
        let rrq = &[
            0x00, 0x01, b'm', b'a', b'i', b'n', b'.', b'r', b's', 0x00, b'n', b'e', b't', b'a',
            b's', b'c', b'i', b'i', 0x00, /**/ 0x00,
        ];

        test_rwrq(rrq, READ_OPCODE, "main.rs", Mode::NetAscii);
    }

    #[test]
    fn test_parse_wrq() {
        // write, main.rs, netascii
        let wrq = &[
            0x00, 0x02, b'm', b'a', b'i', b'n', b'.', b'r', b's', 0x00, b'n', b'e', b't', b'a',
            b's', b'c', b'i', b'i', 0x00, /**/ 0x00,
        ];

        test_rwrq(wrq, WRITE_OPCODE, "main.rs", Mode::NetAscii);
    }

    #[test]
    fn test_parse_data() {
        let data = &[
            0x00, 0x03, 0x00, 0x00, b'h', b'e', b'l', b'l', b'o', b' ', b'w', b'o', b'r', b'l',
            b'd',
        ];

        let packet = Packet::deserialize(data).unwrap();

        match packet {
            Packet::Data { block, data, len } => {
                assert_eq!(block, 0);
                assert_eq!(&data[0..11], b"hello world");
                assert_eq!(len, 11);
            }
            _ => panic!("did not get expected packet: Data"),
        }
    }

    #[test]
    fn test_parse_ack() {
        let data = &[0x00, 0x04, 0x00, 0x00];

        let packet = Packet::deserialize(data).unwrap();

        match packet {
            Packet::Ack { block } => {
                assert_eq!(block, 0);
            }
            _ => panic!("did not get expected packet: Ack"),
        }
    }

    #[test]
    fn test_parse_error() {
        let data = &[
            0x00, 0x05, 0x00, 0x00, b'e', b'r', b'r', b'o', b'r', 0x00, /**/ 0x00,
        ];

        let packet = Packet::deserialize(data).unwrap();

        match packet {
            Packet::Error { code, msg } => {
                assert_eq!(code, 0);
                assert_eq!(msg, "error");
            }
            _ => panic!("did not get expected packet: Error"),
        }
    }
}
