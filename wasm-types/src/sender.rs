use core::fmt::{Display, Formatter, Result as FmtResult};
use parity_scale_codec::{Decode, Encode};

/// Tipo que identifica a origem da mensagem.
#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq)]
pub enum Sender {
    Host,
    Wasm,
}

impl Display for Sender {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Host => f.write_str("Host"),
            Self::Wasm => f.write_str("Wasm"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Sender;
    use parity_scale_codec::{DecodeAll, Encode};

    #[test]
    fn encode_decode_works() {
        let tests = [(Sender::Host, "Host"), (Sender::Wasm, "Wasm")];
        for (sender, display) in tests {
            let encoded = sender.encode();
            let decoded = Sender::decode_all(&mut encoded.as_ref()).unwrap();
            assert_eq!(decoded, sender);
            assert_eq!(format!("{sender}"), display);
        }
    }
}
