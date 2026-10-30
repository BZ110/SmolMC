//! Per-session controls/interest; no terrain authority or fixed-rate simulation.
use smolmc_protocol_777::play::Movement;
use smolmc_world::{ChunkPos, ChunkView, HORIZONTAL_BOUND, SPAWN, SURFACE};
use std::{io, time::Duration};
use tokio::time::Instant;

pub const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(12);
pub const RESPONSE_TIMEOUT: Duration = Duration::from_secs(20);
pub const ACK_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_BATCH: usize = 4;
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub struct Player {
    pub view: ChunkView,
    pub position: [f64; 3],
    pub rotation: [f32; 2],
    pub flying: bool,
    pub on_ground: bool,
    pub crouching: bool,
    pub sprinting: bool,
    pub pending_teleport: Option<(i32, Instant)>,
    pub pending_batch: Option<Instant>,
    pub keepalive: Option<(i64, Instant)>,
    pub next_keepalive: Instant,
    pub next_batch: Instant,
    pub batch_size: usize,
    travel_credit: f64,
    last_move: Instant,
    nonce: i64,
    chat_credit: f64,
    chat_at: Instant,
}
impl Player {
    pub fn new(now: Instant) -> Self {
        Self {
            view: ChunkView::default(),
            position: SPAWN,
            rotation: [0.0; 2],
            flying: false,
            on_ground: false,
            crouching: false,
            sprinting: false,
            pending_teleport: Some((1, now + ACK_TIMEOUT)),
            pending_batch: None,
            keepalive: None,
            next_keepalive: now + KEEPALIVE_INTERVAL,
            next_batch: now,
            batch_size: MAX_BATCH,
            travel_credit: 32.0,
            last_move: now,
            nonce: 0,
            chat_credit: 3.0,
            chat_at: now,
        }
    }
    pub fn switch_world(&mut self, now: Instant, id: i32) {
        self.view = ChunkView::default();
        self.position = SPAWN;
        self.rotation = [0.; 2];
        self.crouching = false;
        self.sprinting = false;
        self.pending_teleport = Some((id, now + ACK_TIMEOUT));
        self.next_batch = now;
        self.travel_credit = 32.;
        self.last_move = now;
        // Keepalive and chat credit belong to the connection, not the world.
    }
    pub fn next_deadline(&self) -> Instant {
        let mut next = self.keepalive.map_or(self.next_keepalive, |(_, due)| due);
        if let Some((_, due)) = self.pending_teleport {
            next = next.min(due);
        }
        if let Some(due) = self.pending_batch {
            next = next.min(due);
        } else if self.view.next_missing().is_some() {
            next = next.min(self.next_batch);
        }
        next
    }
    /// Admit chat before it can consume shared replication ingress. The
    /// directory independently limits accepted fanout as defense in depth.
    pub fn admit_chat(&mut self, now: Instant) -> io::Result<()> {
        self.chat_credit =
            (self.chat_credit + now.saturating_duration_since(self.chat_at).as_secs_f64()).min(3.0);
        self.chat_at = now;
        if self.chat_credit < 1.0 {
            return Err(invalid("chat sender rate exceeded"));
        }
        self.chat_credit -= 1.0;
        Ok(())
    }
    pub fn check_deadlines(&self, now: Instant) -> io::Result<()> {
        if self.pending_teleport.is_some_and(|(_, due)| now >= due)
            || self.pending_batch.is_some_and(|due| now >= due)
            || self.keepalive.is_some_and(|(_, due)| now >= due)
        {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Play acknowledgement deadline exceeded",
            ));
        }
        Ok(())
    }
    pub fn teleport_ack(
        &mut self,
        id: i32,
        position: [f64; 3],
        rotation: [f32; 2],
    ) -> io::Result<()> {
        if self.pending_teleport.map(|(expected, _)| expected) != Some(id) {
            return Err(invalid("unexpected teleport acknowledgement"));
        }
        if position
            .iter()
            .zip(self.position)
            .any(|(a, b)| !a.is_finite() || (a - b).abs() > 0.001)
            || rotation
                .iter()
                .zip(self.rotation)
                .any(|(a, b)| !a.is_finite() || (a - b).abs() > 0.001)
        {
            return Err(invalid(
                "teleport acknowledgement does not match destination",
            ));
        }
        self.pending_teleport = None;
        Ok(())
    }
    pub fn keepalive_due(&self, now: Instant) -> bool {
        self.keepalive.is_none() && now >= self.next_keepalive
    }
    pub fn issue_keepalive(&mut self, now: Instant) -> i64 {
        self.nonce += 1;
        self.keepalive = Some((self.nonce, now + RESPONSE_TIMEOUT));
        self.nonce
    }
    pub fn keepalive_ack(&mut self, id: i64, now: Instant) -> io::Result<()> {
        if self.keepalive.map(|(expected, _)| expected) != Some(id) {
            return Err(invalid("unexpected keepalive response"));
        }
        self.keepalive = None;
        self.next_keepalive = now + KEEPALIVE_INTERVAL;
        Ok(())
    }
    pub fn batch_ready(&self, now: Instant) -> bool {
        self.pending_batch.is_none() && now >= self.next_batch && self.view.next_missing().is_some()
    }
    pub fn batch_sent(&mut self, now: Instant) {
        self.pending_batch = Some(now + ACK_TIMEOUT);
    }
    pub fn batch_ack(&mut self, desired: f32, now: Instant) -> io::Result<()> {
        if self.pending_batch.take().is_none() || !desired.is_finite() || desired <= 0.0 {
            return Err(invalid("unexpected or invalid chunk batch response"));
        }
        // Respect a slow peer without allocating more work. At most four chunks
        // in one batch and one batch in flight; no timer when the view is complete.
        self.batch_size = (desired.ceil() as usize).clamp(1, MAX_BATCH);
        let seconds = (1.0 / f64::from(desired.clamp(0.1, 4.0))).max(1.0) * 0.05;
        self.next_batch = now + Duration::from_secs_f64(seconds);
        Ok(())
    }
    /// Return true only when authoritative chunk interest changed. Client on-ground
    /// and input flags are never world authority. No collision/physics simulation.
    pub fn movement(&mut self, movement: Movement, now: Instant) -> io::Result<bool> {
        if self.pending_teleport.is_some() {
            return Ok(false);
        }
        self.on_ground = movement.flags & 1 != 0;
        if let Some(rotation) = movement.rotation {
            if !rotation.iter().all(|v| v.is_finite()) || rotation[1].abs() > 90.01 {
                return Err(invalid("invalid rotation"));
            }
            self.rotation = [rotation[0] % 360.0, rotation[1]];
        }
        let Some(position) = movement.position else {
            return Ok(false);
        };
        if !position.iter().all(|v| v.is_finite())
            || position[0].abs() > HORIZONTAL_BOUND
            || position[2].abs() > HORIZONTAL_BOUND
            || position[1] < f64::from(SURFACE) - 0.01
            || position[1] > 512.0
        {
            return Err(invalid("position outside M1C bounds"));
        }
        self.travel_credit = (self.travel_credit
            + now.saturating_duration_since(self.last_move).as_secs_f64() * 64.0)
            .min(32.0);
        self.last_move = now;
        let distance = position
            .iter()
            .zip(self.position)
            .map(|(a, b)| (a - b) * (a - b))
            .sum::<f64>()
            .sqrt();
        if distance > self.travel_credit + 0.01 {
            return Err(invalid("movement exceeds M1C travel budget"));
        }
        self.travel_credit = (self.travel_credit - distance).max(0.0);
        self.position = position;
        let center = ChunkPos::from_position(position[0], position[2])
            .ok_or_else(|| invalid("invalid interest center"))?;
        let changed = center != self.view.center();
        self.view.set_center(center);
        Ok(changed)
    }
    pub fn replica(&self) -> super::replication::Replica {
        super::replication::Replica {
            state: smolmc_protocol_777::presence::EntitySnapshot {
                position: self.position,
                rotation: self.rotation,
                on_ground: self.on_ground,
                crouching: self.crouching,
                sprinting: self.sprinting,
            },
            view: self.view,
            ready: self.pending_teleport.is_none(),
            version: 0,
            #[cfg(test)]
            at: Instant::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ack_matching_deadlines_and_no_idle_batch_timer() {
        let now = Instant::now();
        let mut player = Player::new(now);
        for _ in 0..3 {
            player.admit_chat(now).unwrap();
        }
        assert!(player.admit_chat(now).is_err());
        player.admit_chat(now + Duration::from_secs(1)).unwrap();
        assert!(player.teleport_ack(2, SPAWN, [0.0; 2]).is_err());
        assert!(player.teleport_ack(1, [8.0, 999.0, 8.0], [0.0; 2]).is_err());
        player.teleport_ack(1, SPAWN, [0.0; 2]).unwrap();
        assert!(player.keepalive_ack(1, now).is_err());
        let nonce = player.issue_keepalive(now);
        assert!(player.keepalive_ack(nonce + 1, now).is_err());
        player.keepalive_ack(nonce, now).unwrap();
        player.batch_sent(now);
        assert!(player.batch_ack(f32::NAN, now).is_err());
        player.batch_sent(now);
        player.batch_ack(0.1, now).unwrap();
        assert_eq!(player.batch_size, 1);
        while let Some(pos) = player.view.next_missing() {
            assert!(player.view.insert(pos));
        }
        assert_eq!(player.next_deadline(), player.next_keepalive);
        player.batch_sent(now);
        assert!(player.check_deadlines(now + ACK_TIMEOUT).is_err());
    }
    #[test]
    fn movement_is_finite_bounded_and_requires_confirmation() {
        let now = Instant::now();
        let mut player = Player::new(now);
        let event = Movement {
            position: Some([16.1, 65.0, 8.0]),
            rotation: None,
            flags: 0,
        };
        assert!(!player.movement(event, now).unwrap());
        assert_eq!(player.position, SPAWN);
        player.teleport_ack(1, SPAWN, [0.0; 2]).unwrap();
        assert!(player.movement(event, now).unwrap());
        for pos in [[f64::NAN, 65.0, 8.0], [16.0, 0.0, 8.0], [9999.0, 65.0, 8.0]] {
            assert!(
                player
                    .movement(
                        Movement {
                            position: Some(pos),
                            ..event
                        },
                        now
                    )
                    .is_err()
            );
        }
        assert!(
            player
                .movement(
                    Movement {
                        position: None,
                        rotation: Some([0.0, 100.0]),
                        flags: 0
                    },
                    now
                )
                .is_err()
        );
    }
}
