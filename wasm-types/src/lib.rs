#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::inline_always)]

mod bounded_str;
mod sender;

pub use bounded_str::BoundedString;
use parity_scale_codec::{Decode, Encode};
pub use sender::Sender;

#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq)]
pub struct Message {
    pub sender: Sender,
    pub message: BoundedString,
}

#[cfg(test)]
mod tests {
    use parity_scale_codec::{DecodeAll, Encode};

    use super::{BoundedString, Message, Sender};

    #[test]
    fn encode_decode_works() {
        let message =
            Message { sender: Sender::Wasm, message: BoundedString::from("some message") };
        let encoded = message.encode();
        let decoded = Message::decode_all(&mut encoded.as_ref()).unwrap();
        assert_eq!(message, decoded);
    }
}
