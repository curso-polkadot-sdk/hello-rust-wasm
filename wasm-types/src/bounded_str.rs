use core::{
    clone::Clone,
    cmp::{Eq, PartialEq},
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    hash::Hash,
    str::Chars,
};
use parity_scale_codec::{Decode, Encode, Error, Input};

/// Max number of bytes that fits in a `BoundedString`.
///
/// OBS: must be more than zero and less than 256, one
/// extra byte will be added to hold the string length.
pub const CHAR_LIMIT: usize = 127;

/// Checks that `ch` byte is the first byte in a UTF-8 code point
/// sequence.
#[inline]
#[allow(clippy::cast_possible_wrap)]
const fn is_utf8_char_boundary(ch: u8) -> bool {
    // This is bit magic equivalent to: b < 128 || b >= 192
    (ch as i8) >= -0x40
}

/// Inline String up to `LEN - 1` chars long.
#[repr(transparent)]
#[derive(Clone)]
pub struct BoundedString {
    /// The length is the first element of the array, can't move
    /// it to another attribute, because `#[repr(transparent)]`
    /// requires an struct to have just one sized element.
    chars: [u8; CHAR_LIMIT + 1],
}

impl BoundedString {
    /// Creates a new empty `BoundedString`.
    ///
    /// Even if `BoundedString` is empty, it still consumes
    /// `CHAR_LIMIT + 1` in the stack memory space.
    ///
    /// # Examples
    ///
    /// ```
    /// use wasm_types::BoundedString;
    /// let s = BoundedString::new();
    /// ```
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self { chars: [0u8; CHAR_LIMIT + 1] }
    }

    /// Creates a `BoundedString` from `&str`, the result is always
    /// a valid utf-8 string even if the provided `s` doesn't fit.
    ///
    /// When `s.len() > CHAR_LIMIT` the exeeding bytes are ignore, only
    /// bytes that form a valid utf-8 will be considered, once a utf-8
    /// char can be up to 4 bytes long, when the max size is exceed the
    /// final length will be between `CHAR_LIMIT-3 <= len <= CHAR_LIMIT`,
    /// assuming `s` is also valid.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn from_str(mut s: &str) -> Self {
        if s.len() > CHAR_LIMIT {
            // Finds the closest `i` not exceeding CHAR_LIMIT where is_char_boundary(i) is true.
            let mut i = CHAR_LIMIT;
            while i > 0 {
                if is_utf8_char_boundary(s.as_bytes()[i]) {
                    break;
                }
                i -= 1;
            }
            //  The character boundary will be within four bytes of the CHAR_LIMIT
            debug_assert!(i >= CHAR_LIMIT.saturating_sub(3));
            s = s.split_at(i).0;
        }
        unsafe {
            // SAFETY: We guarantee `len` is within `CHAR_LIMIT` above.
            Self::from_str_unchecked(s)
        }
    }

    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn from_str_checked(s: &str) -> Option<Self> {
        if s.len() > CHAR_LIMIT {
            return None;
        }
        let bounded = unsafe {
            // SAFETY: checked that `s.len() <= CHAR_LIMIT` above
            Self::from_str_unchecked(s)
        };
        Some(bounded)
    }

    /// # Safety
    /// caller must assure that `s.len() <= CHAR_LIMIT`.
    #[must_use]
    pub const unsafe fn from_str_unchecked(s: &str) -> Self {
        let mut bounded = Self::new();
        bounded.append_str_unchecked(s);
        bounded
    }

    /// # Safety
    /// caller must assure that `self.len() + src.len() <= CHAR_LIMIT`.
    #[allow(clippy::cast_possible_truncation)]
    pub const unsafe fn append_str_unchecked(&mut self, src: &str) {
        let len = self.chars[0] as usize;
        let new_len = len.saturating_add(src.len());
        debug_assert!(new_len <= CHAR_LIMIT);

        // Safety: Caller has to check that `s.len() + self.len() <= CHAR_LIMIT`
        let (_, dest) = self.chars.split_at_mut_unchecked(len + 1);
        let (dest, _) = dest.split_at_mut_unchecked(src.len());

        dest.copy_from_slice(src.as_bytes());
        self.chars[0] = new_len as u8;
    }

    /// Returns an iterator over the [`char`]s of a string slice.
    ///
    /// As a string slice consists of valid UTF-8, we can iterate through a
    /// string slice by [`char`]. This method returns such an iterator.
    ///
    /// It's important to remember that [`char`] represents a Unicode Scalar
    /// Value, and might not match your idea of what a 'character' is. Iteration
    /// over grapheme clusters may be what you actually want. This functionality
    /// is not provided by Rust's standard library, check crates.io instead.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use wasm_types::BoundedString;
    /// let word = BoundedString::from("goodbye");
    ///
    /// let count = word.chars().count();
    /// assert_eq!(7, count);
    ///
    /// let mut chars = word.chars();
    ///
    /// assert_eq!(Some('g'), chars.next());
    /// assert_eq!(Some('o'), chars.next());
    /// assert_eq!(Some('o'), chars.next());
    /// assert_eq!(Some('d'), chars.next());
    /// assert_eq!(Some('b'), chars.next());
    /// assert_eq!(Some('y'), chars.next());
    /// assert_eq!(Some('e'), chars.next());
    ///
    /// assert_eq!(None, chars.next());
    /// ```
    ///
    /// Remember, [`char`]s might not match your intuition about characters:
    ///
    /// [`char`]: prim@char
    ///
    /// ```
    /// use wasm_types::BoundedString;
    /// let y = BoundedString::from("y̆");
    ///
    /// let mut chars = y.chars();
    ///
    /// assert_eq!(Some('y'), chars.next()); // not 'y̆'
    /// assert_eq!(Some('\u{0306}'), chars.next());
    ///
    /// assert_eq!(None, chars.next());
    /// ```
    #[inline]
    pub fn chars(&self) -> Chars<'_> {
        self.as_str().chars()
    }

    /// Returns the length of this `BoundedString` in bytes, not [`char`]s or
    /// graphemes. In other words, it might not be what a human considers the
    /// length of the string.
    ///
    /// # Examples
    ///
    /// ```
    /// use wasm_types::BoundedString;
    /// let a = BoundedString::from("foo");
    /// assert_eq!(a.len(), 3);
    ///
    /// let fancy_f = BoundedString::from("ƒoo");
    /// assert_eq!(fancy_f.len(), 4);
    /// assert_eq!(fancy_f.chars().count(), 3);
    /// ```
    #[must_use]
    #[inline(always)]
    pub const fn len(&self) -> usize {
        self.chars[0] as usize
    }

    /// Returns `true` if this `BoundedString` has a length of zero, and `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use wasm_types::BoundedString;
    /// let mut v = BoundedString::new();
    /// assert!(v.is_empty());
    ///
    /// v.try_push('a');
    /// assert!(!v.is_empty());
    /// ```
    #[must_use]
    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.chars[0] == 0
    }

    /// Split string into length and  to the bits
    #[must_use]
    #[inline(always)]
    const fn raw_parts_mut(&mut self) -> (&mut u8, &mut [u8], &mut [u8]) {
        unsafe {
            // Safety: self.chars.len() > 0
            let (length, bytes) = self.chars.split_first_mut().unwrap_unchecked();
            // Safety: self.chars[0] < self.chars.len()
            let (bytes, rest) = bytes.split_at_mut_unchecked(*length as usize);
            (length, bytes, rest)
        }
    }

    #[must_use]
    #[inline(always)]
    pub const fn as_ptr(&self) -> *const u8 {
        unsafe { self.chars.as_ptr().add(1) }
    }

    /// Returns a byte slice of this `BoundedString`'s contents.
    ///
    /// # Examples
    ///
    /// ```
    /// use wasm_types::BoundedString;
    /// let s = BoundedString::from("hello");
    ///
    /// assert_eq!(&[104, 101, 108, 108, 111], s.as_bytes());
    /// ```
    #[inline]
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8] {
        let ptr = self.as_ptr();
        let len = self.chars[0] as usize;
        unsafe { core::slice::from_raw_parts(ptr, len) }
    }

    #[must_use]
    #[inline(always)]
    pub const fn as_mut_ptr(&mut self) -> *mut u8 {
        unsafe { self.chars.as_mut_ptr().add(1) }
    }

    #[must_use]
    #[inline(always)]
    pub const fn as_bytes_mut(&mut self) -> &mut [u8] {
        let ptr = self.as_mut_ptr();
        let len = self.chars[0] as usize;
        unsafe { core::slice::from_raw_parts_mut(ptr, len) }
    }

    /// Converts a `BoundedString` into a mutable string slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use wasm_types::BoundedString;
    /// let mut s = BoundedString::from("foobar");
    /// let s_mut_str = s.as_mut_str();
    ///
    /// s_mut_str.make_ascii_uppercase();
    ///
    /// assert_eq!("FOOBAR", s_mut_str);
    /// ```
    #[must_use]
    #[inline(always)]
    pub const fn as_mut_str(&mut self) -> &mut str {
        unsafe { str::from_utf8_unchecked_mut(self.as_bytes_mut()) }
    }

    /// Extracts a string slice hold by `BoundedString`.
    ///
    /// # Examples
    ///
    /// ```
    /// use wasm_types::BoundedString;
    /// let s = BoundedString::from("foo");
    ///
    /// assert_eq!("foo", s.as_str());
    /// ```
    #[must_use]
    #[inline(always)]
    pub const fn as_str(&self) -> &str {
        let bytes = self.as_bytes();
        unsafe { str::from_utf8_unchecked(bytes) }
    }

    /// Try to append the given [`char`] to the end of this `BoundedString`.
    ///
    /// # Examples
    ///
    /// ```
    /// use wasm_types::BoundedString;
    /// let mut s = BoundedString::from("abc");
    ///
    /// s.try_push('1');
    /// s.try_push('2');
    /// s.try_push('3');
    ///
    /// assert_eq!(s, "abc123");
    /// ```
    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn try_push(&mut self, ch: char) -> bool {
        let (length, _, rest) = self.raw_parts_mut();
        if rest.len() >= ch.len_utf8() {
            *length += ch.encode_utf8(rest).len() as u8;
            true
        } else {
            false
        }
    }

    /// Removes the last character from the string buffer and returns it.
    ///
    /// Returns [`None`] if this `BoundedString` is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use wasm_types::BoundedString;
    /// let mut s = BoundedString::from("abč");
    ///
    /// assert_eq!(s.pop(), Some('č'));
    /// assert_eq!(s.pop(), Some('b'));
    /// assert_eq!(s.pop(), Some('a'));
    ///
    /// assert_eq!(s.pop(), None);
    /// ```
    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    pub fn pop(&mut self) -> Option<char> {
        let ch = self.as_str().chars().next_back()?;
        let newlen = self.len() - ch.len_utf8();
        self.chars[0] = newlen as u8;
        Some(ch)
    }
}

