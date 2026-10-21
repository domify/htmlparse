//! The html5ever side of bench/run.py: parses the file named first n times
//! (the second argument) into an RcDom, as a document or, when the third
//! is `fragment`, as a fragment in `body`, and serializes each tree unless
//! the fourth is `0`, as bench/parse.ri does with htmlparse. Prints a
//! checksum of every output and the nanoseconds the parses and the
//! serializations took.

use std::time::Instant;

use html5ever::serialize::{serialize, SerializeOpts};
use html5ever::tendril::TendrilSink;
use html5ever::{local_name, ns, parse_document, parse_fragment, ParseOpts, QualName};
use markup5ever_rcdom::{Handle, RcDom, SerializableHandle};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let input = std::fs::read_to_string(&args[1]).unwrap();
    let n: usize = args[2].parse().unwrap();
    let fragment = args[3] == "fragment";
    let write = args[4] != "0";
    let mut sum: u64 = 0;
    let mut parsing: u128 = 0;
    let mut writing: u128 = 0;
    for _ in 0..n {
        let started = Instant::now();
        let dom = if fragment {
            let body = QualName::new(None, ns!(html), local_name!("body"));
            parse_fragment(RcDom::default(), ParseOpts::default(), body, vec![], false)
                .one(input.as_str())
        } else {
            parse_document(RcDom::default(), ParseOpts::default()).one(input.as_str())
        };
        let parsed = Instant::now();
        parsing += (parsed - started).as_nanos();
        if write {
            // A fragment's nodes are the children of the `html` element
            // under the document, as `Document#root` is in htmlparse.
            let root: Handle = if fragment {
                dom.document.children.borrow()[0].clone()
            } else {
                dom.document.clone()
            };
            let mut out = Vec::with_capacity(input.len());
            let handle: SerializableHandle = root.into();
            serialize(&mut out, &handle, SerializeOpts::default()).unwrap();
            writing += parsed.elapsed().as_nanos();
            let mut hash: u64 = 14695981039346656037;
            for byte in &out {
                hash = (hash ^ *byte as u64).wrapping_mul(1099511628211);
            }
            sum = sum.wrapping_add(hash);
        }
    }
    println!("{}", sum);
    println!("{}", parsing);
    println!("{}", writing);
}
