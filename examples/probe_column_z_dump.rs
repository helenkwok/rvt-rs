//! Dump every 2024 column's id, class, record box and decoded z as JSON lines (bimcode spike 258: compare with Revit's IFC export).
use rvt::RevitFile;
use rvt::partition_schema_mvp::{element_record_bbox, recover_partition_schema_mvp};
use rvt::walker::{InstanceField, WalkerLimits};

fn main() {
    for path in std::env::args().skip(1) {
        let mut rf = RevitFile::open(&path).expect("open");
        let out = recover_partition_schema_mvp(&mut rf, 2024, WalkerLimits::default()).expect("walk");
        for e in out.columns.iter().chain(out.products.iter()) {
            if !matches!(e.class.as_str(), "Column" | "StructuralColumn") {
                continue;
            }
            let f = |w: &str| e.fields.iter().find_map(|(n, v)| match v {
                InstanceField::Float { value, .. } if n == w => Some(*value),
                _ => None,
            });
            let src = e.fields.iter().any(|(n, _)| n == "m_column_z_source");
            let j = serde_json::json!({"file": path, "id": e.id, "class": e.class,
                "z": f("m_locationZ"), "h": f("m_bboxHeight"),
                "bbox": element_record_bbox(e).map(|b| b.to_vec()), "node": src});
            println!("{j}");
        }
    }
}
