#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurrencyDefinition {
    pub field: &'static str,
    pub offset: usize,
}

pub const CURRENCIES: &[CurrencyDefinition] = &[
    CurrencyDefinition {
        field: "game.glory",
        offset: 0x3d4a,
    },
    CurrencyDefinition {
        field: "game.macca",
        offset: 0x3d32,
    },
];

#[must_use]
pub fn by_field(field: &str) -> Option<CurrencyDefinition> {
    CURRENCIES
        .iter()
        .copied()
        .find(|entry| entry.field == field)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn currency_registry_has_unique_fields_and_offsets() {
        assert_ne!(CURRENCIES[0].field, CURRENCIES[1].field);
        assert_ne!(CURRENCIES[0].offset, CURRENCIES[1].offset);
    }
}
