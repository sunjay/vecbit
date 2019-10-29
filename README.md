# `vecbit` – Managing Memory Bit by Bit

[![Crate][crate_img]][crate]
[![Documentation][docs_img]][docs]
[![License][license_img]][license_file]
[![Continuous Integration][travis_img]][travis]
[![Code Coverage][codecov_img]][codecov]
[![Crate Downloads][downloads_img]][crate]
[![Crate Size][loc_img]][loc]

## Summary

`vecbit` enables refining memory manipulation from single-byte precision to
single-bit precision. The bit-precision pointers in this crate allow creation of
more powerful bit-masks, set arithmetic, and I/O packet processing.

The core export of this crate is the type `SliceBit`. This type is a region of
memory with individually-addressable bits. It is accessed by standard Rust
references: `&SliceBit` and `&mut SliceBit`. These references are able to
describe and operate on regions that start and end on any arbitrary bit address,
regardless of alignment to a byte or processor word.

Rust provides three types to manipulate a sequence of memory: `&[T]`/`&mut [T]`
to borrow a region, `Box<[T]>` to statically own a region, and `Vec<T>` to
dynamically own a region. `vecbit` provides parallel types for each: `&SliceBit`
and `&mut SliceBit` borrow, `BitBox` statically owns, and `VecBit` dynamically
owns. These types mirror the relationships and APIs, including inherent methods
and trait implementations, that are found in the standard library.

## What Makes `vecbit` Different Than All The Other Bit Vector Crates

The most significant differences are that `vecbit` provides arbitrary bit
ordering through the `Cursor` trait, and provides a full-featured slice type.
Other crates have fixed orderings, and are often unable to produce slices that
begin at any arbitrary bit.

Additionally, the `vecbit` types implement the full extent of the standard
library APIs possible, in both inherent methods and trait implementations.

The `vecbit` types’ handles are exactly the size of their standard library
counterparts, while the other crates carry bit index information in separate
fields, making their handles wider. Depending on your needs, this may sway your
opinion in either direction. `vecbit` is more compact, but mangles the internal
pointer representation and requires more complex logic to use the bit region,
while other crates’ types are larger, but have more straightforward internal
logic.

## Why Would You Use This

- You need to directly control a bitstream’s representation in memory.
- You need to do unpleasant things with I/O communications protocols.
- You need a list of `bool`s that doesn’t waste 7 bits for every bit used.
- You need to do set arithmetic, or numeric arithmetic, on those lists.
- You are running a find/replace command on your repository from `&[bool]` to
  `&SliceBit`, or `Vec<bool>` to `VecBit`, and expect minimal damage as a
  result.

## Why *Wouldn’t* You Use This

- Your concern with the memory representation of bitsets includes sequence
  compression. `SliceBit` performs absolutely no compression, and maps bits
  directly into memory. Compressed bit sets can be found in other crates, such
  as the [`compacts`] crate, which uses the [Roaring BitSet] format.
- You want discontiguous data structures, such as a hash table, a tree, or any
  other hallmark of computer science beyond the flat array. `vecbit` does not,
  and will not, accomplish this. You may be able to use `vecbit`’s flat
  structures beneath a type wrapper which handles index processing, but `vecbit`
  types are incapable of accomplishing this task themselves. Also, I don’t know
  how any of those data structures work.

## Usage

**Minimum Rust Version**: `1.36.0`

The `1.36` release of Rust stabilized the `alloc` crate, allowing allocating
features (such as the `VecBit` type) to be used in `#![no_std]` environments
with the stable compiler series.

### Symbol Import

```toml
# Cargo.toml
[dependencies]
vecbit = "0.15"
```

`vecbit` is highly modular, and requires several items to function correctly.
The simplest way to use it is via prelude glob import:

```rust
use vecbit::prelude::*;
```

This imports the following symbols:

- `BigEndian`
- `BitBox` (only when an allocator is present)
- `SliceBit`
- `BitStore`
- `VecBit` (only when an allocator is present)
- `Bits`
- `BitsMut`
- `Cursor`
- `LittleEndian`
- `bitbox!` (only when an allocator is present)
- `vecbit!` (only when an allocator is present)

If you do not want these names imported directly into the local scope –
`Cursor`, `BigEndian`, and `LittleEndian` are likely culprits for name collision
– then you can import the prelude with a scope guard:

