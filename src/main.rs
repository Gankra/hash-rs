#![feature(test)]
#![cfg_attr(test, feature(hashmap_hasher))]
#![allow(unused_imports, dead_code)]

extern crate twox_hash;
// extern crate murmurhash64;
extern crate murmurhash3;
extern crate farmhash;
extern crate fnv as _fnv;
extern crate blake2_rfc;
extern crate test;
extern crate regex;
extern crate gnuplot;

use std::process::Command;
use std::io::Result as IoResult;
use std::fs::File;
use regex::Regex;

use std::io::prelude::*;
use std::collections::HashMap;

#[cfg(not(test))]
fn main() {
    do_it().unwrap();
}

struct DataPoint {
    magnitude: u64,
    average: u64,
    variance: u64,
    throughput: u64,
}

fn do_it() -> IoResult<()> {
    let output = try!(Command::new("cargo")
        .arg("bench")
        .output());

    let stdout = String::from_utf8(output.stdout).unwrap();

    let re = Regex::new(r#"test (.*)::(.*)_(\d*) .*bench:\s*(.*) ns/iter \(\+/- (.*)\) = (\d*) MB/s.*"#).unwrap();

    println!("Output:");


    let mut data = HashMap::new();

    for cap in re.captures_iter(&stdout) {
        println!("{}", cap.at(0).unwrap());
        let hasher = String::from(cap.at(1).unwrap());
        let bench_class = String::from(cap.at(2).unwrap());

        data.entry(bench_class)
            .or_insert(HashMap::new())
            .entry(hasher)
            .or_insert(vec![])
            .push(DataPoint {
                magnitude:  cap.at(3).unwrap().split(",").collect::<String>().parse().unwrap(),
                average:    cap.at(4).unwrap().split(",").collect::<String>().parse().unwrap(),
                variance:   cap.at(5).unwrap().split(",").collect::<String>().parse().unwrap(),
                throughput: cap.at(6).unwrap().split(",").collect::<String>().parse().unwrap(),
            });
    }




    for (bench_class, hashers) in &data {
        let mut time_data = try!(File::create(&format!("{}-time.csv", bench_class)));
        let mut tput_data = try!(File::create(&format!("{}-throughput.csv", bench_class)));

        write!(&mut time_data, "bytes").unwrap();
        write!(&mut tput_data, "bytes").unwrap();

        let mut transposer = vec![];

        for (hasher, points) in hashers {
            transposer.push(points);
            write!(&mut time_data, ",{}", hasher).unwrap();
            write!(&mut tput_data, ",{}", hasher).unwrap();
        }

        write!(&mut time_data, "\n").unwrap(); write!(&mut tput_data, "\n").unwrap();

        let len = transposer[0].len();
        for i in 0..len {
            write!(&mut time_data, "{}", transposer[0][i].magnitude).unwrap();
            write!(&mut tput_data, "{}", transposer[0][i].magnitude).unwrap();

            for points in &transposer {
                let point = &points[i];
                write!(&mut time_data, ",{}", point.average).unwrap();
                // write!(&mut time_data, ",{}", point.variance).unwrap();

                write!(&mut tput_data, ",{}", point.throughput).unwrap();
            }
            write!(&mut time_data, "\n").unwrap(); write!(&mut tput_data, "\n").unwrap();
        }
    }

    Ok(())
}



macro_rules! hash_benches {
    ($Impl: ty) => {
        use std::hash::SipHasher as Sip;
        use twox_hash::XxHash as Xx;
        // use murmurhash64 as murmur2;
        // use murmurhash3::Murmur3State as Murmur3State;
        use blake2_rfc::blake2b::Blake2b;
        use blake2_rfc::blake2s::Blake2s;
        use _fnv::FnvHasher as Fnv;
        use farmhash::FarmHasher as Farm;
        use test::{black_box, Bencher};
        use std::hash::Hasher;
        use std::collections::hash_state::{DefaultState, HashState};

        pub type B<'a> = &'a mut Bencher;

        fn hasher_bench<H>(b: B, len: usize)
        where H: Hasher + Default
        {
            let hash_state = DefaultState::<H>::default();
            let bytes: Vec<_> = (0..100).cycle().take(len).collect();
            let bytes = black_box(bytes);

            b.bytes = bytes.len() as u64;
            b.iter(|| {
                let mut hasher = hash_state.hasher();
                hasher.write(&bytes);
                hasher.finish()
            });
        }

        #[bench] fn bytes_000000001(b: B) { hasher_bench::<$Impl>(b, 1) }
        #[bench] fn bytes_000000002(b: B) { hasher_bench::<$Impl>(b, 2) }
        #[bench] fn bytes_000000004(b: B) { hasher_bench::<$Impl>(b, 4) }
        #[bench] fn bytes_000000008(b: B) { hasher_bench::<$Impl>(b, 8) }
        #[bench] fn bytes_000000016(b: B) { hasher_bench::<$Impl>(b, 16) }
        #[bench] fn bytes_000000032(b: B) { hasher_bench::<$Impl>(b, 32) }
        #[bench] fn bytes_000000064(b: B) { hasher_bench::<$Impl>(b, 64) }
        #[bench] fn bytes_000000128(b: B) { hasher_bench::<$Impl>(b, 128) }
        #[bench] fn bytes_000000256(b: B) { hasher_bench::<$Impl>(b, 256) }
        #[bench] fn bytes_000000512(b: B) { hasher_bench::<$Impl>(b, 512) }
        #[bench] fn bytes_000001024(b: B) { hasher_bench::<$Impl>(b, 1024) }
        #[bench] fn bytes_000002048(b: B) { hasher_bench::<$Impl>(b, 2048) }
        #[bench] fn bytes_000004096(b: B) { hasher_bench::<$Impl>(b, 4096) }
        #[bench] fn bytes_000008000(b: B) { hasher_bench::<$Impl>(b, 8000) }
        #[bench] fn bytes_000016000(b: B) { hasher_bench::<$Impl>(b, 16_000) }
        #[bench] fn bytes_000032000(b: B) { hasher_bench::<$Impl>(b, 32_000) }
        #[bench] fn bytes_000064000(b: B) { hasher_bench::<$Impl>(b, 64_000) }
        #[bench] fn bytes_001000000(b: B) { hasher_bench::<$Impl>(b, 1_000_000) }
    }
}

#[cfg(test)] mod sip { hash_benches!{Sip} }
#[cfg(test)] mod xx { hash_benches!{Xx} }
#[cfg(test)] mod farm { hash_benches!{Farm} }
// #[cfg(test)] mod blake2b { hash_benches!{Blake2b} }
// #[cfg(test)] mod blake2s { hash_benches!{Blake2s} }
#[cfg(test)] mod fnv { hash_benches!{Fnv} }
// #[cfg(test)] mod murmur { hash_benches!{MurMur}}

