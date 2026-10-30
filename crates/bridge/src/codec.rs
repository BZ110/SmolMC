//! Fixed M2C schema; no implicit layout or unbounded collection deserialization.
use std::io;
pub const MAX_FRAME: usize = 131072;
pub fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid bridge frame")
}
pub struct Reader<'a>(pub &'a [u8]);
impl<'a> Reader<'a> {
    pub fn take(&mut self, n: usize) -> io::Result<&'a [u8]> {
        if n > self.0.len() {
            return Err(invalid());
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    pub fn u8(&mut self) -> io::Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn u16(&mut self) -> io::Result<u16> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn u64(&mut self) -> io::Result<u64> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn i32(&mut self) -> io::Result<i32> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn string(&mut self, max: usize) -> io::Result<&'a str> {
        let n = self.u16()? as usize;
        if n > max {
            return Err(invalid());
        }
        std::str::from_utf8(self.take(n)?).map_err(|_| invalid())
    }
    pub fn end(&self) -> io::Result<()> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(invalid())
        }
    }
    pub fn world(&mut self) -> io::Result<smolmc_world::WorldHandle> {
        Ok(smolmc_world::WorldHandle {
            id: smolmc_world::WorldId(self.u64()?),
            generation: self.u64()?,
        })
    }
    pub fn positions(&mut self) -> io::Result<Vec<[i32; 3]>> {
        let count = self.u16()? as usize;
        if count > 4096 {
            return Err(invalid());
        }
        (0..count)
            .map(|_| Ok([self.i32()?, self.i32()?, self.i32()?]))
            .collect()
    }
}
pub fn u16(out: &mut Vec<u8>, v: u16) {
    out.extend(v.to_be_bytes());
}
pub fn u64(out: &mut Vec<u8>, v: u64) {
    out.extend(v.to_be_bytes());
}
pub fn string(out: &mut Vec<u8>, s: &str) {
    u16(out, s.len() as u16);
    out.extend(s.as_bytes());
}
pub fn world(out: &mut Vec<u8>, w: smolmc_world::WorldHandle) {
    u64(out, w.id.0);
    u64(out, w.generation);
}
pub fn player(out: &mut Vec<u8>, p: &smolmc_sessions::control::PlayerSnapshot) {
    out.extend(p.entity.to_be_bytes());
    out.extend(p.uuid);
    string(out, &p.name);
    world(out, p.world);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_bounds_truncation_utf8_and_trailing() {
        assert!(Reader(&[0, 1, 255]).string(1).is_err());
        assert!(Reader(&[0, 2, 65]).string(8).is_err());
        assert!(Reader(&[0, 2, 65, 65]).string(1).is_err());
        assert!(Reader(&[1]).end().is_err());
        assert!(Reader(&4097u16.to_be_bytes()).positions().is_err());
        let mut out = Vec::new();
        string(&mut out, "é");
        let mut r = Reader(&out);
        assert_eq!(r.string(2).unwrap(), "é");
        r.end().unwrap();
    }
}
