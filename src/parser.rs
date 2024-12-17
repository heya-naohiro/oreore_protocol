use bytes::BytesMut;
use bytes::{Buf, Bytes};
use std::error;
use std::fmt;

type Result<T> = std::result::Result<T, OreErrorInsufficient>;

#[derive(Debug, Clone)]
pub struct OreErrorInsufficient;

impl fmt::Display for OreErrorInsufficient {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "insufficient bufffer")
    }
}

impl error::Error for OreErrorInsufficient {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None
    }
}
#[derive(Default, Debug)]
pub enum ProtocolState {
    #[default]
    WaitHeader,
    WaitPayload,
}

#[derive(Default, Debug)]
pub struct OreProtocol {
    payload_size: u16,
    pub state: ProtocolState,
    pub payload: Option<BytesMut>,
}

impl OreProtocol {
    pub fn new() -> Self {
        OreProtocol {
            payload_size: 0,
            state: ProtocolState::WaitHeader,
            payload: None,
        }
    }

    pub fn parse_fixed_header(&mut self, buf: &mut BytesMut) -> Result<()> {
        if buf.len() < 2 {
            return Err(OreErrorInsufficient);
        }
        self.payload_size = ((buf[0] as u16) << 8) + buf[1] as u16;
        self.state = ProtocolState::WaitPayload;
        // 前にすすめる
        buf.advance(2);
        Ok(())
    }

    pub fn parse_payload(&mut self, buf: &mut BytesMut) -> Result<()> {
        if buf.len() < self.payload_size as usize {
            return Err(OreErrorInsufficient);
        }
        // buf.split_toで消費するのでadvanceする必要がない
        self.payload = Some(buf.split_to(self.payload_size.into()));
        Ok(())
    }
}
