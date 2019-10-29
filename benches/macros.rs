/*! Macro construction benchmarks.

This is taken from [issue #28], which noted that the `vecbit![bit; rep]`
expansion was horribly inefficient.

This benchmark crate should be used for all macro performance recording, and
compare the macros against `vec!`. While `vec!` will always be faster, because
`vecbit!` does more work than `vec!`, they should at least be close.

Original performance was 10,000x slower. Performance after the fix for #28 was
within 20ns.

[issue #28]: https://github.com/sunjay/vecbit/issues/28
!*/

#![feature(test)]

extern crate test;

use test::Bencher;
use vecbit::prelude::*;

#[bench]
fn vecbit_init(b: &mut Bencher) {
	b.iter(|| vecbit![0; 16 * 16 * 9]);
	b.iter(|| vecbit![1; 16 * 16 * 9]);
}

#[bench]
fn vec_init(b: &mut Bencher) {
	b.iter(|| vec![0u8; 16 * 16 * 9 / 8]);
	b.iter(|| vec![-1i8; 16 * 16 * 9 / 8]);
}
