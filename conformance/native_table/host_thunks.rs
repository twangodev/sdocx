const THUNK_BYTES: u64 = 32;

#[derive(Clone, Copy)]
pub(super) struct HostThunkRegion {
    start: u64,
    capacity: usize,
}

impl HostThunkRegion {
    const fn new(start: u64, capacity: usize) -> Self {
        Self { start, capacity }
    }

    pub(super) const fn start(self) -> u64 {
        self.start
    }

    pub(super) const fn capacity(self) -> usize {
        self.capacity
    }

    pub(super) const fn byte_len(self) -> u64 {
        self.capacity as u64 * THUNK_BYTES
    }

    pub(super) fn address(self, index: usize) -> u64 {
        assert!(index < self.capacity, "host thunk region capacity exceeded");
        self.start + index as u64 * THUNK_BYTES
    }
}

pub(super) const HOST_IMPORT_PRIMARY: HostThunkRegion = HostThunkRegion::new(0x0700_0000, 1024);
pub(super) const HOST_IMPORT_OVERFLOW: HostThunkRegion = HostThunkRegion::new(0x0702_0000, 1024);
pub(super) const DYNAMIC_ICU: HostThunkRegion = HostThunkRegion::new(0x0700_8000, 896);
pub(super) const PARAGRAPH_ICU: HostThunkRegion = HostThunkRegion::new(0x0700_f000, 64);
pub(super) const HOST_IMPORT_CAPACITY: usize =
    HOST_IMPORT_PRIMARY.capacity() + HOST_IMPORT_OVERFLOW.capacity();

pub(super) fn host_import_address(index: usize) -> u64 {
    if index < HOST_IMPORT_PRIMARY.capacity() {
        HOST_IMPORT_PRIMARY.address(index)
    } else {
        HOST_IMPORT_OVERFLOW.address(index - HOST_IMPORT_PRIMARY.capacity())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn all_thunk_slots_are_disjoint_and_fit_their_reserved_regions() {
        let regions = [
            HOST_IMPORT_PRIMARY,
            DYNAMIC_ICU,
            PARAGRAPH_ICU,
            HOST_IMPORT_OVERFLOW,
        ];
        let mut occupied = BTreeSet::new();
        for region in regions {
            assert_eq!(region.start() % THUNK_BYTES, 0);
            for index in 0..region.capacity() {
                let address = region.address(index);
                assert!(occupied.insert(address));
                assert!(address + 28 <= region.start() + region.byte_len());
            }
            assert!(std::panic::catch_unwind(|| region.address(region.capacity())).is_err());
        }
        assert_eq!(occupied.len(), HOST_IMPORT_CAPACITY + 896 + 64);
        assert_eq!(
            HOST_IMPORT_PRIMARY.start() + HOST_IMPORT_PRIMARY.byte_len(),
            DYNAMIC_ICU.start()
        );
        assert_eq!(
            DYNAMIC_ICU.start() + DYNAMIC_ICU.byte_len(),
            PARAGRAPH_ICU.start()
        );
        assert!(PARAGRAPH_ICU.start() + PARAGRAPH_ICU.byte_len() <= 0x0701_0000);
        assert!(HOST_IMPORT_OVERFLOW.start() >= 0x0701_0000);
    }

    #[test]
    fn import_overflow_preserves_legacy_addresses_and_rejects_exhaustion() {
        for (index, address) in [
            (0, 0x0700_0000),
            (1023, 0x0700_7fe0),
            (1024, 0x0702_0000),
            (2047, 0x0702_7fe0),
        ] {
            assert_eq!(host_import_address(index), address);
        }
        assert!(std::panic::catch_unwind(|| host_import_address(HOST_IMPORT_CAPACITY)).is_err());
        assert!(std::panic::catch_unwind(|| host_import_address(usize::MAX)).is_err());
    }
}