impl From<&'_ str> for BoundedString {
    #[allow(clippy::cast_possible_truncation)]
    fn from(s: &'_ str) -> Self {
        Self::from_str(s)
    }
}

impl Default for BoundedString {
    fn default() -> Self {
        Self::new()
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

impl<'a> PartialEq<&'a str> for BoundedString {
    fn eq(&self, other: &&'a str) -> bool {
        <str as PartialEq>::eq(self.as_str(), other)
    }
}

impl Eq for BoundedString {}

impl Hash for BoundedString {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        <str as Hash>::hash::<H>(self.as_str(), state);
    }
}

impl AsRef<[u8]> for BoundedString {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
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
        let mut chars = [0u8; CHAR_LIMIT + 1];
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
    use super::{BoundedString, CHAR_LIMIT};
    use unicode_segmentation::UnicodeSegmentation;

    #[test]
    fn it_works() {
        let tests = ["", "hello", "hello world", "a"];
        for test in tests {
            let s = BoundedString::from_str(test);
            assert_eq!(s, test);
            assert_eq!(format!("{s}"), format!("{test}"));
            assert_eq!(format!("{s:?}"), format!("{test:?}"));
        }
    }

    #[test]
    fn test_from_str() {
        // Make sure `BoundedString::from_str` always parses valid utf-8 chars
        // when the provided string is greater than `CHAR_LIMIT`.
        let unicode_chars = "❤️🧡💛💚💙💜";
        assert_eq!(unicode_chars.len(), 26);

        // Notice the number of utf-8 chars isn't the same as the number of
        // Unicode chars. The emoji `❤️` uses 6 bytes, and the first 3 bytes
        // also represents a valid utf-8 char `❤`. For simplicity we only
        // consider the utf-8 char boundary.
        assert_eq!(unicode_chars.graphemes(true).count(), 6);
        assert_eq!(unicode_chars.chars().count(), 7);

        // Get the indices of UTF-8 chars (not unicode).
        let mut indices = unicode_chars.char_indices().map(|i| i.0).collect::<Vec<_>>();

        // Use the indices to compute the byte size of each char.
        let mut prev = indices[0];
        for i in 0..indices.len() {
            indices[i] = indices
                .get(i + 1)
                .copied()
                .map_or_else(|| unicode_chars.len() - indices[i], |next| next - prev);
            prev += indices[i];
        }

        // Create a string we will use in the tests, it must be
        // prefixed with `CHAR_LIMIT` single byte utf-8 chars.
        let input = {
            // Make sure
            let prefix_len = usize::max(CHAR_LIMIT, unicode_chars.len());
            let mut s = String::with_capacity(prefix_len + unicode_chars.len());
            // append single utf-8
            while s.len() < prefix_len {
                s.push('a');
            }
            // append unicode suffix.
            s.push_str(unicode_chars);
            s
        };
        // Convert `String` to `&str`
        let mut str = input.as_str();

        // If we pass the whole string to `BoundedString`,
        // it must contain only the prefix.
        let mut bounded = BoundedString::from_str(str);
        assert_eq!(bounded, str[..CHAR_LIMIT]);

        for index in indices {
            for i in 1..index {
                str = &str[1..];
                bounded = BoundedString::from_str(str);
                assert_eq!(bounded, str[0..(CHAR_LIMIT - i)]);
            }
            str = &str[1..];
            bounded = BoundedString::from_str(str);
            assert_eq!(bounded, str[..CHAR_LIMIT]);
        }
        assert!(bounded.as_str().ends_with(unicode_chars));
    }
}
