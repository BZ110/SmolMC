//! Persistent, cancellation-safe framing. Each half has one owner. Raw read-ahead
//! stays raw until consumed, so encryption can start at an exact packet boundary.
use flate2::{Compression, Decompress, FlushDecompress, Status, write::ZlibEncoder};
use openssl::symm::{Cipher, Crypter, Mode};
use smolmc_protocol_777::{decode_varint, encode_varint};
use std::{
    io,
    ops::Deref,
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed},
    },
    time::Instant,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    sync::{OwnedSemaphorePermit, Semaphore},
};

pub const LOGIN_LIMIT: usize = 1024;
pub const CONTROL_LIMIT: usize = 64 * 1024;
pub const OUTPUT_LIMIT: usize = 256 * 1024;
const READ_AHEAD: usize = 4096;
const MAX_RATIO: usize = 128;
pub const BATCH_PACKETS: usize = 64;
pub const BATCH_BYTES: usize = 16 * 1024;

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

const INBOUND_WORK_PER_SECOND: u32 = 8 * 1024 * 1024;
const CONNECTION_WORK_PER_SECOND: usize = 128 * 1024;
struct BudgetState {
    bytes: Arc<Semaphore>,
    capacity: usize,
    peak: AtomicUsize,
    epoch: Instant,
    traffic: AtomicU64,
}
#[derive(Clone)]
pub struct ByteBudget(Arc<BudgetState>);
impl ByteBudget {
    pub fn new(bytes: usize) -> Self {
        Self(Arc::new(BudgetState {
            bytes: Arc::new(Semaphore::new(bytes)),
            capacity: bytes,
            peak: AtomicUsize::new(0),
            epoch: Instant::now(),
            traffic: AtomicU64::new(u64::from(INBOUND_WORK_PER_SECOND)),
        }))
    }
    pub fn available(&self) -> usize {
        self.0.bytes.available_permits()
    }
    pub fn peak_reserved(&self) -> usize {
        self.0.peak.load(Relaxed)
    }
    fn claim(&self, bytes: usize) -> io::Result<OwnedSemaphorePermit> {
        let bytes = u32::try_from(bytes).map_err(|_| invalid("byte budget overflow"))?;
        let permit = Arc::clone(&self.0.bytes)
            .try_acquire_many_owned(bytes)
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::OutOfMemory,
                    "connection byte budget exhausted",
                )
            })?;
        self.0
            .peak
            .fetch_max(self.0.capacity - self.available(), Relaxed);
        Ok(permit)
    }
    /// Bound aggregate decrypt/decompression work without a mutex or a periodic
    /// task. Charged for wire bodies and additionally for decompressed output.
    fn charge_inbound(&self, bytes: usize) -> io::Result<()> {
        let epoch = (self.0.epoch.elapsed().as_secs().min(u32::MAX as u64) as u32) as u64;
        let mut old = self.0.traffic.load(Relaxed);
        loop {
            let remaining = if old >> 32 == epoch {
                old as u32
            } else {
                INBOUND_WORK_PER_SECOND
            };
            if bytes > remaining as usize {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "global inbound work budget exceeded",
                ));
            }
            let new = (epoch << 32) | u64::from(remaining - bytes as u32);
            match self
                .0
                .traffic
                .compare_exchange_weak(old, new, Relaxed, Relaxed)
            {
                Ok(_) => return Ok(()),
                Err(actual) => old = actual,
            }
        }
    }
}

/// The allocation lease travels with the packet, including across async calls.
pub struct Packet {
    bytes: Vec<u8>,
    _lease: OwnedSemaphorePermit,
}
impl Deref for Packet {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.bytes
    }
}

struct StreamCipher(Crypter);
impl StreamCipher {
    fn new(key: &[u8; 16], mode: Mode) -> io::Result<Self> {
        let mut cipher = Crypter::new(Cipher::aes_128_cfb8(), mode, key, Some(key))
            .map_err(|_| invalid("cannot initialize stream cipher"))?;
        cipher.pad(false);
        Ok(Self(cipher))
    }
    fn apply(&mut self, input: &[u8], output: &mut [u8]) -> io::Result<usize> {
        let used = self
            .0
            .update(input, output)
            .map_err(|_| invalid("stream cipher error"))?;
        if used != input.len() {
            return Err(invalid("unexpected cipher output size"));
        }
        Ok(used)
    }
}

