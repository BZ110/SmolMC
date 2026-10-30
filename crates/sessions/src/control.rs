//! Cold, optional M2C control snapshots. No movement subscription or Java callback.
use smolmc_protocol_777::login::Profile;
use smolmc_world::WorldHandle;
use std::sync::Arc;
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
}
pub(crate) struct Events {
    pub tx: broadcast::Sender<Arc<ControlEvent>>,
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
