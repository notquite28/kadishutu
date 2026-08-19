#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemDefinition {
    pub field: &'static str,
    pub item_id: u16,
    pub limit: u8,
}

impl ItemDefinition {
    pub const TABLE_OFFSET: usize = 0x4c72;

    #[must_use]
    pub const fn offset(self) -> usize {
        Self::TABLE_OFFSET + self.item_id as usize
    }
}

pub const ITEMS: &[ItemDefinition] = &[
    ItemDefinition {
        field: "items.life_stone.amount",
        item_id: 1,
        limit: 50,
    },
    ItemDefinition {
        field: "items.chakra_drop.amount",
        item_id: 2,
        limit: 30,
    },
    ItemDefinition {
        field: "items.medicine.amount",
        item_id: 11,
        limit: 50,
    },
];

#[must_use]
pub fn by_field(field: &str) -> Option<ItemDefinition> {
    ITEMS.iter().copied().find(|entry| entry.field == field)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn released_items_have_unique_fields_offsets_and_positive_limits() {
        for (index, item) in ITEMS.iter().enumerate() {
            assert!(item.limit > 0);
            for other in &ITEMS[index + 1..] {
                assert_ne!(item.field, other.field);
                assert_ne!(item.offset(), other.offset());
            }
        }
    }
}