pub struct Inbound<R> {
    io: R,
    raw: Box<[u8; READ_AHEAD]>,
    start: usize,
    end: usize,
    prefix: [u8; 3],
    prefix_used: usize,
    body: Option<Packet>,
    received: usize,
    expected: usize,
    cipher: Option<StreamCipher>,
    compression: Option<usize>,
    limit: usize,
    budget: ByteBudget,
    work_window: Instant,
    work_bytes: usize,
    work_limit: usize,
}

impl<R: AsyncRead + Unpin> Inbound<R> {
    pub fn new(io: R, budget: ByteBudget) -> Self {
        Self {
            io,
            raw: Box::new([0; READ_AHEAD]),
            start: 0,
            end: 0,
            prefix: [0; 3],
            prefix_used: 0,
            body: None,
            received: 0,
            expected: 0,
            cipher: None,
            compression: None,
            limit: LOGIN_LIMIT,
            budget,
            work_window: Instant::now(),
            work_bytes: 0,
            work_limit: CONNECTION_WORK_PER_SECOND,
        }
    }
    pub fn set_limit(&mut self, limit: usize) -> io::Result<()> {
        if self.body.is_some() || self.prefix_used != 0 || !(1..=CONTROL_LIMIT).contains(&limit) {
            return Err(invalid("invalid frame limit transition"));
        }
        self.limit = limit;
        Ok(())
    }
    /// Receiver policy for trusted library users (e.g. client-side test probes
    /// receive terrain). Deployed server sessions retain the 128 KiB/s default.
    pub fn set_work_limit(&mut self, bytes: usize) -> io::Result<()> {
        if bytes == 0 || bytes > INBOUND_WORK_PER_SECOND as usize {
            return Err(invalid("invalid connection work limit"));
        }
        self.work_limit = bytes;
        Ok(())
    }
    pub fn enable_encryption(&mut self, key: &[u8; 16]) -> io::Result<()> {
        if self.cipher.is_some() || self.body.is_some() || self.prefix_used != 0 {
            return Err(invalid("invalid encryption transition"));
        }
        self.cipher = Some(StreamCipher::new(key, Mode::Decrypt)?);
        Ok(())
    }
    pub fn enable_compression(&mut self, threshold: usize) -> io::Result<()> {
        if self.compression.is_some() || self.body.is_some() || self.prefix_used != 0 {
            return Err(invalid("invalid compression transition"));
        }
        self.compression = Some(threshold);
        Ok(())
    }
    async fn fill(&mut self) -> io::Result<()> {
        if self.start == self.end {
            let count = self.io.read(&mut self.raw[..]).await?;
            if count == 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            self.start = 0;
            self.end = count;
        }
        Ok(())
    }
    fn charge_work(&mut self, bytes: usize) -> io::Result<()> {
        if self.work_window.elapsed().as_secs() >= 1 {
            self.work_window = Instant::now();
            self.work_bytes = 0;
        }
        if bytes > self.work_limit.saturating_sub(self.work_bytes) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "connection inbound work budget exceeded",
            ));
        }
        self.work_bytes += bytes;
        self.budget.charge_inbound(bytes)
    }
    /// Safe to cancel and resume: prefix, partial body and cipher positions are
    /// committed inside this object, never in a discarded read_exact future.
    pub async fn read_packet(&mut self) -> io::Result<Packet> {
        while self.body.is_none() {
            self.fill().await?;
            let raw = self.raw[self.start];
            let mut byte = raw;
            if let Some(cipher) = &mut self.cipher {
                let mut clear = [0; 17];
                cipher.apply(&[raw], &mut clear)?;
                byte = clear[0];
            }
            self.start += 1;
            if self.prefix_used == 3 {
                return Err(invalid("frame prefix exceeds 21 bits"));
            }
            self.prefix[self.prefix_used] = byte;
            self.prefix_used += 1;
            if byte & 0x80 != 0 {
                if self.prefix_used == 3 {
                    return Err(invalid("frame prefix exceeds 21 bits"));
                }
                continue;
            }
            let (length, _) = decode_varint(&self.prefix[..self.prefix_used])
                .map_err(|_| invalid("bad frame length"))?;
            if length <= 0 || length as usize > self.limit {
                return Err(invalid("frame exceeds phase budget"));
            }
            let length = length as usize;
            self.charge_work(length)?;
            let lease = self.budget.claim(length + 16)?;
            self.body = Some(Packet {
                bytes: vec![0; length + 16],
                _lease: lease,
            });
            self.expected = length;
            self.received = 0;
        }
        let mut work_since_yield = 0;
        while self.received < self.expected {
            self.fill().await?;
            let count = (self.end - self.start).min(self.expected - self.received);
            let input = &self.raw[self.start..self.start + count];
            let body = &mut self.body.as_mut().expect("body initialized").bytes;
            if let Some(cipher) = &mut self.cipher {
                cipher.apply(input, &mut body[self.received..self.received + count + 16])?;
            } else {
                body[self.received..self.received + count].copy_from_slice(input);
            }
            self.start += count;
            self.received += count;
            work_since_yield += count;
            if work_since_yield >= 8192 {
                work_since_yield = 0;
                tokio::task::yield_now().await;
            }
        }
        let mut packet = self.body.take().expect("complete packet");
        packet.bytes.truncate(self.expected);
        self.prefix_used = 0;
        self.received = 0;
        self.expected = 0;
        let Some(threshold) = self.compression else {
            return Ok(packet);
        };
        let (decoded, used) =
            decode_varint(&packet).map_err(|_| invalid("bad compression length"))?;
        if decoded == 0 {
            if packet.len() - used >= threshold {
                return Err(invalid("uncompressed packet above threshold"));
            }
            packet.bytes.drain(..used);
            return Ok(packet);
        }
        let decoded =
            usize::try_from(decoded).map_err(|_| invalid("negative decompressed size"))?;
        let compressed = &packet[used..];
        if decoded < threshold
            || decoded > self.limit
            || decoded > compressed.len().saturating_mul(MAX_RATIO)
        {
            return Err(invalid("compressed packet exceeds budget"));
        }
        let lease = self.budget.claim(decoded + 1)?;
        self.charge_work(decoded)?;
        let mut output = vec![0; decoded + 1]; // one extra byte detects a false declared length
        let mut decoder = Decompress::new(true);
        let status = decoder
            .decompress(compressed, &mut output, FlushDecompress::Finish)
            .map_err(|_| invalid("invalid zlib stream"))?;
        if status != Status::StreamEnd
            || decoder.total_out() as usize != decoded
            || decoder.total_in() as usize != compressed.len()
        {
            return Err(invalid("zlib length mismatch or trailing bytes"));
        }
        output.truncate(decoded);
        Ok(Packet {
            bytes: output,
            _lease: lease,
        })
    }
}

