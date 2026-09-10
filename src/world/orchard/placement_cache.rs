//! Bounded spatial memoization of immutable procedural placement, including empty cells.
//! Per-thread storage avoids contention between collision queries and streaming workers.
use bevy::prelude::IVec2;

pub(super) struct PlacementCache<T: Copy> {
    seed: Option<u64>,
    entries: Vec<Option<(IVec2, Option<T>)>>,
}
impl<T: Copy> Default for PlacementCache<T> {
    fn default() -> Self {
        Self {
            seed: None,
            entries: vec![None; 64 * 64],
        }
    }
}
impl<T: Copy> PlacementCache<T> {
    pub(super) fn get_or_insert(
        &mut self,
        seed: u64,
        cell: IVec2,
        build: impl FnOnce() -> Option<T>,
    ) -> Option<T> {
        static UNCACHED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if *UNCACHED.get_or_init(|| {
            std::env::var_os("HITHER_PROFILE_FOREST").is_some()
                && std::env::var_os("HITHER_PROFILE_UNCACHED_PLACEMENT").is_some()
        }) {
            return build();
        }
        if self.seed != Some(seed) {
            self.entries.fill(None);
            self.seed = Some(seed);
        }
        // Nearby cells never alias; moving across a 64-cell window replaces only
        // the corresponding old rows/columns, without a full-cache eviction burst.
        let index = cell.x.rem_euclid(64) as usize + cell.y.rem_euclid(64) as usize * 64;
        if let Some((key, value)) = self.entries[index]
            && key == cell
        {
            return value;
        }
        let value = build();
        self.entries[index] = Some((cell, value));
        value
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_cells_aliases_and_seed_changes_preserve_exact_results() {
        let mut cache = PlacementCache::default();
        assert_eq!(
            cache.get_or_insert(42, IVec2::splat(-1), || None::<u32>),
            None
        );
        assert_eq!(
            cache.get_or_insert(42, IVec2::splat(-1), || panic!("cached empty cell")),
            None
        );
        assert_eq!(
            cache.get_or_insert(42, IVec2::splat(63), || Some(7)),
            Some(7)
        );
        assert_eq!(
            cache.get_or_insert(42, IVec2::splat(-1), || Some(8)),
            Some(8)
        );
        assert_eq!(
            cache.get_or_insert(43, IVec2::splat(-1), || Some(9)),
            Some(9)
        );
    }
}
