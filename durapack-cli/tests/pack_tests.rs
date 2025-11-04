use std::fs;
use tempfile::tempdir;

use durapack_cli::{commands::pack, ChunkStrategy};
use durapack_core::constants::TrailerType;
use durapack_core::scanner::scan_stream;

fn write_file<P: AsRef<std::path::Path>>(p: P, s: &str) {
    fs::write(p, s.as_bytes()).unwrap();
}

#[test]
fn pack_json_array_crc32c_basic() {
    let td = tempdir().unwrap();
    let in_path = td.path().join("in.json");
    let out_path = td.path().join("out.durp");

    let input = r#"[
      {"a":1},
      {"b":2},
      {"c":3}
    ]"#;
    write_file(&in_path, input);

    pack::execute_ext(
        in_path.to_str().unwrap(),
        out_path.to_str().unwrap(),
        /*use_blake3*/ false,
        /*start_id*/ 1,
        /*jsonl*/ false,
        ChunkStrategy::Aggregate,
        /*rate_limit*/ None,
        /*progress*/ false,
        /*fec_rs*/ None,
        /*fec_index_out*/ None,
        /*sign_key_path*/ None,
    )
    .unwrap();

    let bytes = fs::read(&out_path).unwrap();
    let frames = scan_stream(&bytes);
    assert_eq!(frames.len(), 3);
    for lf in frames {
        assert_eq!(lf.frame.header.flags.trailer_type(), TrailerType::Crc32c);
    }
}

#[test]
fn pack_jsonl_blake3_with_progress() {
    let td = tempdir().unwrap();
    let in_path = td.path().join("in.jsonl");
    let out_path = td.path().join("out_b3.durp");

    let lines = "{\"x\":10}\n{\"y\":20}\n{\"z\":30}\n";
    write_file(&in_path, lines);

    pack::execute_ext(
        in_path.to_str().unwrap(),
        out_path.to_str().unwrap(),
        /*use_blake3*/ true,
        /*start_id*/ 100,
        /*jsonl*/ true,
        ChunkStrategy::Jsonl,
        /*rate_limit*/ None,
        /*progress*/ true,
        /*fec_rs*/ None,
        /*fec_index_out*/ None,
        /*sign_key_path*/ None,
    )
    .unwrap();

    let bytes = fs::read(&out_path).unwrap();
    let frames = scan_stream(&bytes);
    assert_eq!(frames.len(), 3);
    for lf in frames {
        assert_eq!(lf.frame.header.flags.trailer_type(), TrailerType::Blake3);
    }
}

#[test]
fn pack_with_sign_flag_and_exportable_trailer() {
    let td = tempdir().unwrap();
    let in_path = td.path().join("in.json");
    let out_path = td.path().join("out_sig.durp");
    let key_path = td.path().join("sk.bin");

    // 32-byte file; if feature is enabled, it's used; otherwise it is ignored.
    fs::write(&key_path, [7u8; 32]).unwrap();

    let input = "[{\"m\":1},{\"n\":2}]";
    write_file(&in_path, input);

    pack::execute_ext(
        in_path.to_str().unwrap(),
        out_path.to_str().unwrap(),
        /*use_blake3*/ false,
        /*start_id*/ 1,
        /*jsonl*/ false,
        ChunkStrategy::Aggregate,
        /*rate_limit*/ None,
        /*progress*/ false,
        /*fec_rs*/ None,
        /*fec_index_out*/ None,
        /*sign_key_path*/ Some(key_path.to_str().unwrap()),
    )
    .unwrap();

    let bytes = fs::read(&out_path).unwrap();
    let frames = scan_stream(&bytes);
    assert_eq!(frames.len(), 2);
    // Regardless of feature, builder sets the combined trailer when sign flag is present.
    for lf in frames {
        assert_eq!(
            lf.frame.header.flags.trailer_type(),
            TrailerType::Blake3WithEd25519Sig
        );
    }
}

#[test]
fn pack_writes_empty_fec_sidecar_without_feature() {
    let td = tempdir().unwrap();
    let in_path = td.path().join("in.json");
    let out_path = td.path().join("out_fec.durp");
    let sidecar = td.path().join("out_fec.durp.fec.json");

    let input = "[{\"q\":1},{\"r\":2},{\"s\":3},{\"t\":4}]";
    write_file(&in_path, input);

    pack::execute_ext(
        in_path.to_str().unwrap(),
        out_path.to_str().unwrap(),
        /*use_blake3*/ true,
        /*start_id*/ 1,
        /*jsonl*/ false,
        ChunkStrategy::Aggregate,
        /*rate_limit*/ None,
        /*progress*/ false,
        /*fec_rs*/ Some((2, 1)),
        /*fec_index_out*/ Some(sidecar.to_str().unwrap()),
        /*sign_key_path*/ None,
    )
    .unwrap();

    // Sidecar should exist; without fec feature, it's valid JSON but typically empty array
    let sc = fs::read_to_string(&sidecar).unwrap();
    let v: serde_json::Value = serde_json::from_str(&sc).unwrap();
    assert!(v.is_array());
}
