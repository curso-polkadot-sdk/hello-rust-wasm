use core::{
    clone::Clone,
    cmp::{Eq, PartialEq},
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    hash::Hash,
};
use parity_scale_codec::{Decode, Encode, Error, Input};

/// Inline String up to 128 chars long.
#[repr(transparent)]
#[derive(Clone)]
pub struct BoundedString {
    /// The length is the first element of the array, can't move
    /// it to another attribute, because `#[repr(transparent)]`
    /// requires an struct to have just one sized element.
    chars: [u8; 128],
}

impl BoundedString {
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn try_from_str(s: &str) -> Option<Self> {
        let src = s.as_bytes();
        let len = src.len();
        if len > 127 {
            return None;
        }
        let mut chars = [0u8; 128];
        chars[0] = len as u8;
        // SAFETY: `self` is valid for `self.len()` elements by definition, and `src` was
        // checked to have the same length. The slices cannot overlap because
        // mutable references are exclusive.
        unsafe {
            let src = src.as_ptr();
            let dest = chars.as_mut_ptr().add(1);
            core::ptr::copy_nonoverlapping(src, dest, len);
        }
        Some(Self { chars })
    }

    #[must_use]
    #[inline(always)]
    pub const fn len(&self) -> usize {
        self.chars[0] as usize
    }

    #[must_use]
    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.chars[0] == 0
    }

    #[must_use]
    #[inline(always)]
    pub const fn as_bytes(&self) -> &[u8] {
        unsafe {
            let len = self.chars[0] as usize;
            let ptr = self.chars.as_ptr().add(1);
            core::slice::from_raw_parts(ptr, len)
        }
    }

    #[must_use]
    #[inline(always)]
    pub const fn as_mut_str(&mut self) -> &mut str {
        unsafe {
            let ptr = self.chars.as_mut_ptr().add(1);
            let len = self.chars[0] as usize;
            let bytes = core::slice::from_raw_parts_mut(ptr, len);
            str::from_utf8_unchecked_mut(bytes)
        }
    }

    #[must_use]
    #[inline(always)]
    pub const fn as_str(&self) -> &str {
        let bytes = self.as_bytes();
        unsafe { str::from_utf8_unchecked(bytes) }
    }
}

impl From<&'_ str> for BoundedString {
    #[allow(clippy::cast_possible_truncation)]
    fn from(s: &'_ str) -> Self {
        let mut chars = [0u8; 128];
        let (len, mut buf) = unsafe {
            // Safety: chars.len() > 0
            chars.split_first_mut().unwrap_unchecked()
        };
        for c in s.chars() {
            let Some((dest, rest)) = buf.split_at_mut_checked(c.len_utf8()) else {
                break;
            };
            c.encode_utf8(dest);
            *len += dest.len() as u8;
            buf = rest;
        }
        Self { chars }
    }
}

impl Display for BoundedString {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        <str as Display>::fmt(self.as_str(), f)
    }
}

impl Debug for BoundedString {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        <str as Debug>::fmt(self.as_str(), f)
    }
}

impl PartialEq<Self> for BoundedString {
    fn eq(&self, other: &Self) -> bool {
        let a = self.as_str();
        let b = other.as_str();
        <str as PartialEq>::eq(a, b)
    }
}

impl PartialEq<str> for BoundedString {
    fn eq(&self, other: &str) -> bool {
        <str as PartialEq>::eq(self.as_str(), other)
    }
}

impl Eq for BoundedString {}

impl Hash for BoundedString {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        <str as Hash>::hash::<H>(self.as_str(), state);
    }
}

impl AsRef<str> for BoundedString {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl AsMut<str> for BoundedString {
    fn as_mut(&mut self) -> &mut str {
        self.as_mut_str()
    }
}

impl Encode for BoundedString {
    #[inline(always)]
    fn size_hint(&self) -> usize {
        self.chars.len()
    }

    #[inline]
    fn encode_to<T: parity_scale_codec::Output + ?Sized>(&self, dest: &mut T) {
        let len = self.len();
        let bytes = &self.chars[..=len];
        dest.write(bytes);
    }

    #[inline]
    fn using_encoded<R, F: FnOnce(&[u8]) -> R>(&self, f: F) -> R {
        let len = self.len();
        let bytes = &self.chars[..=len];
        f(bytes)
    }

    #[inline]
    fn encoded_size(&self) -> usize {
        self.chars[0] as usize + 1
    }
}

impl Decode for BoundedString {
    fn decode<I: Input>(input: &mut I) -> Result<Self, Error> {
        let len = input.read_byte()?;
        let mut chars = [0u8; 128];
        let Some((length, bytes)) = chars.split_first_mut() else {
            unreachable!("qed; chars.length > 0")
        };
        let Some((bytes, _)) = bytes.split_at_mut_checked(len as usize) else {
            return Err(Error::from("string out of bounds"));
        };
        input.read(bytes)?;
        str::from_utf8(bytes).map_err(|_| Error::from("invalid utf8 string"))?;
        *length = len;
        Ok(Self { chars })
    }
}

#[cfg(test)]
mod tests {
    use super::BoundedString;

    #[test]
    fn it_works() {
        let tests = ["", "hello", "hello world", "a"];
        for test in tests {
            let s = BoundedString::try_from_str(test).unwrap();
            assert_eq!(s.as_str(), test);
            assert_eq!(format!("{s}"), format!("{test}"));
            assert_eq!(format!("{s:?}"), format!("{test:?}"));
        }
    }
}
