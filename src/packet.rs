use std::io::{BufReader, Cursor, Read};

#[derive(Debug, PartialEq)]
pub enum Mode {
    NetAscii,
    Octet,
    Mail,
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

const READ_OPCODE: u16 = 1;
const WRITE_OPCODE: u16 = 2;
const DATA_OPCODE: u16 = 3;
const ACK_OPCODE: u16 = 4;
const ERROR_OPCODE: u16 = 5;

/// https://www.rfc-editor.org/rfc/rfc1350
pub enum Packet<'a> {
    /// RRQ/WRQ Packet
    ///  2 bytes     string    1 byte     string   1 byte
    ///  ------------------------------------------------
    /// | Opcode |  Filename  |   0  |    Mode    |   0  |
    ///  ------------------------------------------------
    /// Mode can be either "netascii", "octet" or "mail"
    Request {
        op_code: u16,
        file_name: &'a str,
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
        op_code: u16,
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
    Ack { op_code: u16, block: u16 },
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
    Error {
        op_code: u16,
        error_code: u16,
        error_msg: &'a [u8],
    },
}

impl<'a> Packet<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Packet<'a>, Error> {
        // let mut src = Cursor::new(&bytes[..]);
        let op_code = u16::from_be_bytes([bytes[0], bytes[1]]);

        let packet = match op_code {
            READ_OPCODE => parse_rwrq(bytes, op_code)?,
            WRITE_OPCODE => parse_rwrq(bytes, op_code)?,
            DATA_OPCODE => todo!(),
            ACK_OPCODE => todo!(),
            ERROR_OPCODE => todo!(),
            _ => Err(Error::InvalidOpcode)?,
        };

        Ok(packet)
    }
}

fn parse_rwrq(bytes: &[u8], op_code: u16) -> Result<Packet, Error> {
    let mut cursor = Cursor::new(&bytes[2..]);

    let file_name = read_until_zero_byte(&mut cursor)?;
    let file_name = std::str::from_utf8(file_name).unwrap();

    let mode = read_until_zero_byte(&mut cursor)?;
    let mode = std::str::from_utf8(mode).unwrap();
    let mode: Mode = mode.into();

    Ok(Packet::Request {
        op_code,
        file_name,
        mode,
    })
}

fn parse_data(bytes: &[u8]) -> Result<Packet, Error> {
    let block = u16::from_be_bytes([bytes[2], bytes[3]]);

    let mut data = [0; 512];
    let mut reader = BufReader::new(&bytes[4..]);
    // TODO: handle error
    let len = reader.read(&mut data).expect("ok");

    Ok(Packet::Data {
        op_code: DATA_OPCODE,
        block,
        data,
        len,
    })
}

fn parse_ack(bytes: &[u8]) -> Result<Packet, Error> {
    let block = u16::from_be_bytes([bytes[2], bytes[3]]);

    Ok(Packet::Ack {
        op_code: ACK_OPCODE,
        block,
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

    fn test_rwrq(rq: &[u8], exp_op_code: u16, exp_file_name: &str, exp_mode: Mode) {
        let packet = Packet::parse(rq).unwrap();

        match packet {
            Packet::Request {
                op_code,
                file_name,
                mode,
            } => {
                assert_eq!(
                    op_code, exp_op_code,
                    "Expected: {}\nGot: {}",
                    exp_op_code, op_code
                );
                assert_eq!(
                    file_name, exp_file_name,
                    "Expected: {}\nGot: {}",
                    exp_file_name, file_name
                );
                assert_eq!(mode, exp_mode, "Expected: {:?}\nGot: {:?}", exp_mode, mode)
            }
            _ => panic!("did not get expected: PacketRequest"),
        }
    }

    #[test]
    fn test_parse_rrq() {
        // read, main.rs, netascii
        let rrq = &[
            0x00, 0x01, 0x6D, 0x61, 0x69, 0x6E, 0x2E, 0x72, 0x73, 0x00, 0x6E, 0x65, 0x74, 0x61,
            0x73, 0x63, 0x69, 0x69, 0x00, /**/ 0x00,
        ];

        test_rwrq(rrq, READ_OPCODE, "main.rs", Mode::NetAscii);
    }

    #[test]
    fn test_parse_wrq() {
        // read, main.rs, netascii
        let wrq = &[
            0x00, 0x02, 0x6D, 0x61, 0x69, 0x6E, 0x2E, 0x72, 0x73, 0x00, 0x6E, 0x65, 0x74, 0x61,
            0x73, 0x63, 0x69, 0x69, 0x00, /**/ 0x00,
        ];

        test_rwrq(wrq, WRITE_OPCODE, "main.rs", Mode::NetAscii);
    }
}