pub struct Outbound<W> {
    io: W,
    pending: Option<Packet>,
    cursor: usize,
    cipher: Option<StreamCipher>,
    compression: Option<usize>,
    budget: ByteBudget,
}

impl<W: AsyncWrite + Unpin> Outbound<W> {
    pub fn new(io: W, budget: ByteBudget) -> Self {
        Self {
            io,
            pending: None,
            cursor: 0,
            cipher: None,
            compression: None,
            budget,
        }
    }
    pub fn enable_encryption(&mut self, key: &[u8; 16]) -> io::Result<()> {
        if self.pending.is_some() || self.cipher.is_some() {
            return Err(invalid("invalid write encryption transition"));
        }
        self.cipher = Some(StreamCipher::new(key, Mode::Encrypt)?);
        Ok(())
    }
    pub fn enable_compression(&mut self, threshold: usize) -> io::Result<()> {
        if self.pending.is_some() || self.compression.is_some() {
            return Err(invalid("invalid write compression transition"));
        }
        self.compression = Some(threshold);
        Ok(())
    }
    /// Exactly one bounded pending frame. Re-queueing a partially written frame
    /// is rejected; flush resumes its saved cursor without re-encrypting bytes.
    pub fn queue(&mut self, payload: &[u8]) -> io::Result<()> {
        use std::io::Write;
        if self.pending.is_some() {
            return Err(invalid("write already pending"));
        }
        if payload.is_empty() || payload.len() > OUTPUT_LIMIT {
            return Err(invalid("outbound packet exceeds budget"));
        }
        // Reserve scratch + framed + encrypted worst-case space before allocation.
        let lease = self.budget.claim(payload.len() * 3 + 256)?;
        let mut inner = Vec::new();
        if let Some(threshold) = self.compression {
            if payload.len() >= threshold {
                encode_varint(payload.len() as i32, &mut inner);
                let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
                encoder.write_all(payload)?;
                inner.extend_from_slice(&encoder.finish()?);
            } else {
                inner.push(0);
                inner.extend_from_slice(payload);
            }
        } else {
            inner.extend_from_slice(payload);
        }
        let mut framed = Vec::with_capacity(inner.len() + 3);
        encode_varint(inner.len() as i32, &mut framed);
        framed.extend_from_slice(&inner);
        if let Some(cipher) = &mut self.cipher {
            let mut encrypted = vec![0; framed.len() + 16];
            let used = cipher.apply(&framed, &mut encrypted)?;
            encrypted.truncate(used);
            framed = encrypted;
        }
        self.pending = Some(Packet {
            bytes: framed,
            _lease: lease,
        });
        self.cursor = 0;
        Ok(())
    }
    /// Independent Minecraft frames in one bounded pending write. Compression is
    /// still per packet; the connection's cipher advances once over the same byte
    /// stream. A cancelled flush resumes its cursor without framing/encrypting twice.
    pub fn queue_batch(&mut self, packets: &[Vec<u8>]) -> io::Result<()> {
        use std::io::Write;
        if self.pending.is_some() {
            return Err(invalid("write already pending"));
        }
        if packets.is_empty() || packets.len() > BATCH_PACKETS {
            return Err(invalid("invalid packet batch count"));
        }
        let mut total = 0usize;
        for packet in packets {
            if packet.is_empty() || packet.len() > BATCH_BYTES {
                return Err(invalid("invalid batched packet size"));
            }
            total = total
                .checked_add(packet.len())
                .ok_or_else(|| invalid("batch size overflow"))?;
        }
        if total > BATCH_BYTES {
            return Err(invalid("packet batch exceeds byte bound"));
        }
        let lease = self.budget.claim(total * 3 + packets.len() * 256)?;
        let mut framed = Vec::with_capacity(total + packets.len() * 6);
        for packet in packets {
            if let Some(threshold) = self.compression {
                if packet.len() >= threshold {
                    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
                    encoder.write_all(packet)?;
                    let compressed = encoder.finish()?;
                    let header = (32 - (packet.len() as u32).leading_zeros()).div_ceil(7) as usize;
                    encode_varint((header + compressed.len()) as i32, &mut framed);
                    encode_varint(packet.len() as i32, &mut framed);
                    framed.extend_from_slice(&compressed);
                } else {
                    encode_varint((packet.len() + 1) as i32, &mut framed);
                    framed.push(0);
                    framed.extend_from_slice(packet);
                }
            } else {
                encode_varint(packet.len() as i32, &mut framed);
                framed.extend_from_slice(packet);
            }
        }
        if let Some(cipher) = &mut self.cipher {
            let mut encrypted = vec![0; framed.len() + 16];
            let used = cipher.apply(&framed, &mut encrypted)?;
            encrypted.truncate(used);
            framed = encrypted;
        }
        self.pending = Some(Packet {
            bytes: framed,
            _lease: lease,
        });
        self.cursor = 0;
        Ok(())
    }
    pub async fn send_batch(&mut self, packets: &[Vec<u8>]) -> io::Result<()> {
        self.queue_batch(packets)?;
        self.flush().await
    }
    pub async fn flush(&mut self) -> io::Result<()> {
        if let Some(packet) = &self.pending {
            while self.cursor < packet.len() {
                let written = self.io.write(&packet[self.cursor..]).await?;
                if written == 0 {
                    return Err(io::ErrorKind::WriteZero.into());
                }
                self.cursor += written;
            }
        }
        self.pending = None;
        self.cursor = 0;
        Ok(())
    }
    pub async fn send(&mut self, packet: &[u8]) -> io::Result<()> {
        self.queue(packet)?;
        self.flush().await
    }
    pub async fn shutdown(&mut self) -> io::Result<()> {
        self.flush().await?;
        self.io.shutdown().await
    }
}

