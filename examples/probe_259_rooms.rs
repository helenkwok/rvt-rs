//! Probe for spike 259: why a room of the Nordic LCA Office files is not
//! recovered. Usage: probe_259_rooms <file.rvt> <id>...
use rvt::RevitFile;
use rvt::partition_element_records as per;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut rf = RevitFile::open(&args[1]).expect("open");
    let ids: Vec<u32> = args[2..].iter().map(|s| s.parse().unwrap()).collect();
    let recs = rvt::elem_table::parse_records(&mut rf).expect("elemtable");
    let declared = rvt::elem_table::declared_ids(&recs);
    for id in &ids {
        println!("id {id} declared={}", declared.contains(id));
    }
    if std::env::var_os("VARIANT").is_some() || ids.is_empty() {
        variant_scan(&mut rf, &declared);
        return;
    }
    let rooms = per::scan_category_records(&mut rf, 2024, per::OST_ROOMS, &declared).unwrap();
    let opts = rf.design_options();
    let mut tally = std::collections::BTreeMap::new();
    for r in &rooms {
        let key = (
            r.is_container_member(),
            format!("{:#x}", r.placement_kind),
            r.design_option.is_some(),
            opts.excludes(r),
        );
        *tally.entry(key).or_insert(0) += 1;
        if ids.contains(&r.element_id) {
            println!("ROOM REC {r:?}");
        }
    }
    println!(
        "rooms records {} tally (container, kind, option, excluded) {tally:?}",
        rooms.len()
    );
    // Every frame (any category) whose leading u64 is one of ids.
    let marker = per::file_bbox_marker(&mut rf, 2024).unwrap();
    for stream in rf.partition_stream_names() {
        let inf = rf.inflated_partition(&stream).unwrap();
        let b = inf.bytes();
        for id in &ids {
            let needle = (*id as u64).to_le_bytes();
            let mut at = 0;
            while let Some(p) = memchr::memmem::find(&b[at..], &needle) {
                let o = at + p;
                let m = b.get(o + per::BBOX_MARKER_OFFSET..o + per::BBOX_MARKER_OFFSET + 8);
                let cat = b
                    .get(o + per::CATEGORY_OFFSET..o + per::CATEGORY_OFFSET + 8)
                    .map(|x| i64::from_le_bytes(x.try_into().unwrap()));
                if std::env::var_os("RAW").is_some() {
                    let lo = o.saturating_sub(0x60);
                    let hex: String = b[lo..(o + 0xa0).min(b.len())]
                        .iter()
                        .map(|x| format!("{x:02x}"))
                        .collect();
                    println!("RAW {stream} @{o} (from {lo}) {hex}");
                }
                if m == Some(&marker[..]) {
                    let d = per::decode_at_with_marker(&stream, b, o, &declared, &marker);
                    println!(
                        "{stream} @{o} id {id} cat {cat:?} framed decoded={:?}",
                        d.map(|r| (
                            r.builtin_category,
                            r.container,
                            r.placement_kind,
                            r.design_option
                        ))
                    );
                }
                at = o + 1;
            }
        }
    }
}

/// Frames shaped like an element record up to the class tag whose marker
/// tail `ff ff ff ff ab 05` sits at +0x50 instead of +0x52 (no bbox).
#[allow(dead_code)]
pub fn variant_scan(rf: &mut RevitFile, declared: &std::collections::BTreeSet<u32>) {
    let mut tally = std::collections::BTreeMap::new();
    for stream in rf.partition_stream_names() {
        let inf = rf.inflated_partition(&stream).unwrap();
        let b = inf.bytes();
        let tail = [0xffu8, 0xff, 0xff, 0xff, 0xab, 0x05];
        let mut at = 0;
        while let Some(p) = memchr::memmem::find(&b[at..], &tail) {
            let t = at + p;
            at = t + 1;
            for (shift, name) in [(0x52usize, "std"), (0x50, "short")] {
                let Some(o) = t.checked_sub(shift) else {
                    continue;
                };
                if b.get(o + 0x0c..o + 0x0e) != Some(&[0x9f, 0x05][..]) {
                    continue;
                }
                let id = u64::from_le_bytes(b[o..o + 8].try_into().unwrap());
                if id > u32::MAX as u64 || !declared.contains(&(id as u32)) {
                    continue;
                }
                let cat = i64::from_le_bytes(b[o + 0x12..o + 0x1a].try_into().unwrap());
                let kind = u32::from_le_bytes(b[o + 0x42..o + 0x46].try_into().unwrap());
                let cont = u64::from_le_bytes(b[o + 0x32..o + 0x3a].try_into().unwrap());
                let flags = u32::from_le_bytes(b[o + 8..o + 12].try_into().unwrap());
                let pre = b[o + 0x4c..o + shift]
                    .iter()
                    .map(|x| format!("{x:02x}"))
                    .collect::<String>();
                let inst = kind == per::PLACEMENT_KIND_INSTANCE && cont == per::CONTAINER_NONE;
                *tally.entry((name, cat, inst, pre.clone())).or_insert(0) += 1;
                if name == "short" && inst {
                    println!("SHORT {stream} @{o} id {id} cat {cat} flags {flags:#x} pre {pre}");
                }
            }
        }
    }
    for (k, v) in tally {
        if k.2 {
            println!("TALLY {k:?} {v}");
        }
    }
}