```rust
use vecbit::prelude as bv;
```

and those symbols will all be available only with a `bv::` prefix.

### Cargo Features

`vecbit` uses Cargo features to conditionally control some behavior.

The most prominent such behavior is one that cannot be controlled by Cargo
configuration: `u64` is only usable with this library when targeting a 64-bit
system. 32-bit system targets are only permitted to use `u8`, `u16`, and `u32`.

#### Atomic Behavior

`vecbit` uses atomic read/modify/write instructions by default. This is
necessary to avoid data races in `&mut SliceBit` operations without using
heavier synchronization mechanisms. If your target does not support Rust’s
`AtomicU*` types, or you do not want to use atomic RMW instructions, you may
disable the `atomic` feature:

```toml
# Cargo.toml

[dependencies.vecbit]
default-features = false
features = [
  # "atomic",
  "std",
]
```

#### Allocator Support

The two owning structures, `BitBox` and `VecBit`, require the presence of an
allocator. As `vecbit` is written specifically for use cases where an allocator
may not exist, this dependence can be disabled. `vecbit` is
`#![no_std]`-compatible once the `std` feature is disabled. It is not a design
goal to be `#![no_core]`-compatible.

```toml
# Cargo.toml

[dependencies.vecbit]
default-features = false
features = [
  "atomic",
  # "std",
]
```

If you are working in a `#![no_std]` environment that does have an allocator
available, you can reënable allocator support with the `alloc` feature:

```toml
# Cargo.toml

[dependencies.vecbit]
default-features = false # disables "std"
features = ["alloc"] # enables the allocator
```

This uses `#![feature(alloc)]`, which requires the nightly compiler.

#### Serde Support

De/serialization of bit slices is implemented through the `serde` crate. This
functionality is governed by both the `serde` feature and the `std` feature.

By default, when `serde` is enabled, `SliceBit`, `BitBox`, and `VecBit` all gain
the `Serialize` trait, and `BitBox` and `VecBit` gain the `Deserialize` trait.

When `std` is disabled, the `BitBox` and `VecBit` types are removed, leaving
only `SliceBit` with `Serialize`.

```toml
# Cargo.toml

[dependencies.vecbit]
features = ["serde"]
```

### Data Structures

`vecbit`’s three data structures are `&SliceBit`, `BitBox`, and `VecBit`. Each
of these types takes two type parameters, which I have elided previously.

The first type parameter is the `Cursor` trait. This trait governs how a bit
index maps to a bit position in the underlying memory. This parameter defaults
to the `BigEndian` type, which counts from the most significant bit first to the
least significant bit last. The `LittleEndian` type counts in the opposite
direction.

The second type parameter is the `BitStore` trait. This trait abstracts over the
Rust fundamental types `u8`, `u16`, and `u32`. On 64-bit targets, `u64` is also
available. This parameter defaults to `u8`, which acts on individual bytes.

These traits are both explained in the next section.

`&SliceBit<C: Cursor, T: BitStore>` is an immutable region of memory,
addressable at bit precision. This has all the inherent methods of Rust’s slice
primitive, `&[bool]`, and all the trait implementations. It has additional
methods which are specialized to its status as a slice of individual bits.

`&mut SliceBit<C: Cursor, T: BitStore>` is a mutable region of memory. This
functions identically to `&mut [bool]`, with the exception that `IndexMut` is
impossible: you cannot write `bitslice[index] = bit;`. This restriction is
sidestepped with the C++-style method `at`: `*bitslice.at(index) = bit;` is the
shim for write indexing.

The slice references have no restrictions on the alignment of their start or
end bits.

The owning references, described below, will always begin their slice aligned to
the edge of their `T: BitStore` type parameter. While this is not strictly
required by the implementation, it is convenient for ensuring that the
allocation pointer is preserved.

`BitBox<C: Cursor, T: BitStore>` is a `&mut SliceBit<C: Cursor, T: BitStore>` in
owned memory. It has few useful methods and no trait implementations of its own.
It is only capable of taking a bit slice into owned memory.

`VecBit<C: Cursor, T: BitStore>` is a `BitBox` that can adjust its allocation
size. It follows the inherent and trait API of the standard library’s `Vec`
type.