pub type TcpInbound = Inbound<tokio::net::tcp::OwnedReadHalf>;
pub type TcpOutbound = Outbound<tokio::net::tcp::OwnedWriteHalf>;
pub struct Connection {
    pub inbound: TcpInbound,
    pub outbound: TcpOutbound,
}
impl Connection {
    pub fn new(socket: tokio::net::TcpStream, budget: ByteBudget) -> Self {
        let (read, write) = socket.into_split();
        Self {
            inbound: Inbound::new(read, budget.clone()),
            outbound: Outbound::new(write, budget),
        }
    }
    pub fn enable_encryption(&mut self, key: &[u8; 16]) -> io::Result<()> {
        self.inbound.enable_encryption(key)?;
        self.outbound.enable_encryption(key)
    }
    pub fn enable_compression(&mut self, threshold: usize) -> io::Result<()> {
        self.inbound.enable_compression(threshold)?;
        self.outbound.enable_compression(threshold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::{io::duplex, time::timeout};

    #[tokio::test]
    async fn one_peer_cannot_spend_the_global_inbound_allowance() {
        let budget = ByteBudget::new(1024 * 1024);
        let (mut attacker, stream) = duplex(256 * 1024);
        let mut inbound = Inbound::new(stream, budget.clone());
        inbound.set_limit(CONTROL_LIMIT).unwrap();
        let mut frame = Vec::new();
        encode_varint(CONTROL_LIMIT as i32, &mut frame);
        frame.extend_from_slice(&vec![42; CONTROL_LIMIT]);
        for _ in 0..3 {
            attacker.write_all(&frame).await.unwrap();
        }
        drop(inbound.read_packet().await.unwrap());
        drop(inbound.read_packet().await.unwrap());
        assert_eq!(
            inbound.read_packet().await.err().unwrap().kind(),
            io::ErrorKind::WouldBlock
        );
        assert_eq!(
            budget.0.traffic.load(Relaxed) as u32,
            INBOUND_WORK_PER_SECOND - CONNECTION_WORK_PER_SECOND as u32
        );
        let (mut healthy, stream) = duplex(64);
        let mut other = Inbound::new(stream, budget.clone());
        healthy.write_all(&[1, 0]).await.unwrap();
        assert_eq!(&*other.read_packet().await.unwrap(), &[0]);
        inbound.work_window = Instant::now() - std::time::Duration::from_secs(1);
        inbound.charge_work(1).unwrap(); // allowance renews without a worker/timer
        drop(inbound);
        drop(other);
        assert_eq!(budget.available(), 1024 * 1024);
    }

    #[tokio::test]
    async fn partial_header_and_body_survive_cancellation() {
        for split in [1, 2, 7, 2900] {
            let budget = ByteBudget::new(1024 * 1024);
            let (mut sender, receiver) = duplex(4096);
            let mut input = Inbound::new(receiver, budget.clone());
            input.set_limit(CONTROL_LIMIT).unwrap();
            let body = vec![42; 3000];
            let bytes = smolmc_protocol_777::frame(&body).unwrap();
            sender.write_all(&bytes[..split]).await.unwrap();
            assert!(
                timeout(Duration::from_millis(2), input.read_packet())
                    .await
                    .is_err()
            );
            sender.write_all(&bytes[split..]).await.unwrap();
            let packet = input.read_packet().await.unwrap();
            assert_eq!(&*packet, &body);
            drop(packet);
            drop(input);
            assert_eq!(budget.available(), 1024 * 1024);
        }
    }

    #[tokio::test]
    async fn partial_write_survives_cancellation_without_reencoding() {
        let budget = ByteBudget::new(1024 * 1024);
        let (sender, mut receiver) = duplex(1);
        let mut output = Outbound::new(sender, budget.clone());
        let payload = vec![17; 300];
        output.queue(&payload).unwrap();
        assert!(
            timeout(Duration::from_millis(2), output.flush())
                .await
                .is_err()
        );
        assert!(output.queue(&payload).is_err());
        let expected = smolmc_protocol_777::frame(&payload).unwrap();
        let count = expected.len();
        let task = tokio::spawn(async move {
            let mut got = vec![0; count];
            receiver.read_exact(&mut got).await.unwrap();
            got
        });
        output.flush().await.unwrap();
        assert_eq!(task.await.unwrap(), expected);
        assert_eq!(budget.available(), 1024 * 1024);
    }

    #[tokio::test]
    async fn encrypted_read_ahead_starts_at_exact_boundary() {
        let budget = ByteBudget::new(1024 * 1024);
        let (mut sender, receiver) = duplex(4096);
        let key = [13; 16];
        let next = smolmc_protocol_777::frame(&[1, 2, 3, 4]).unwrap();
        let encrypted =
            openssl::symm::encrypt(Cipher::aes_128_cfb8(), &key, Some(&key), &next).unwrap();
        sender
            .write_all(&[vec![1, 0], encrypted].concat())
            .await
            .unwrap();
        let mut input = Inbound::new(receiver, budget);
        assert_eq!(&*input.read_packet().await.unwrap(), &[0]);
        input.enable_encryption(&key).unwrap();
        assert_eq!(&*input.read_packet().await.unwrap(), &[1, 2, 3, 4]);
    }

    #[tokio::test]
    async fn compression_encryption_and_budget_cleanup() {
        let budget = ByteBudget::new(1024 * 1024);
        let (sender, receiver) = duplex(4096);
        let mut output = Outbound::new(sender, budget.clone());
        let mut input = Inbound::new(receiver, budget.clone());
        let key = [19; 16];
        output.enable_encryption(&key).unwrap();
        input.enable_encryption(&key).unwrap();
        output.enable_compression(256).unwrap();
        input.enable_compression(256).unwrap();
        for size in [1, 255, 256, 1000] {
            let payload: Vec<_> = (0..size).map(|i| (i % 251) as u8).collect();
            output.send(&payload).await.unwrap();
            assert_eq!(&*input.read_packet().await.unwrap(), &payload);
        }
        assert_eq!(budget.available(), 1024 * 1024);
    }

    #[tokio::test]
    async fn batched_ciphertext_matches_individual_framing_and_cipher_continuation() {
        for compression in [None, Some(256)] {
            let budget = ByteBudget::new(1024 * 1024);
            let mut single = Outbound::new(tokio::io::sink(), budget.clone());
            let mut batch = Outbound::new(tokio::io::sink(), budget.clone());
            for out in [&mut single, &mut batch] {
                out.enable_encryption(&[23; 16]).unwrap();
                if let Some(threshold) = compression {
                    out.enable_compression(threshold).unwrap();
                }
            }
            let packets: Vec<Vec<u8>> = [1, 5, 255, 256, 1000]
                .into_iter()
                .map(|n| (0..n).map(|i| (i % 251) as u8).collect())
                .collect();
            let mut expected = Vec::new();
            for packet in &packets {
                single.queue(packet).unwrap();
                expected.extend_from_slice(&single.pending.as_ref().unwrap().bytes);
                single.flush().await.unwrap();
            }
            batch.queue_batch(&packets).unwrap();
            assert_eq!(batch.pending.as_ref().unwrap().bytes, expected);
            batch.flush().await.unwrap();
            single.queue(&[1, 2, 3]).unwrap();
            batch.queue(&[1, 2, 3]).unwrap();
            assert_eq!(
                single.pending.as_ref().unwrap().bytes,
                batch.pending.as_ref().unwrap().bytes
            );
            single.flush().await.unwrap();
            batch.flush().await.unwrap();
            assert_eq!(budget.available(), 1024 * 1024);
        }
    }
    #[tokio::test]
    async fn cancelled_batch_resumes_cursor_and_rejects_invalid_or_overlapping_work() {
        let budget = ByteBudget::new(1024 * 1024);
        let (sender, receiver) = duplex(11);
        let mut out = Outbound::new(sender, budget.clone());
        let mut input = Inbound::new(receiver, budget.clone());
        out.enable_encryption(&[17; 16]).unwrap();
        input.enable_encryption(&[17; 16]).unwrap();
        out.enable_compression(256).unwrap();
        input.enable_compression(256).unwrap();
        for packets in [
            vec![],
            vec![vec![]],
            vec![vec![1]; BATCH_PACKETS + 1],
            vec![vec![1; BATCH_BYTES + 1]],
            vec![vec![1; BATCH_BYTES], vec![2]],
        ] {
            assert!(out.queue_batch(&packets).is_err());
            assert_eq!(budget.available(), 1024 * 1024);
        }
        let packets: Vec<_> = (0..64u8).map(|n| vec![0x10, n]).collect();
        out.queue_batch(&packets).unwrap();
        assert!(out.queue_batch(&packets).is_err());
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(5), out.flush())
                .await
                .is_err()
        );
        let (written, ()) = tokio::join!(out.flush(), async {
            for packet in &packets {
                assert_eq!(&*input.read_packet().await.unwrap(), packet.as_slice());
            }
        });
        written.unwrap();
        assert_eq!(budget.available(), 1024 * 1024);
    }
    #[tokio::test]
    async fn oversized_frame_rejected_before_body_allocation() {
        let budget = ByteBudget::new(100);
        let (mut sender, receiver) = duplex(16);
        sender.write_all(&[0x81, 8]).await.unwrap(); // 1025, over login cap
        let mut input = Inbound::new(receiver, budget.clone());
        assert_eq!(
            input.read_packet().await.err().unwrap().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(budget.available(), 100);
    }

    #[tokio::test]
    async fn malformed_compression_and_decompression_bombs_are_bounded() {
        use std::io::Write;
        let payload = vec![0; 1024];
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(&payload).unwrap();
        let compressed = encoder.finish().unwrap();
        let mut cases = vec![
            vec![0, 1, 2, 3],
            vec![0x80; 5],
            vec![0xff, 0xff, 0xff, 0xff, 0x0f],
            vec![0x80, 8, 1, 2, 3],
        ];
        for length in [1023, 1025, 65537] {
            let mut frame = Vec::new();
            encode_varint(length, &mut frame);
            frame.extend_from_slice(&compressed);
            cases.push(frame);
        }
        let mut trailing = Vec::new();
        encode_varint(1024, &mut trailing);
        trailing.extend_from_slice(&compressed);
        trailing.push(0);
        cases.push(trailing);
        for body in cases {
            let budget = ByteBudget::new(1024 * 1024);
            let (mut sender, receiver) = duplex(4096);
            let mut input = Inbound::new(receiver, budget.clone());
            input.enable_compression(1).unwrap();
            let frame = smolmc_protocol_777::frame(&body).unwrap();
            sender.write_all(&frame).await.unwrap();
            assert!(input.read_packet().await.is_err());
            drop(input);
            assert_eq!(budget.available(), 1024 * 1024);
        }
    }

    #[tokio::test]
    async fn pending_allocations_and_global_work_are_bounded() {
        let budget = ByteBudget::new(50);
        let (mut sender, receiver) = duplex(32);
        sender.write_all(&[100]).await.unwrap();
        let mut input = Inbound::new(receiver, budget.clone());
        assert_eq!(
            input.read_packet().await.err().unwrap().kind(),
            io::ErrorKind::OutOfMemory
        );
        assert_eq!(budget.available(), 50);
        let budget = ByteBudget::new(100);
        budget
            .charge_inbound(INBOUND_WORK_PER_SECOND as usize)
            .unwrap();
        assert_eq!(
            budget.charge_inbound(1).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }
}
