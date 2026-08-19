mod support;

use kadishutu::{
    crypto,
    format::{
        bytes::ByteView,
        detect::{CheckStatus, InputKind, validate_bytes},
        game_save::{FieldValue, SaveDocument, evidence_catalog},
        unreal,
    },
    integrity,
};
use proptest::prelude::*;

#[test]
fn synthetic_pc_profile_passes_all_validation() {
    let bytes = support::synthetic::valid_pc_profile();
    let report = validate_bytes(&bytes);
    assert_eq!(report.input_kind, InputKind::Decrypted);
    assert_eq!(report.length, CheckStatus::Pass);
    assert_eq!(report.gvas, CheckStatus::Pass);
    assert_eq!(report.structure, CheckStatus::Pass);
    assert_eq!(report.sha1, CheckStatus::Pass);
    assert_eq!(
        report.profile.map(|profile| profile.to_string()).as_deref(),
        Some("smtvv-pc-gamesave-449680")
    );
}

#[test]
fn encrypted_pc_profile_is_recognized_only_after_full_validation() {
    let plaintext = support::synthetic::valid_pc_profile();
    let ciphertext = crypto::encrypt(&plaintext).unwrap();
    let report = validate_bytes(&ciphertext);
    assert_eq!(report.input_kind, InputKind::Encrypted);
    assert!(report.is_valid());
    assert_eq!(
        report.profile.map(|profile| profile.to_string()).as_deref(),
        Some("smtvv-pc-gamesave-449680")
    );
}

#[test]
fn encrypted_bad_hash_and_unsupported_structure_remain_unrecognized() {
    let mut bad_hash = support::synthetic::valid_pc_profile();
    bad_hash[0] ^= 1;
    let report = validate_bytes(&crypto::encrypt(&bad_hash).unwrap());
    assert_eq!(report.input_kind, InputKind::Unrecognized);
    assert!(report.profile.is_none());
    assert_eq!(report.sha1, CheckStatus::NotRun);

    let mut unsupported = support::synthetic::valid_pc_profile();
    unsupported[0x75] ^= 1;
    support::synthetic::update_hash(&mut unsupported);
    let report = validate_bytes(&crypto::encrypt(&unsupported).unwrap());
    assert_eq!(report.input_kind, InputKind::Unrecognized);
    assert!(report.profile.is_none());
    assert_eq!(report.structure, CheckStatus::NotRun);
}

#[test]
fn changed_custom_version_entry_does_not_match_pc_profile() {
    let mut bytes = support::synthetic::valid_pc_profile();
    bytes[0x75] ^= 1;
    support::synthetic::update_hash(&mut bytes);
    let report = validate_bytes(&bytes);
    assert_eq!(report.gvas, CheckStatus::Pass);
    assert_eq!(report.structure, CheckStatus::Fail);
    assert_eq!(report.sha1, CheckStatus::NotRun);
    assert!(report.profile.is_none());
}

#[test]
fn byte_view_checks_every_primitive_boundary() {
    let bytes = [1, 2, 3, 4, 5, 6, 7, 8];
    let view = ByteView::new(&bytes);
    assert_eq!(view.u8(7).unwrap(), 8);
    assert_eq!(view.u16_le(6).unwrap(), 0x0807);
    assert_eq!(view.i16_le(6).unwrap(), 0x0807);
    assert_eq!(view.u32_le(4).unwrap(), 0x0807_0605);
    assert_eq!(view.u64_le(0).unwrap(), 0x0807_0605_0403_0201);
    assert!(view.f32_le(4).unwrap().is_finite());
    assert!(view.bit(0, 0).unwrap());
    assert!(view.u16_le(7).is_err());
    assert!(view.range(usize::MAX, 2).is_err());
    assert!(view.bit(0, 8).is_err());
}

#[test]
fn fstring_supports_utf8_utf16_and_zero_lengths() {
    let utf8 = [4, 0, 0, 0, b'a', b'b', b'c', 0];
    assert_eq!(
        ByteView::new(&utf8).fstring(0).unwrap(),
        ("abc".to_owned(), 8)
    );

    let utf16 = [0xfd, 0xff, 0xff, 0xff, b'A', 0, b'B', 0, 0, 0];
    assert_eq!(
        ByteView::new(&utf16).fstring(0).unwrap(),
        ("AB".to_owned(), 10)
    );

    assert_eq!(
        ByteView::new(&[0, 0, 0, 0]).fstring(0).unwrap(),
        (String::new(), 4)
    );
    assert!(ByteView::new(&[2, 0, 0, 0, b'x', b'y']).fstring(0).is_err());
    assert!(
        ByteView::new(&[0xfe, 0xff, 0xff, 0xff, 0, 0xd8, 0, 0])
            .fstring(0)
            .is_err()
    );
    assert!(ByteView::new(&[2, 0, 0, 0, 0xff, 0]).fstring(0).is_err());
    assert!(ByteView::new(&[0, 0, 0, 0x80]).fstring(0).is_err());
}