The API for these types is deliberately uninteresting. They are written to be as
close to drop-in replacements for the standard library types as possible. The
end goal of `vecbit` is that you should be able to adopt it by running three
`sed` find/replace commands on your repository. This is not literally possible,
but the work required for replacement is intended to be minimal.

### Traits

`vecbit` generalizes its behavior through the use of traits. In order to
optimize performance, these traits are *not* object-safe, and may *not* be used
as `dyn Trait` patterns for type erasure. Refactoring `vecbit` to support type
erasure would require significantly rewriting core infrastructure, and this is
not a design goal. I am willing to consider it if demand is shown, but I am not
going to proactively pursue it.

#### `Cursor`

The `Cursor` trait is an open-ended trait, that you are free to implement
yourself. It has one required function: `fn at<T: BitStore>(BitIdx) -> BitPos`.

This function translates a semantic index to an electrical position. `vecbit`
provides two implementations for you: `BigEndian` and `LittleEndian`, described
above. The invariants this function must uphold are listed in its documentation.

#### `BitStore`

The `BitStore` trait is sealed, and may only be implemented by this library. It
is used to abstract over the Rust fundamentals `u8`, `u16`, `u32`, and (on
64-bit systems) `u64`.

Your choice in fundamental types governs how the `Cursor` type translates
indices, and how the memory underneath your slice is written. The document
`doc/Bit Patterns.md` enumerates the effect of the `Cursor` and `BitStore`
combinations on raw memory.

If you are using `vecbit` to perform set arithmetic, and you expect that your
sets will have more full elements in the interior than partially-used elements
on the front and back edge, it is advantageous to use the local CPU word. The
`SliceBit` operations which traverse the slice are required to perform
bit-by-bit crawls on partially-used elements, but are able to use whole-word
instructions on full elements. The latter is a marked acceleration. The local
CPU word size (accessible through the type alias `vecbit::store::Local`) is the
default storage type for all data structures in the library.

If you are using `vecbit` to perform I/O packet manipulation, you should use the
fundamental and ordering that match your protocol specification.

#### `Bits` and `BitsMut`

The `Bits` and `BitsMut` traits are entry interfaces to the `SliceBit` types.
These are equivalent to the `AsRef` and `AsMut` reference conversion traits in
the standard library, and should be used as such.

These traits are implemented on the Rust fundamentals that implement `BitStore`
(`uN`), on slices of those fundamentals (`[uN]`), and the first thirty-two
arrays of them (`[uN; 0]` to `[uN; 32]`). Each implementation of these traits
causes a linear expansion of compile time, and going beyond thirty-two both
surpasses the standard library’s manual implementation limits, and is a
denial-of-service attack on each rebuild.

These traits are left open so that if you need to implement them on wider
arrays, you are able to do so.

You can use these traits to attach `.bits::<C: Cursor>()` and
`.bits_mut::<C: Cursor>()` conversion methods to any implementor, and gain
access to a `SliceBit` over that type, or to bound a generic function similar to
how the standard library uses `AsRef<Path>`:

```rust
let mut base = [0u8; 8];
let bits = base.bits_mut::<LittleEndian>();
//  bits is now an `&mut SliceBit<LittleEndian, u8>`
println!("{}", bits.len()); // 64

fn operate_on_bits(mut data: impl BitsMut) {
  let bits = data.bits_mut::<BigEndian>();
  //  `bits` is now an `&mut SliceBit<BigEndian, _>`
}
```

### Macros

The `bitbox!` and `vecbit!` macros allow convenient production of their
eponymous types, equivalent to the `vec!` macro in the standard library.

These macros accept an optional cursor token, an optional type token, and either
a list of bits or a single bit and a repetition counter.

Because these are standard macros, not proc-macros, they do not yet produce
well-optimized expanded code.

These macros are more thoroughly explained, including a list of all available
use syntaxes, in their documentation.

## Example Usage

This snippet runs through a selection of library functionality to demonstrate
behavior. It is deliberately not representative of likely usage.

