//! Tests: can OCCT BRep readers tolerate extra trailing bytes?
//!
//! We write a valid BRep (text and binary), append 1 KB of garbage,
//! then read it back.  If the reader only consumes what it needs
//! (count-driven parsing) and ignores trailing data, these tests pass.

use cadrum::Solid;
use glam::DVec3;

fn test_box() -> Vec<Solid> {
    vec![Solid::box_from_corners(DVec3::ZERO, DVec3::ONE)]
}

/// Write BRep binary, append 1 KB of 0xAB, read back.
#[test]
fn read_brep_binary_with_trailing_garbage() {
    let shape = test_box();

    // Write valid BRep binary
    let mut buf = Vec::new();
    cadrum::io::write_brep_binary(&shape, &mut buf).expect("write_brep_binary should succeed");
    let brep_len = buf.len();
    assert!(brep_len > 0);

    // Append 1 KB of garbage
    buf.extend_from_slice(&[0xAB; 1024]);
    assert_eq!(buf.len(), brep_len + 1024);

    // Read back — should succeed despite trailing garbage
    let result = cadrum::io::read_brep_binary(&mut buf.as_slice());
    match &result {
        Ok(solids) => {
            assert!(!solids.is_empty(), "should read at least one solid");
            let vol = solids[0].volume();
            assert!(
                (vol - 1.0).abs() < 1e-6,
                "unit box volume should be ~1.0, got {}",
                vol
            );
            println!(
                "BINARY: OK — read {} solid(s), volume={:.6}, brep_len={}, total={}",
                solids.len(),
                vol,
                brep_len,
                brep_len + 1024
            );
        }
        Err(e) => {
            println!(
                "BINARY: FAILED — {:?} (brep_len={}, total={})",
                e,
                brep_len,
                brep_len + 1024
            );
            panic!("read_brep_binary failed with trailing garbage: {:?}", e);
        }
    }
}

/// Write BRep text, append 1 KB of 'X', read back.
#[test]
fn read_brep_text_with_trailing_garbage() {
    let shape = test_box();

    // Write valid BRep text
    let mut buf = Vec::new();
    cadrum::io::write_brep_text(&shape, &mut buf).expect("write_brep_text should succeed");
    let brep_len = buf.len();
    assert!(brep_len > 0);

    // Append 1 KB of garbage (printable ASCII to avoid parser confusion)
    buf.extend_from_slice(&[b'X'; 1024]);
    assert_eq!(buf.len(), brep_len + 1024);

    // Read back — should succeed despite trailing garbage
    let result = cadrum::io::read_brep_text(&mut buf.as_slice());
    match &result {
        Ok(solids) => {
            assert!(!solids.is_empty(), "should read at least one solid");
            let vol = solids[0].volume();
            assert!(
                (vol - 1.0).abs() < 1e-6,
                "unit box volume should be ~1.0, got {}",
                vol
            );
            println!(
                "TEXT: OK — read {} solid(s), volume={:.6}, brep_len={}, total={}",
                solids.len(),
                vol,
                brep_len,
                brep_len + 1024
            );
        }
        Err(e) => {
            println!(
                "TEXT: FAILED — {:?} (brep_len={}, total={})",
                e,
                brep_len,
                brep_len + 1024
            );
            panic!("read_brep_text failed with trailing garbage: {:?}", e);
        }
    }
}
