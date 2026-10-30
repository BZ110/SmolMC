//! Resolved M2B world capabilities and deterministic resource limits.
//! This engine feature set is unrelated to client protocol Feature Flags.
use crate::WorldError;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feature {
    Core,
    Worlds,
    ChunkStream,
    Players,
    Presence,
    SystemChat,
    WorldEdits,
    BlockInteractions,
    Physics,
    Mobs,
    Persistence,
    JavaBridge,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldLimits {
    pub max_edits: usize,
    pub max_modified_chunks: usize,
    pub max_overlay_bytes: usize,
    pub max_players: usize,
}
impl Default for WorldLimits {
    fn default() -> Self {
        Self {
            max_edits: 65_536,
            max_modified_chunks: 4096,
            max_overlay_bytes: 1024 * 1024,
            max_players: 512,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldProfile {
    bits: u8,
    pub limits: WorldLimits,
}
impl Default for WorldProfile {
    fn default() -> Self {
        Self::editable()
    }
}
impl WorldProfile {
    pub fn editable() -> Self {
        Self {
            bits: 127,
            limits: WorldLimits::default(),
        }
    }
    pub fn builder() -> Self {
        Self {
            bits: 255,
            limits: WorldLimits::default(),
        }
    }
    pub fn read_only() -> Self {
        Self {
            bits: 63,
            limits: WorldLimits::default(),
        }
    }
    /// Resolve dependencies before allocating world state. Required connection/
    /// world services cannot be disabled; unimplemented capabilities fail explicitly.
    pub fn resolve(requested: &[Feature], limits: WorldLimits) -> Result<Self, WorldError> {
        if limits.max_edits == 0
            || limits.max_edits > 65_536
            || limits.max_modified_chunks == 0
            || limits.max_modified_chunks > 4096
            || limits.max_overlay_bytes == 0
            || limits.max_overlay_bytes > 1024 * 1024
            || limits.max_players == 0
            || limits.max_players > 512
        {
            return Err(WorldError::InvalidBudget);
        }
        let mut profile = Self { bits: 63, limits };
        for feature in requested {
            match feature {
                Feature::Core
                | Feature::Worlds
                | Feature::ChunkStream
                | Feature::Players
                | Feature::Presence
                | Feature::SystemChat => {}
                Feature::WorldEdits => profile.bits |= 64,
                Feature::BlockInteractions => profile.bits |= 192,
                _ => return Err(WorldError::UnsupportedFeature),
            }
        }
        Ok(profile)
    }
    pub fn enabled(self, feature: Feature) -> bool {
        let bit = match feature {
            Feature::Core => 1,
            Feature::Worlds => 2,
            Feature::ChunkStream => 4,
            Feature::Players => 8,
            Feature::Presence => 16,
            Feature::SystemChat => 32,
            Feature::WorldEdits => 64,
            Feature::BlockInteractions => 128,
            _ => 0,
        };
        bit != 0 && self.bits & bit != 0
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dependencies_resolve_and_unavailable_features_do_not_initialize() {
        let p = WorldProfile::resolve(&[Feature::WorldEdits], WorldLimits::default()).unwrap();
        for f in [
            Feature::Core,
            Feature::Worlds,
            Feature::ChunkStream,
            Feature::Players,
            Feature::Presence,
            Feature::SystemChat,
            Feature::WorldEdits,
        ] {
            assert!(p.enabled(f));
        }
        assert!(!WorldProfile::read_only().enabled(Feature::WorldEdits));
        for f in [
            Feature::JavaBridge,
            Feature::Physics,
            Feature::Mobs,
            Feature::Persistence,
        ] {
            assert_eq!(
                WorldProfile::resolve(&[f], WorldLimits::default()),
                Err(WorldError::UnsupportedFeature)
            );
        }
        assert_eq!(
            WorldProfile::resolve(
                &[],
                WorldLimits {
                    max_players: 0,
                    ..WorldLimits::default()
                }
            ),
            Err(WorldError::InvalidBudget)
        );
    }
}
