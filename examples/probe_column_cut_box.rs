//! Spike 260 (bimcode): each 2024 column's own extent and its extent after geometry joins.
//! Geometry node (RE-173): u64 ElementId | u64 3 | u64 -1, boxes of six f64 at id-96 and id-48.
//! Prints JSON lines: id, record box, and for the node in the highest-numbered partition whose plan
//! extent equals the record's, the z-range of both boxes (box48 = Revit's body after joins).
use rvt::RevitFile;
use rvt::partition_schema_mvp::{element_record_bbox, recover_partition_schema_mvp};
use rvt::walker::WalkerLimits;
use std::collections::BTreeMap;

const AFTER_ID: [u8; 16] = [3, 0, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255];

fn six(buf: &[u8], at: usize) -> [f64; 6] {
    std::array::from_fn(|i| f64::from_le_bytes(buf[at + 8 * i..at + 8 * i + 8].try_into().unwrap()))
}

fn main() {
    for path in std::env::args().skip(1) {
        let mut rf = RevitFile::open(&path).expect("open");
        let out = recover_partition_schema_mvp(&mut rf, 2024, WalkerLimits::default()).expect("walk");
        let cols: BTreeMap<u64, [f64; 6]> = out.columns.iter().chain(out.products.iter())
            .filter(|e| matches!(e.class.as_str(), "Column" | "StructuralColumn"))
            .filter_map(|e| Some((e.id? as u64, element_record_bbox(e)?)))
            .collect();
        // id -> [(partition number, box96, box48)]
        let mut nodes: BTreeMap<u64, Vec<(u32, [f64; 6], [f64; 6])>> = BTreeMap::new();
        for stream in rf.partition_stream_names() {
            let part: u32 = stream.rsplit('/').next().and_then(|s| s.parse().ok()).unwrap_or(0);
            let Ok(inflated) = rf.inflated_partition(&stream) else { continue };
            let buf = inflated.bytes();
            for at in memchr::memmem::find_iter(buf, &AFTER_ID) {
                if at < 104 { continue; }
                let id = u64::from_le_bytes(buf[at - 8..at].try_into().unwrap());
                let Some(r) = cols.get(&id) else { continue };
                let (a, b) = (six(buf, at - 104), six(buf, at - 56));
                if [0, 1, 3, 4].iter().all(|&i| (b[i] - r[i]).abs() < 1e-6) {
                    nodes.entry(id).or_default().push((part, a, b));
                }
            }
        }
        for (id, r) in &cols {
            let ns = nodes.get(id).cloned().unwrap_or_default();
            let top = ns.iter().map(|n| n.0).max();
            let pick: Vec<_> = ns.iter().filter(|n| Some(n.0) == top).collect();
            let j = serde_json::json!({"file": path, "id": id, "record": r.to_vec(), "nodes": ns.len(),
                "partitions": ns.iter().map(|n| n.0).collect::<std::collections::BTreeSet<_>>(),
                "box96_z": pick.first().map(|n| [n.1[2], n.1[5]]), "box48_z": pick.first().map(|n| [n.2[2], n.2[5]]),
                "agree": pick.windows(2).all(|w| (w[0].2[2] - w[1].2[2]).abs() < 0.01 && (w[0].2[5] - w[1].2[5]).abs() < 0.01)});
            println!("{j}");
        }
    }
}
