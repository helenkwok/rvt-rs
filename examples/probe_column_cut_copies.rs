//! RE-178 probe: every copy of each 2024 column's geometry node (all partitions), as JSON lines:
//! id, record box, and per copy the partition, box@-96 z-range, box@-48 z-range and whether its plan
//! equals the record's (1e-6 ft) and its two boxes' plans are equal.
use rvt::RevitFile;
use rvt::partition_schema_mvp::{element_record_bbox, recover_partition_schema_mvp};
use rvt::walker::WalkerLimits;
use std::collections::BTreeMap;

const AFTER_ID: [u8; 16] = [
    3, 0, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255,
];

fn six(buf: &[u8], at: usize) -> [f64; 6] {
    std::array::from_fn(|i| f64::from_le_bytes(buf[at + 8 * i..at + 8 * i + 8].try_into().unwrap()))
}

fn main() {
    for path in std::env::args().skip(1) {
        let mut rf = RevitFile::open(&path).expect("open");
        let out =
            recover_partition_schema_mvp(&mut rf, 2024, WalkerLimits::default()).expect("walk");
        let cols: BTreeMap<u64, [f64; 6]> = out
            .columns
            .iter()
            .chain(out.products.iter())
            .filter(|e| matches!(e.class.as_str(), "Column" | "StructuralColumn"))
            .filter_map(|e| Some((e.id? as u64, element_record_bbox(e)?)))
            .collect();
        let mut nodes: BTreeMap<u64, Vec<serde_json::Value>> = BTreeMap::new();
        for stream in rf.partition_stream_names() {
            let part: u32 = stream
                .rsplit('/')
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let Ok(inflated) = rf.inflated_partition(&stream) else {
                continue;
            };
            let buf = inflated.bytes();
            for at in memchr::memmem::find_iter(buf, &AFTER_ID) {
                if at < 104 {
                    continue;
                }
                let id = u64::from_le_bytes(buf[at - 8..at].try_into().unwrap());
                let Some(r) = cols.get(&id) else { continue };
                let (a, b) = (six(buf, at - 104), six(buf, at - 56));
                let plan = [0, 1, 3, 4].iter().all(|&i| (b[i] - r[i]).abs() < 1e-6);
                let same = [0, 1, 3, 4].iter().all(|&i| (a[i] - b[i]).abs() < 1e-9);
                nodes.entry(id).or_default().push(serde_json::json!({"part": part,
                    "box96_z": [a[2], a[5]], "box48_z": [b[2], b[5]], "plan": plan, "same_plan": same}));
            }
        }
        for (id, r) in &cols {
            let j = serde_json::json!({"file": path, "id": id, "record": r.to_vec(),
                "copies": nodes.get(id).cloned().unwrap_or_default()});
            println!("{j}");
        }
    }
}
