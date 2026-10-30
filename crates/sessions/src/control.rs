//! Cold, optional M2C control snapshots. No movement subscription or Java callback.
use smolmc_protocol_777::login::Profile;
use smolmc_world::WorldHandle;
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, watch};

pub const EVENT_CAPACITY: usize = 128;
#[derive(Clone, Debug)]
pub struct PlayerSnapshot {
    pub entity: i32,
    pub uuid: [u8; 16],
    pub name: String,
    pub world: WorldHandle,
}
impl PlayerSnapshot {
    pub(crate) fn new(entity: i32, profile: &Profile, world: WorldHandle) -> Self {
        Self {
            entity,
            uuid: profile.uuid,
            name: profile.name.clone(),
            world,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct CombatSnapshot {
    pub health: f32,
    pub dead: bool,
    pub selected: u8,
    pub main: i32,
    pub off: i32,
}
pub struct CombatNotice {
    pub state: CombatSnapshot,
    pub source: i32,
    pub damage: f32,
}
impl std::fmt::Debug for CombatNotice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CombatNotice")
            .field("state", &self.state)
            .finish()
    }
}
impl Clone for CombatNotice {
    fn clone(&self) -> Self {
        Self {
            state: self.state,
            source: self.source,
            damage: self.damage,
        }
    }
}
#[derive(Clone, Debug)]
pub struct ControlEvent {
    pub sequence: u64,
    /// 1 create, 2 clone, 3 reset, 4 destroy, 5 edits, 6 park,
    /// 7 player join, 8 leave, 9 world committed, 10 command.
    pub kind: u8,
    pub world: WorldHandle,
    pub player: Option<PlayerSnapshot>,
    pub text: String,
    pub owner: Option<u64>,
    pub combat: Option<CombatNotice>,
}
pub(crate) struct Events {
    pub tx: broadcast::Sender<Arc<ControlEvent>>,
    // Optional control-event publication only; never covers IO or callbacks.
    pub sequence: Mutex<u64>,
    pub commands: watch::Sender<Arc<[String]>>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlError {
    World(smolmc_world::WorldError),
    InvalidRequest,
    Backpressure,
    Conflict,
    Disabled,
}
impl From<smolmc_world::WorldError> for ControlError {
    fn from(e: smolmc_world::WorldError) -> Self {
        Self::World(e)
    }
}

impl Events {
    pub fn emit(&self, mut event: ControlEvent) {
        let mut sequence = self.sequence.lock().unwrap();
        *sequence += 1;
        event.sequence = *sequence;
        let _ = self.tx.send(Arc::new(event));
    }
}

#[cfg(test)]
mod event_tests {
    use super::*;
    #[test]
    fn concurrent_control_events_keep_sequence_and_publication_order() {
        let (tx, mut rx) = broadcast::channel(128);
        let (commands, _) = watch::channel(Arc::from([]));
        let events = Events {
            tx,
            sequence: Mutex::new(0),
            commands,
        };
        std::thread::scope(|scope| {
            for _ in 0..4 {
                let events = &events;
                scope.spawn(move || {
                    for _ in 0..16 {
                        events.emit(ControlEvent {
                            sequence: 0,
                            kind: 1,
                            world: WorldHandle {
                                id: smolmc_world::WorldId(1),
                                generation: 0,
                            },
                            player: None,
                            text: String::new(),
                            owner: None,
                            combat: None,
                        });
                    }
                });
            }
        });
        for expected in 1..=64 {
            assert_eq!(rx.try_recv().unwrap().sequence, expected);
        }
        assert!(rx.try_recv().is_err());
    }
}
