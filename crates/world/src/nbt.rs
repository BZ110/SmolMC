//! Bounded disk NBT reader. No interpretation/execution of entities, scripts or assets.
use std::{collections::BTreeMap, io};
#[derive(Debug)]
pub(crate) enum Tag {
    Number(i64),
    Bytes(Vec<u8>),
    String(String),
    List(Vec<Tag>),
    Compound(BTreeMap<String, Tag>),
    Longs(Vec<u64>),
    Other,
}
impl Tag {
    pub fn get(&self, k: &str) -> Option<&Tag> {
        if let Self::Compound(v) = self {
            v.get(k)
        } else {
            None
        }
    }
    pub fn number(&self) -> Option<i64> {
        if let Self::Number(v) = self {
            Some(*v)
        } else {
            None
        }
    }
    pub fn string(&self) -> Option<&str> {
        if let Self::String(v) = self {
            Some(v)
        } else {
            None
        }
    }
    pub fn list(&self) -> Option<&[Tag]> {
        if let Self::List(v) = self {
            Some(v)
        } else {
            None
        }
    }
    pub fn bytes(&self) -> Option<&[u8]> {
        if let Self::Bytes(v) = self {
            Some(v)
        } else {
            None
        }
    }
    pub fn longs(&self) -> Option<&[u64]> {
        if let Self::Longs(v) = self {
            Some(v)
        } else {
            None
        }
    }
    pub fn compound(&self) -> Option<&BTreeMap<String, Tag>> {
        if let Self::Compound(v) = self {
            Some(v)
        } else {
            None
        }
    }
}
pub(crate) fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}
struct Reader<'a> {
    data: &'a [u8],
    at: usize,
    items: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> io::Result<&'a [u8]> {
        let end = self
            .at
            .checked_add(n)
            .ok_or_else(|| invalid("NBT offset overflow"))?;
        let b = self
            .data
            .get(self.at..end)
            .ok_or_else(|| invalid("truncated NBT"))?;
        self.at = end;
        Ok(b)
    }
    fn byte(&mut self) -> io::Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn count(&mut self, unit: usize) -> io::Result<usize> {
        let n = i32::from_be_bytes(self.take(4)?.try_into().unwrap());
        let n = usize::try_from(n).map_err(|_| invalid("negative NBT length"))?;
        if n > 262144
            || n.checked_mul(unit)
                .is_none_or(|b| b > self.data.len().saturating_sub(self.at))
        {
            return Err(invalid("NBT array/list budget"));
        }
        Ok(n)
    }
    fn text(&mut self) -> io::Result<String> {
        let n = u16::from_be_bytes(self.take(2)?.try_into().unwrap()) as usize;
        if n > 4096 {
            return Err(invalid("NBT string budget"));
        }
        let s = std::str::from_utf8(self.take(n)?).map_err(|_| invalid("NBT identifier UTF-8"))?;
        Ok(s.to_owned())
    }
    fn tag(&mut self, t: u8, depth: usize) -> io::Result<Tag> {
        self.items += 1;
        if depth > 32 || self.items > 131072 {
            return Err(invalid("NBT depth/node budget"));
        }
        Ok(match t {
            1 => Tag::Number(self.byte()? as i8 as i64),
            2 => Tag::Number(i16::from_be_bytes(self.take(2)?.try_into().unwrap()) as i64),
            3 => Tag::Number(i32::from_be_bytes(self.take(4)?.try_into().unwrap()) as i64),
            4 => Tag::Number(i64::from_be_bytes(self.take(8)?.try_into().unwrap())),
            5 => {
                self.take(4)?;
                Tag::Other
            }
            6 => {
                self.take(8)?;
                Tag::Other
            }
            7 => {
                let n = self.count(1)?;
                Tag::Bytes(self.take(n)?.to_vec())
            }
            8 => Tag::String(self.text()?),
            9 => {
                let kind = self.byte()?;
                let n = self.count(1)?;
                if kind == 0 && n != 0 {
                    return Err(invalid("nonempty END list"));
                }
                let mut v = Vec::with_capacity(n.min(4096));
                for _ in 0..n {
                    let mut tag = self.tag(kind, depth + 1)?;
                    if let Tag::Compound(c) = &mut tag
                        && c.len() == 1
                        && c.contains_key("")
                    {
                        tag = c.remove("").unwrap()
                    }
                    v.push(tag)
                }
                Tag::List(v)
            }
            10 => {
                let mut c = BTreeMap::new();
                loop {
                    let kind = self.byte()?;
                    if kind == 0 {
                        break;
                    }
                    let name = self.text()?;
                    let value = self.tag(kind, depth + 1)?;
                    if c.insert(name, value).is_some() {
                        return Err(invalid("duplicate NBT key"));
                    }
                }
                Tag::Compound(c)
            }
            11 => {
                let n = self.count(4)?;
                self.take(n * 4)?;
                Tag::Other
            }
            12 => {
                let n = self.count(8)?;
                let mut v = Vec::with_capacity(n);
                for _ in 0..n {
                    v.push(u64::from_be_bytes(self.take(8)?.try_into().unwrap()))
                }
                Tag::Longs(v)
            }
            _ => return Err(invalid("unknown NBT tag")),
        })
    }
}
pub(crate) fn read(bytes: &[u8]) -> io::Result<Tag> {
    if bytes.len() > 2 * 1024 * 1024 {
        return Err(invalid("NBT decoded budget"));
    }
    let mut r = Reader {
        data: bytes,
        at: 0,
        items: 0,
    };
    if r.byte()? != 10 {
        return Err(invalid("NBT root must be compound"));
    }
    r.text()?;
    let root = r.tag(10, 0)?;
    if r.at != bytes.len() {
        return Err(invalid("trailing disk NBT"));
    }
    Ok(root)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hostile_lengths_depth_and_duplicates() {
        assert!(read(&[10, 0, 0, 7, 0, 1, b'x', 127, 255, 255, 255, 0]).is_err());
        assert!(read(&[10, 0, 0, 1, 0, 1, b'x', 1, 1, 0, 1, b'x', 2, 0]).is_err());
        let mut b = vec![10, 0, 0];
        for _ in 0..34 {
            b.extend([10, 0, 1, b'x'])
        }
        b.extend([0; 35]);
        assert!(read(&b).is_err());
        assert!(read(&[10, 0, 0, 0, 1]).is_err());
        assert!(read(&[10, 0, 0, 0]).is_ok());
    }
}
