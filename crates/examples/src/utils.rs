use dev_utils::ByteSize;
use loro::LoroDoc;
use std::time::{Duration, Instant};

pub fn bench_fast_snapshot(doc: &LoroDoc) {
    let old_v;
    {
        println!("======== Old snapshot mode =========");
        let start = Instant::now();
        let snapshot = doc.export_snapshot();
        let elapsed = start.elapsed();
        println!("Snapshot size: {}", ByteSize(snapshot.len()));
        println!("Export snapshot time: {:?}", elapsed);
        let start = Instant::now();
        let compressed = zstd::encode_all(snapshot.as_slice(), 0).unwrap();
        println!(
            "Snapshot size after compression: {}",
            ByteSize(compressed.len())
        );
        println!("Snapshot compression time: {:?}", start.elapsed());

        let start = Instant::now();
        let mem = dev_utils::get_mem_usage();
        let doc = LoroDoc::new();
        doc.import(&snapshot).unwrap();
        let elapsed = start.elapsed();
        println!("Import snapshot time: {:?}", elapsed);
        println!(
            "Memory usage for new doc: {}",
            dev_utils::get_mem_usage() - mem
        );

        let start = Instant::now();
        old_v = doc.get_deep_value();
        println!("Get deep value time: {:?}", start.elapsed());
    }

    {
        println!("======== New snapshot mode =========");
        let start = Instant::now();
        let snapshot = doc.export_fast_snapshot();
        let elapsed = start.elapsed();
        println!("Fast Snapshot size: {}", ByteSize(snapshot.len()));
        println!("Export fast snapshot time: {:?}", elapsed);
        let start = Instant::now();
        let compressed = zstd::encode_all(snapshot.as_slice(), 0).unwrap();
        println!(
            "Snapshot size after compression: {}",
            ByteSize(compressed.len())
        );
        println!("Snapshot compression time: {:?}", start.elapsed());

        let loop_start = Instant::now();
        let mut new_doc;
        let mem = dev_utils::get_mem_usage();
        loop {
            new_doc = LoroDoc::new();
            let start = Instant::now();
            new_doc.import(&snapshot).unwrap();
            let elapsed = start.elapsed();
            println!("Import fast snapshot time: {:?}", elapsed);
            println!(
                "Memory usage for new doc: {}",
                dev_utils::get_mem_usage() - mem
            );

            if loop_start.elapsed() > Duration::from_secs(10) {
                break;
            }
        }

        let start = Instant::now();
        let v = new_doc.get_deep_value();
        println!("Get deep value time: {:?}", start.elapsed());
        assert_eq!(v, old_v);
        println!(
            "Memory usage for new doc after getting deep value: {}",
            dev_utils::get_mem_usage() - mem
        );

        let start = Instant::now();
        let _snapshot = new_doc.export_fast_snapshot();
        let elapsed = start.elapsed();
        println!(
            "Export fast snapshot time (from doc created by fast snapshot): {:?}",
            elapsed
        );

        // let start = Instant::now();
        // new_doc.check_state_correctness_slow();
        // println!("Check state correctness duration: {:?}", start.elapsed());
    }
}