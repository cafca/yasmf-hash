//! yamf-hash
//!
//! Encode and decode [yamf-hashes](https://github.com/AljoschaMeyer/yamf-hash)
//!
#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate static_assertions;

use crate::error::*;
#[cfg(feature = "std")]
mod util;
pub mod error;

#[cfg(feature = "std")]
use crate::util::hex_serde::{hex_from_bytes, vec_from_hex};
use arrayvec::ArrayVec;
pub use blake3::{hash as blake3, Hash as Blake3Hash, OUT_LEN as BLAKE3_OUT_LEN};
use core::borrow::Borrow;
use core::iter::FromIterator;

#[cfg(feature = "std")]
use std::io::Write;

use varu64::{decode as varu64_decode, encode as varu64_encode, encoding_length};

pub const BLAKE3_HASH_SIZE: usize = 32;
// This is a way to hard code a value that cbindgen can use, but make sure at compile time
// that the value is actually correct.
const_assert_eq!(blake3_hash_size; BLAKE3_HASH_SIZE, BLAKE3_OUT_LEN);

pub const BLAKE3_NUMERIC_ID: u64 = 0;

/// The maximum number of bytes this will use for any variant.
///
/// This is a bit yuck because it knows the number of bytes varu64 uses to encode the
/// Blake3 hash size and the blake3 numeric id (2).
/// This is unlikely to cause a problem until there are hundreds of variants.
pub const MAX_YAMF_HASH_SIZE: usize = BLAKE3_HASH_SIZE + 2;

/// Variants of `YamfHash`
#[derive(Deserialize, Serialize, Debug, Eq)]
pub enum YamfHash<T: Borrow<[u8]>> {
    #[cfg_attr(
        feature = "std",
        serde(serialize_with = "hex_from_bytes", deserialize_with = "vec_from_hex")
    )]
    #[cfg_attr(feature = "std", serde(bound(deserialize = "T: From<Vec<u8>>")))]
    Blake3(T),
}

impl<B1: Borrow<[u8]>, B2: Borrow<[u8]>> PartialEq<YamfHash<B1>> for YamfHash<B2> {
    fn eq(&self, other: &YamfHash<B1>) -> bool {
        match (self, other) {
            (YamfHash::Blake3(vec), YamfHash::Blake3(vec2)) => vec.borrow() == vec2.borrow(),
        }
    }
}

pub fn new_blake3(bytes: &[u8]) -> YamfHash<ArrayVec<[u8; BLAKE3_HASH_SIZE]>> {
    let hash_bytes = blake3(bytes);

    let vec_bytes: ArrayVec<[u8; BLAKE3_HASH_SIZE]> =
        ArrayVec::from_iter(hash_bytes.as_bytes().iter().map(|b| *b));

    YamfHash::Blake3(vec_bytes)
}

impl<'a> From<&'a YamfHash<ArrayVec<[u8; BLAKE3_HASH_SIZE]>>> for YamfHash<&'a [u8]> {
    fn from(hash: &YamfHash<ArrayVec<[u8; BLAKE3_HASH_SIZE]>>) -> YamfHash<&[u8]> {
        match hash {
            YamfHash::Blake3(bytes) => YamfHash::Blake3(&bytes[..]),
        }
    }
}


impl<'a> From<Blake3Hash> for YamfHash<ArrayVec<[u8; BLAKE3_HASH_SIZE]>> {
    fn from(hash: Blake3Hash) -> Self {
        let vec_bytes: ArrayVec<[u8; BLAKE3_HASH_SIZE]> =
            ArrayVec::from_iter(hash.as_bytes().iter().map(|b| *b));

        YamfHash::Blake3(vec_bytes)
    }
}
impl<T: Borrow<[u8]>> YamfHash<T> {
    /// Encode a YamfHash into the out buffer.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, Error> {
        let encoded_size = self.encoding_length();