#[test]
fn sha1_uses_exact_gamesave_coverage() {
    let mut bytes = vec![0_u8; 67];
    bytes[64..].copy_from_slice(b"abc");
    let digest = integrity::calculate_sha1(&bytes).unwrap();
    assert_eq!(hex(&digest), "a9993e364706816aba3e25717850c26c9cd0d89d");
    bytes[..20].copy_from_slice(&digest);
    assert!(integrity::validate_sha1(&bytes).unwrap());
    bytes[63] = 1;
    assert!(integrity::validate_sha1(&bytes).unwrap());
    bytes[64] = b'z';
    assert!(!integrity::validate_sha1(&bytes).unwrap());
}

#[test]
fn unreal_parser_reads_count_before_entries() {
    let bytes = support::synthetic::valid_pc_profile();
    let header = unreal::parse_header(&bytes).unwrap();
    assert_eq!(header.custom_versions.len(), 55);
    assert_eq!(
        header.custom_versions[0].guid,
        [
            0x78, 0x20, 0x2c, 0x12, 0xb6, 0x72, 0x72, 0x95, 0x9a, 0xbb, 0x66, 0xb1, 0x9e, 0xd2,
            0x9a, 0xf4,
        ]
    );
    assert_eq!(header.custom_versions[0].version, 2);
}

#[test]
fn embedded_catalog_and_reader_map_are_consistent() {
    let catalog = evidence_catalog().unwrap();
    assert!(catalog.get("save.profile").unwrap().readable());
    assert!(catalog.get("game.macca").unwrap().readable());
    assert!(catalog.get("player.name.first").unwrap().sensitive);
    let ids = catalog
        .fields()
        .map(|field| field.id.as_str())
        .collect::<Vec<_>>();
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn released_essence_reader_reports_present_absent_split_and_unknown_bits() {
    let mut bytes = support::synthetic::valid_pc_profile();
    let owned = 0x4ea0;
    let metadata = 0x5220;

    for (amount, flags, expected_present, expected_consistent) in [
        (1, 0x06, true, true),
        (0, 0x16, false, true),
        (1, 0x16, false, false),
        (0, 0x86, true, false),
    ] {
        bytes[owned] = amount;
        bytes[metadata] = flags;
        support::synthetic::update_hash(&mut bytes);
        let document = SaveDocument::open(bytes.clone()).unwrap();
        let FieldValue::Essence(state) = document.read("essences.aogami_type_c.owned").unwrap()
        else {
            panic!("essence field returned the wrong value type");
        };
        assert_eq!(state.amount, amount);
        assert_eq!(state.metadata, flags);
        assert_eq!(state.fusion_available, amount != 0);
        assert_eq!(state.main_menu_present, expected_present);
        assert_eq!(state.consistent, expected_consistent);
        assert_eq!(state.owned_flag, flags & 0x04 != 0);
        assert_eq!(state.new, flags & 0x02 != 0);
    }
}

#[test]
fn released_essence_reader_accepts_valid_encrypted_save() {
    let mut plaintext = support::synthetic::valid_pc_profile();
    plaintext[0x4eac] = 1;
    plaintext[0x522c] = 0x06;
    support::synthetic::update_hash(&mut plaintext);
    let document = SaveDocument::open(crypto::encrypt(&plaintext).unwrap()).unwrap();
    let FieldValue::Essence(state) = document.read("essences.nozuchi.owned").unwrap() else {
        panic!("essence field returned the wrong value type");
    };
    assert!(state.consistent);
    assert!(state.fusion_available);
    assert!(state.main_menu_present);
}

#[test]
fn currency_reader_returns_exact_u32_values_from_encrypted_save() {
    let mut plaintext = support::synthetic::valid_pc_profile();
    plaintext[0x3d32..0x3d36].copy_from_slice(&8_207_492_u32.to_le_bytes());
    plaintext[0x3d4a..0x3d4e].copy_from_slice(&72_u32.to_le_bytes());
    support::synthetic::update_hash(&mut plaintext);
    let document = SaveDocument::open(crypto::encrypt(&plaintext).unwrap()).unwrap();
    assert_eq!(
        document.read("game.macca").unwrap(),
        FieldValue::Integer(8_207_492)
    );
    assert_eq!(
        document.read("game.glory").unwrap(),
        FieldValue::Integer(72)
    );
}
proptest! {
    #[test]
    fn detection_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..500_000)) {
        let _ = validate_bytes(&bytes);
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
