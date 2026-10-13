//! Research probe (#745): what the record count in `Global/ElemTable` truncated to 16 bits
//! (v0.4.0's reading) declares, against the full `u32` count, and how many of the partition's
//! marker frames carry a declared id under each.
use rvt::partition_element_records as per;
use rvt::{RevitFile, elem_table};
use std::collections::BTreeSet;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).expect("usage: probe <file.rvt>");
    let mut rf = RevitFile::open(&path)?;
    let version = rf.basic_file_info()?.version;
    let marker = per::bbox_marker(version).expect("2024/2025/2026 only");
    let records = elem_table::parse_records(&mut rf)?;
    let full = elem_table::declared_ids(&records);
    let low = records.len() & 0xffff;
    let wrapped = elem_table::declared_ids(&records[..low.min(records.len())]);
    println!(
        "records(u32 count)={} low16={} wrapped={}",
        records.len(),
        low,
        records.len() > 0xffff
    );
    println!(
        "declared ids: full={} u16-truncated={}",
        full.len(),
        wrapped.len()
    );
    let (mut frames, mut in_full, mut in_wrapped) = (0usize, 0usize, 0usize);
    for stream in rf.partition_stream_names() {
        let inf = rf.inflated_partition(&stream)?;
        let buf = inf.bytes();
        let ids = |set: &BTreeSet<u32>| {
            memchr::memmem::find_iter(buf, &marker)
                .filter_map(|h| h.checked_sub(per::BBOX_MARKER_OFFSET))
                .filter(|&o| per::decode_at_with_marker(&stream, buf, o, set, &marker).is_some())
                .count()
        };
        frames += memchr::memmem::find_iter(buf, &marker).count();
        in_full += ids(&full);
        in_wrapped += ids(&wrapped);
    }
    println!(
        "marker frames={frames} decoded with full declared set={in_full} with u16-truncated set={in_wrapped}"
    );
    Ok(())
}
