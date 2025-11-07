#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::inline_always)]

mod tiny_str;

use core::fmt::{Display, Formatter, Result as FmtResult};
use parity_scale_codec::{Decode, Encode};
pub use tiny_str::BoundedString;

#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq)]
pub enum Kind {
    Ping,
    Pong,
}

impl Display for Kind {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Ping => f.write_str("ping"),
            Self::Pong => f.write_str("pong"),
        }
    }
}

#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq)]
pub struct Message {
    pub kind: Kind,
    pub message: BoundedString,
}

#[cfg(test)]
mod tests {
    use parity_scale_codec::{DecodeAll, Encode};

    use super::{BoundedString, Kind, Message};

    #[test]
    fn encode_decode_works() {
        let message = Message { kind: Kind::Ping, message: BoundedString::from("some message") };
        let encoded = message.encode();
        let decoded = Message::decode_all(&mut encoded.as_ref()).unwrap();
        assert_eq!(message, decoded);
    }
}