```rust
extern crate vecbit;

use vecbit::prelude::*;

use std::iter::repeat;

fn main() {
    let mut bv = vecbit![BigEndian, u8; 0, 1, 0, 1];
    bv.reserve(8);
    bv.extend(repeat(false).take(4).chain(repeat(true).take(4)));

    //  Memory access
    assert_eq!(bv.as_slice(), &[0b0101_0000, 0b1111_0000]);
    //                   index 0 -^               ^- index 11
    assert_eq!(bv.len(), 12);
    assert!(bv.capacity() >= 16);

    //  Stack operations
    bv.push(true);
    bv.push(false);
    bv.push(true);

    assert!(bv[12]);
    assert!(!bv[13]);
    assert!(bv[14]);
    assert!(bv.get(15).is_none());

    bv.pop();
    bv.pop();
    bv.pop();

    //  Set operations. These deliberately have no effect.
    bv &= repeat(true);
    bv = bv | repeat(false);
    bv ^= repeat(true);
    bv = !bv;

    //  Arithmetic operations
    let one = vecbit![BigEndian, u8; 1];
    bv += one.clone();
    assert_eq!(bv.as_slice(), &[0b0101_0001, 0b0000_0000]);
    bv -= one.clone();
    assert_eq!(bv.as_slice(), &[0b0101_0000, 0b1111_0000]);

    //  Borrowing iteration
    let mut iter = bv.iter();
    //  index 0
    assert_eq!(iter.next().unwrap(), false);
    //  index 11
    assert_eq!(iter.next_back().unwrap(), true);
    assert_eq!(iter.len(), 10);
}
```

Immutable and mutable access to the underlying memory is provided by the `AsRef`
and `AsMut` implementations, so the `VecBit` can be readily passed to transport
functions.

`VecBit` implements `Borrow` down to `SliceBit`, and `SliceBit` implements
`ToOwned` up to `VecBit`, so they can be used in a `Cow` or wherever this API
is desired. Any case where a `Vec`/`[T]` pair cannot be replaced with a
`VecBit`/`SliceBit` pair is a bug in this library, and a bug report is
appropriate.

`VecBit` can relinquish its owned memory with `.into_vec()` or
`.into_boxed_slice()`, and `SliceBit` can relinquish its borrow by going out
of scope.

## Warnings

The `SliceBit` type is able to cause memory aliasing, as multiple independent
`&mut SliceBit` instances may use the same underlying memory. This crate takes
care to ensure that all observed behavior is exactly as expected, without any
side effects.

The `SliceBit` methods only use whole-element instructions when the slice spans
the full width of the element; when the slice has only partial use, the methods
crawl each bit individually. This is slower on most architectures, but
guarantees safety.

Race conditions are avoided through use of the atomic read/modify/write
instructions stabilized in `1.34.0`, as described above.

## Planned Features

Contributions of items in this list are *absolutely* welcome! Contributions of
other features are also welcome, but I’ll have to be sold on them.

- Creation of specialized pointers `Rc<SliceBit>` and `Arc<SliceBit>`.
- Procedural macros for `vecbit!` and `bitbox!`
- An FFI module, and bindings from other languages.

[codecov]: https://codecov.io/gh/sunjay/vecbit "Code Coverage"
[codecov_img]: https://img.shields.io/codecov/c/github/sunjay/vecbit.svg?logo=codecov "Code Coverage Display"
[crate]: https://crates.io/crates/vecbit "Crate Link"
[crate_img]: https://img.shields.io/crates/v/vecbit.svg?logo=rust "Crate Page"
[docs]: https://docs.rs/vecbit "Documentation"
[docs_img]: https://docs.rs/vecbit/badge.svg "Documentation Display"
[downloads_img]: https://img.shields.io/crates/dv/vecbit.svg?logo=rust "Crate Downloads"
[license_file]: https://github.com/sunjay/vecbit/blob/master/LICENSE.txt "License File"
[license_img]: https://img.shields.io/crates/l/vecbit.svg "License Display"
[loc]: https://github.com/sunjay/vecbit "Repository"
[loc_img]: https://tokei.rs/b1/github/sunjay/vecbit "Repository Size"
[travis]: https://travis-ci.org/sunjay/vecbit "Travis CI"
[travis_img]: https://img.shields.io/travis/sunjay/vecbit.svg?logo=travis "Travis CI Display"
[`compacts`]: https://crates.io/crates/compacts
[Roaring BitSet]: https://arxiv.org/pdf/1603.06549.pdf
