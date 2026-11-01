//! Protocol-777 supported static terrain facts; no simulation implied by a state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Material(u16);
pub struct StateInfo {
    pub id: u16,
    pub hardness: f32,
    pub requires_tool: bool,
    pub replaceable: bool,
    pub pickaxe: bool,
    pub iron_correct: bool,
    pub sword_speed: f32,
    pub name: &'static str,
    pub properties: &'static str,
    pub solid: bool,
    pub opacity: u8,
    pub item: i32,
    pub default: bool,
    pub boxes: &'static [[f64; 6]],
}
include!("block_catalog.rs");
#[allow(non_upper_case_globals)]
impl Material {
    pub const Air: Self = Self(0);
    pub const Stone: Self = Self(1);
    pub(crate) fn known(id: i32) -> Self {
        Self(id as u16)
    }
    pub fn from_state(id: i32) -> Option<Self> {
        let n = u16::try_from(id).ok()?;
        STATES
            .binary_search_by_key(&n, |s| s.id)
            .ok()
            .map(|_| Self(n))
    }
    pub fn state(self) -> i32 {
        i32::from(self.0)
    }
    pub fn info(self) -> &'static StateInfo {
        &STATES[STATES
            .binary_search_by_key(&self.0, |s| s.id)
            .expect("validated state")]
    }
    pub fn solid(self) -> bool {
        self.info().solid
    }
    pub fn air(self) -> bool {
        matches!(
            self.info().name,
            "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
        )
    }
    pub fn parse(name: &str, properties: Option<&str>) -> Option<Self> {
        STATES
            .iter()
            .find(|s| s.name == name && properties.map_or(s.default, |p| s.properties == p))
            .map(|s| Self(s.id))
    }
    pub fn placeable(self) -> bool {
        self.solid() && self.info().default && self.info().opacity == 15 && self.info().item > 0
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn supported_roundtrip_and_bounds() {
        assert_eq!(Material::from_state(0), Some(Material::Air));
        assert_eq!(Material::from_state(-1), None);
        assert_eq!(
            Material::parse("minecraft:stone", None),
            Some(Material::Stone)
        );
        for s in STATES {
            assert_eq!(
                Material::from_state(s.id as i32).unwrap().info().name,
                s.name
            );
            assert!(
                s.boxes
                    .iter()
                    .flatten()
                    .all(|n| n.is_finite() && *n >= 0. && *n <= 1.)
            );
        }
        assert!(Material::parse("minecraft:water", None).is_none());
    }
}
