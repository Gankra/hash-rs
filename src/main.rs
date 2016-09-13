#![feature(asm)]
#![feature(test)]
#![allow(unused_imports, dead_code)]

extern crate twox_hash;
// extern crate murmurhash64;
extern crate murmurhash3;
extern crate farmhash;
extern crate fnv as _fnv;
extern crate blake2_rfc;
extern crate test;
extern crate regex;
extern crate rand;
extern crate btree_rewrite;

mod multiply_shift;

use std::process::{Stdio, Command};
use std::io::Result as IoResult;
use std::io::stdout;
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
    let child = Command::new("cargo")
                         .arg("bench")
                         .stdout(Stdio::piped())
                         .spawn()
                         .unwrap_or_else(|e| { panic!("failed to execute process: {}", e) });
    let mut out = child.stdout.unwrap();
    let mut read_buf = [0u8; 64];
    let mut out_buf: Vec<u8> = Vec::new();
    while let Ok(size) = out.read(&mut read_buf) {
        if size == 0 {
            break;
        }
        try!(stdout().write_all(&read_buf[..size]));
        out_buf.extend(&read_buf[..size]);
    }

    let re = Regex::new(r#"test (.*)::(.*)_(\d*) .*bench:\s*(.*) ns/iter \(\+/- (.*)\) = (\d*) MB/s.*"#).unwrap();

    println!("Output:");

    let mut data = HashMap::new();

    for cap in re.captures_iter(&String::from_utf8(out_buf).unwrap()) {
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
        use std::hash::Hasher;
        use std::hash::{BuildHasherDefault, BuildHasher};
        use multiply_shift::HornerHasher;

        use std::collections::HashMap;
        use test::{black_box, Bencher};
        pub type B<'a> = &'a mut Bencher;
        use rand::{Rng, thread_rng};

        fn hasher_bench<H>(b: B, len: usize)
        where H: Hasher + Default
        {
            let hash_state = BuildHasherDefault::<H>::default();
            let bytes: Vec<u8> = (0..100).cycle().take(len).collect();
            let bytes = black_box(bytes);

            b.bytes = bytes.len() as u64;
            b.iter(|| {
                let mut hasher = hash_state.build_hasher();
                hasher.write(&bytes);
                hasher.finish()
            });
        }

        fn map_bench_dense<H>(b: B, len: usize)
        where H: Hasher + Default
        {
            let num_strings = 1000;
            let prime1 = 93;
            let data: Vec<u8> = (0..prime1).cycle().take(len * num_strings).collect();
            let data = black_box(data);

            b.bytes = (len * num_strings) as u64;
            b.iter(|| {
                // don't reserve space to be fair to BTree
                let mut map = HashMap::with_hasher(BuildHasherDefault::<H>::default());
                for chunk in data.chunks(len) {
                    *map.entry(chunk).or_insert(0) += 1;
                }
                map
            });
        }

        fn map_bench_sparse<H>(b: B, len: usize)
        where H: Hasher + Default
        {
            let num_strings = 1000;
            let data: Vec<u8> = thread_rng().gen_iter()
                                            .take(len * num_strings)
                                            .collect();


            b.bytes = (len * num_strings) as u64;
            b.iter(|| {
                // don't reserve space to be fair to BTree
                let mut map = HashMap::with_hasher(BuildHasherDefault::<H>::default());
                for chunk in data.chunks(len) {
                    *map.entry(chunk).or_insert(0) += 1;
                }
                map
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

        #[bench] fn mapcountsparse_000000001(b: B) { map_bench_sparse::<$Impl>(b, 1) }
        #[bench] fn mapcountsparse_000000002(b: B) { map_bench_sparse::<$Impl>(b, 2) }
        #[bench] fn mapcountsparse_000000004(b: B) { map_bench_sparse::<$Impl>(b, 4) }
        #[bench] fn mapcountsparse_000000008(b: B) { map_bench_sparse::<$Impl>(b, 8) }
        #[bench] fn mapcountsparse_000000016(b: B) { map_bench_sparse::<$Impl>(b, 16) }
        #[bench] fn mapcountsparse_000000032(b: B) { map_bench_sparse::<$Impl>(b, 32) }
        #[bench] fn mapcountsparse_000000064(b: B) { map_bench_sparse::<$Impl>(b, 64) }
        #[bench] fn mapcountsparse_000000128(b: B) { map_bench_sparse::<$Impl>(b, 128) }
        #[bench] fn mapcountsparse_000000256(b: B) { map_bench_sparse::<$Impl>(b, 256) }
        #[bench] fn mapcountsparse_000000512(b: B) { map_bench_sparse::<$Impl>(b, 512) }
        #[bench] fn mapcountsparse_000001024(b: B) { map_bench_sparse::<$Impl>(b, 1024) }
        #[bench] fn mapcountsparse_000002048(b: B) { map_bench_sparse::<$Impl>(b, 2048) }

        #[bench] fn mapcountdense_000000001(b: B) { map_bench_dense::<$Impl>(b, 1) }
        #[bench] fn mapcountdense_000000002(b: B) { map_bench_dense::<$Impl>(b, 2) }
        #[bench] fn mapcountdense_000000004(b: B) { map_bench_dense::<$Impl>(b, 4) }
        #[bench] fn mapcountdense_000000008(b: B) { map_bench_dense::<$Impl>(b, 8) }
        #[bench] fn mapcountdense_000000016(b: B) { map_bench_dense::<$Impl>(b, 16) }
        #[bench] fn mapcountdense_000000032(b: B) { map_bench_dense::<$Impl>(b, 32) }
        #[bench] fn mapcountdense_000000064(b: B) { map_bench_dense::<$Impl>(b, 64) }
        #[bench] fn mapcountdense_000000128(b: B) { map_bench_dense::<$Impl>(b, 128) }
        #[bench] fn mapcountdense_000000256(b: B) { map_bench_dense::<$Impl>(b, 256) }
        #[bench] fn mapcountdense_000000512(b: B) { map_bench_dense::<$Impl>(b, 512) }
        #[bench] fn mapcountdense_000001024(b: B) { map_bench_dense::<$Impl>(b, 1024) }
        #[bench] fn mapcountdense_000002048(b: B) { map_bench_dense::<$Impl>(b, 2048) }
   }
}

macro_rules! tree_benches {
    ($Impl: ty) => {
        use std::collections::BTreeMap as StdBTree;
        use btree_rewrite::map::BTreeMap as NewBTree;

        use test::{black_box, Bencher};
        pub type B<'a> = &'a mut Bencher;
        use rand::{Rng, thread_rng};

        fn map_bench_dense(b: B, len: usize) {
            let num_strings = 1000;
            let prime1 = 93;
            let data: Vec<u8> = (0..prime1).cycle().take(len * num_strings).collect();
            let data = black_box(data);

            b.bytes = (len * num_strings) as u64;
            b.iter(|| {
                let mut map: $Impl = Default::default();
                for chunk in data.chunks(len) {
                    *map.entry(chunk).or_insert(0) += 1;
                }
                map
            });
        }

        fn map_bench_sparse(b: B, len: usize) {
            let num_strings = 1000;
            let data: Vec<u8> = thread_rng().gen_iter()
                                            .take(len * num_strings)
                                            .collect();
            let data = black_box(data);

            b.bytes = (len * num_strings) as u64;
            b.iter(|| {
                let mut map: $Impl = Default::default();
                for chunk in data.chunks(len) {
                    *map.entry(chunk).or_insert(0) += 1;
                }
                map
            });
        }


        #[bench] fn mapcountsparse_000000001(b: B) { map_bench_sparse(b, 1) }
        #[bench] fn mapcountsparse_000000002(b: B) { map_bench_sparse(b, 2) }
        #[bench] fn mapcountsparse_000000004(b: B) { map_bench_sparse(b, 4) }
        #[bench] fn mapcountsparse_000000008(b: B) { map_bench_sparse(b, 8) }
        #[bench] fn mapcountsparse_000000016(b: B) { map_bench_sparse(b, 16) }
        #[bench] fn mapcountsparse_000000032(b: B) { map_bench_sparse(b, 32) }
        #[bench] fn mapcountsparse_000000064(b: B) { map_bench_sparse(b, 64) }
        #[bench] fn mapcountsparse_000000128(b: B) { map_bench_sparse(b, 128) }
        #[bench] fn mapcountsparse_000000256(b: B) { map_bench_sparse(b, 256) }
        #[bench] fn mapcountsparse_000000512(b: B) { map_bench_sparse(b, 512) }
        #[bench] fn mapcountsparse_000001024(b: B) { map_bench_sparse(b, 1024) }
        #[bench] fn mapcountsparse_000002048(b: B) { map_bench_sparse(b, 2048) }

        #[bench] fn mapcountdense_000000001(b: B) { map_bench_dense(b, 1) }
        #[bench] fn mapcountdense_000000002(b: B) { map_bench_dense(b, 2) }
        #[bench] fn mapcountdense_000000004(b: B) { map_bench_dense(b, 4) }
        #[bench] fn mapcountdense_000000008(b: B) { map_bench_dense(b, 8) }
        #[bench] fn mapcountdense_000000016(b: B) { map_bench_dense(b, 16) }
        #[bench] fn mapcountdense_000000032(b: B) { map_bench_dense(b, 32) }
        #[bench] fn mapcountdense_000000064(b: B) { map_bench_dense(b, 64) }
        #[bench] fn mapcountdense_000000128(b: B) { map_bench_dense(b, 128) }
        #[bench] fn mapcountdense_000000256(b: B) { map_bench_dense(b, 256) }
        #[bench] fn mapcountdense_000000512(b: B) { map_bench_dense(b, 512) }
        #[bench] fn mapcountdense_000001024(b: B) { map_bench_dense(b, 1024) }
        #[bench] fn mapcountdense_000002048(b: B) { map_bench_dense(b, 2048) }
    }
}

#[cfg(test)] mod sip { hash_benches!{Sip} }
#[cfg(test)] mod xx { hash_benches!{Xx} }
#[cfg(test)] mod farm { hash_benches!{Farm} }
#[cfg(test)] mod fnv { hash_benches!{Fnv} }
#[cfg(test)] mod horner { hash_benches!{HornerHasher} }

// one day?

// #[cfg(test)] mod blake2b { hash_benches!{Blake2b} }
// #[cfg(test)] mod blake2s { hash_benches!{Blake2s} }
// #[cfg(test)] mod murmur { hash_benches!{MurMur}}


#[cfg(test)] mod btree { tree_benches!{StdBTree<&[u8], i32>} }
#[cfg(test)] mod btreenew { tree_benches!{NewBTree<&[u8], i32>} }