        match (self, out.len()) {
            (YamfHash::Blake3(vec), len) if len >= encoded_size => {
                varu64_encode(BLAKE3_NUMERIC_ID, &mut out[0..1]);
                varu64_encode(BLAKE3_HASH_SIZE as u64, &mut out[1..2]);
                out[2..encoded_size].copy_from_slice(vec.borrow());
                Ok(encoded_size)
            }
            _ => Err(Error::EncodeError),
        }
    }

    pub fn encoding_length(&self) -> usize {
        match self {
            YamfHash::Blake3(_) => {
                encoding_length(BLAKE3_NUMERIC_ID)
                    + encoding_length(BLAKE3_HASH_SIZE as u64)
                    + BLAKE3_HASH_SIZE
            }
        }
    }

    /// Decode the `bytes` as a `YamfHash`
    pub fn decode<'a>(bytes: &'a [u8]) -> Result<(YamfHash<&'a [u8]>, &'a [u8]), Error> {
        match varu64_decode(&bytes) {
            Ok((BLAKE3_NUMERIC_ID, remaining_bytes)) if remaining_bytes.len() >= 33 => {
                let hash = &remaining_bytes[1..33];
                Ok((YamfHash::Blake3(hash), &remaining_bytes[33..]))
            }
            Err((_, _)) => Err(Error::DecodeVaru64Error),
            _ => Err(Error::DecodeError {}),
        }
    }

    /// Decode the `bytes` as a `YamfHash`
    pub fn decode_owned<'a>(
        bytes: &'a [u8],
    ) -> Result<(YamfHash<ArrayVec<[u8; BLAKE3_HASH_SIZE]>>, &'a [u8]), Error> {
        match varu64_decode(&bytes) {
            Ok((BLAKE3_NUMERIC_ID, remaining_bytes)) if remaining_bytes.len() >= 33 => {
                let mut vec = ArrayVec::new();
                let slice = &remaining_bytes[1..33];
                vec.try_extend_from_slice(slice).unwrap();
                Ok((YamfHash::Blake3(vec), &remaining_bytes[33..]))
            }
            Err((_, _)) => Err(Error::DecodeVaru64Error),
            _ => Err(Error::DecodeError {}),
        }
    }

    /// Encode a YamfHash into the writer.
    #[cfg(feature = "std")]
    pub fn encode_write<W: Write>(&self, mut w: W) -> Result<(), Error> {
        let mut out = [0; 2];
        match self {
            YamfHash::Blake3(vec) => {
                varu64_encode(BLAKE3_NUMERIC_ID, &mut out[0..1]);
                varu64_encode(BLAKE3_HASH_SIZE as u64, &mut out[1..2]);
                w.write_all(&out).map_err(|_| Error::EncodeWriteError)?;
                w.write_all(vec.borrow())
                    .map_err(|_| Error::EncodeWriteError)?;
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::MAX_YAMF_HASH_SIZE;

    use super::{new_blake3, blake3, Error, YamfHash, BLAKE3_HASH_SIZE};
    use arrayvec::ArrayVec;
    use core::iter::FromIterator;

    #[test]
    fn encode_yamf() {
        let hash_bytes = vec![0xFF; BLAKE3_HASH_SIZE];
        let yamf_hash = YamfHash::Blake3(hash_bytes);

        let mut encoded = vec![0; MAX_YAMF_HASH_SIZE];
        let length = yamf_hash.encode(&mut encoded).unwrap();
        assert_eq!(length, MAX_YAMF_HASH_SIZE);
        assert_eq!(encoded[0], 0);
        assert_eq!(encoded[1], 32);
    }
    #[test]
    fn encode_yamf_write() {
        let hash_bytes = vec![0xFF; BLAKE3_HASH_SIZE];
        let yamf_hash = YamfHash::Blake3(hash_bytes);

        let mut encoded = Vec::new();
        yamf_hash.encode_write(&mut encoded).unwrap();
        assert_eq!(encoded.len(), 34);
        assert_eq!(encoded[0], 0);
        assert_eq!(encoded[1], 32);
    }
    #[test]
    fn encode_yamf_not_enough_bytes_for_varu() {
        let hash_bytes = vec![0xFF; 4];
        let yamf_hash = YamfHash::Blake3(hash_bytes);

        let mut encoded = [0; 2];
        match yamf_hash.encode_write(&mut encoded[..]) {
            Err(Error::EncodeWriteError) => {}
            _ => panic!("Go ok, expected error"),
        }
    }
    #[test]
    fn encode_yamf_not_enough_bytes_for_hash() {
        let hash_bytes = vec![0xFF; 4];
        let yamf_hash = YamfHash::Blake3(hash_bytes);

        let mut encoded = [0; 4];
        match yamf_hash.encode_write(&mut encoded[..]) {
            Err(Error::EncodeWriteError) => {}
            _ => panic!("Go ok, expected error"),
        }
    }
    #[test]
    fn decode_yamf() {
        let mut hash_bytes = vec![0xFF; 35];
        hash_bytes[0] = 0;
        hash_bytes[1] = 32;
        hash_bytes[34] = 0xAA;
        let result = YamfHash::<&[u8]>::decode(&hash_bytes);

        match result {
            Ok((YamfHash::Blake3(vec), remaining_bytes)) => {
                assert_eq!(vec.len(), 32);
                assert_eq!(vec, &hash_bytes[2..34]);
                assert_eq!(remaining_bytes, &[0xAA]);
            }
            _ => panic!(),
        }
    }
    #[test]
    fn decode_yamf_varu_error() {
        let mut hash_bytes = vec![0xFF; 67];
        hash_bytes[0] = 248;
        hash_bytes[1] = 1;
        hash_bytes[2] = 32;
        hash_bytes[66] = 0xAA;
        let result = YamfHash::<&[u8]>::decode(&hash_bytes);

        match result {
            Err(Error::DecodeVaru64Error) => {}
            _ => panic!(),
        }
    }
    #[test]
    fn decode_yamf_not_enough_bytes_error() {
        let mut hash_bytes = vec![0xFF; BLAKE3_HASH_SIZE];
        hash_bytes[0] = 0;
        hash_bytes[1] = 32;
        let result = YamfHash::<&[u8]>::decode(&hash_bytes);

        match result {
            Err(Error::DecodeError {}) => {}
            _ => panic!(),
        }
    }

    #[test]
    fn blake_yamf_hash() {
        let lam = || {
            let hash_bytes = blake3(&[1, 2]);
            let vec_bytes: ArrayVec<[u8; BLAKE3_HASH_SIZE]> =
                ArrayVec::from_iter(hash_bytes.as_bytes().iter().map(|b| *b));
            YamfHash::Blake3(vec_bytes)
        };
        let _ = lam();
    }

    #[test]
    fn blake2b_yamf_hash_eq() {
        let lam = || {
            let hash_bytes = blake3(&[1, 2]);
            let vec_bytes: ArrayVec<[u8; BLAKE3_HASH_SIZE]> =
                ArrayVec::from_iter(hash_bytes.as_bytes().iter().map(|b| *b));
            YamfHash::Blake3(vec_bytes)
        };
        let result = lam();

        let hash_bytes = blake3(&[1, 2]);
        let result2 = YamfHash::Blake3(&hash_bytes.as_bytes()[..]);

        assert_eq!(result, result2);
        assert_eq!(result2, result);
    }

    #[test]
    fn owned_yamf_hash() {
        let lam = || {
            let mut hash_bytes = ArrayVec::<[u8; BLAKE3_HASH_SIZE]>::new();
            hash_bytes.push(1);
            hash_bytes.push(64);
            YamfHash::Blake3(hash_bytes)
        };
        let _ = lam();
    }
    #[test]
    fn ref_yamf_hash() {
        let mut hash_bytes = ArrayVec::<[u8; BLAKE3_HASH_SIZE * 2]>::new();
        hash_bytes.push(1);
        hash_bytes.push(64);
        YamfHash::Blake3(hash_bytes);
    }
    #[test]
    fn from_owned_to_ref_yamf_hash() {
        let lam = || {
            let mut hash_bytes = ArrayVec::<[u8; BLAKE3_HASH_SIZE]>::new();
            hash_bytes.push(1);
            hash_bytes.push(64);
            YamfHash::Blake3(hash_bytes)
        };
        let result = lam();
        let _: YamfHash<&[u8]> = YamfHash::from(&result);
    }

    #[test]
    fn encode_decode_blake2b() {
        let bytes = vec![1, 2, 3];
        let yamf_hash = new_blake3(&bytes);

        let mut encoded = Vec::new();
        yamf_hash.encode_write(&mut encoded).unwrap();

        let (decoded, _) = YamfHash::<ArrayVec<[u8; BLAKE3_HASH_SIZE]>>::decode_owned(&encoded).unwrap();

        assert_eq!(decoded, yamf_hash);
    }

    #[test]
    fn encode_decode_blake3() {
        let bytes = vec![1, 2, 3];
        let yamf_hash = new_blake3(&bytes);

        let mut encoded = Vec::new();
        yamf_hash.encode_write(&mut encoded).unwrap();

        let (decoded, _) = YamfHash::<ArrayVec<[u8; BLAKE3_HASH_SIZE]>>::decode_owned(&encoded).unwrap();

        assert_eq!(decoded, yamf_hash);
    }
}
