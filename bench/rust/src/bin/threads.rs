//! The html5ever side of bench/run.py's threaded runs: `threads` threads
//! (the third argument), each parsing the file named first n times (the
//! second) and serializing every tree, as bench/threads.ri does. Prints a
//! checksum of every output and the nanoseconds from the first start to
//! the last join.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use html5ever::serialize::{serialize, SerializeOpts};
use html5ever::tendril::TendrilSink;
use html5ever::{local_name, ns, parse_document, parse_fragment, ParseOpts, QualName};
use markup5ever_rcdom::{Handle, RcDom, SerializableHandle};

fn work(input: &str, n: usize, fragment: bool) -> u64 {
    let mut sum: u64 = 0;
    for _ in 0..n {
        let dom = if fragment {
            let body = QualName::new(None, ns!(html), local_name!("body"));
            parse_fragment(RcDom::default(), ParseOpts::default(), body, vec![], false).one(input)
        } else {
            parse_document(RcDom::default(), ParseOpts::default()).one(input)
        };
        let root: Handle = if fragment {
            dom.document.children.borrow()[0].clone()
        } else {
            dom.document.clone()
        };
        let mut out = Vec::with_capacity(input.len());
        let handle: SerializableHandle = root.into();
        serialize(&mut out, &handle, SerializeOpts::default()).unwrap();
        let mut hash: u64 = 14695981039346656037;
        for byte in &out {
            hash = (hash ^ *byte as u64).wrapping_mul(1099511628211);
        }
        sum = sum.wrapping_add(hash);
    }
    sum
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let input = std::fs::read_to_string(&args[1]).unwrap();
    let n: usize = args[2].parse().unwrap();
    let count: usize = args[3].parse().unwrap();
    let fragment = args.get(4).map_or(false, |a| a == "fragment");
    let total = AtomicU64::new(0);
    let started = Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..count {
            scope.spawn(|| {
                let sum = work(&input, n, fragment);
                total.fetch_add(sum, Ordering::Relaxed);
            });
        }
    });
    let took = started.elapsed();
    println!("{}", total.load(Ordering::Relaxed));
    println!("{}", took.as_nanos());
}
