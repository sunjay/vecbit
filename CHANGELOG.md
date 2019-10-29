# Changelog

All notable changes will be documented in this file.

This document is written according to the [Keep a Changelog][kac] style.

## 0.16.0

### Added

- `Cursor` now provides a `mask` function, which produces a one-hot mask usable
  for direct memory access. Implementors of `Cursor` may use the default, or
  provide their own.
- `Bits` and `BitsMut` renamed their methods to `bits` and `bits_mut`,
  respectively; `as_bitslice` and `as_mut_bitslice` are marked deprecated and
  will be removed in `0.17`.
- The `BitField` trait allows `SliceBit<BigEndian, _>` and
  `SliceBit<LittleEndian, _>` to provide behavior analagous to bitfields in C
  and C++ `struct` definitions. This trait provides `load` and `store` methods
  on `SliceBit`s with those two `Cursor`s which allow for parallel access to the
  underlying memory. This trait is currently not able to be implemented by
  downstream crates; this restriction may ease in the future.
- The `cursor::Local` type alias is a default bit ordering. Big-endian targets
  set it to `cursor::BigEndian`; all other targets set it to
  `cursor::LittleEndian`.
- The `store::Word` type alias is a default unit size. Targets with 32-bit CPU
  words set it to `u32`; 64-bit CPU word targets set it to `u64`; all other
  targets set it to `u8`.

### Changed

- The default order and storage type parameters for all type constructors in the
  library have been changed. This means that `SliceBit`, `BitBox`, `VecBit`, and
  the `bitbox!` and `vecbit!` macros, are all changing the produced type if you
  have not specified their ordering and storage. The new default storage type is
  the target CPU word (`u32` on 32-bit systems, `u64` on 64-bit, `u8` on other)
  and the new default order type is the target byte ordering (`BigEndian` on
  big-endian, `LittleEndian` on little-endian and unknown).

  This change is expected to break dependent crates. The fix is straightforward:
  specify the types produced by this crate’s constructors, or adapt the types
  that receive them.

  This change was made in order to provide performance advantages by using the
  native CPU word size, and to ease choice of a bit ordering in usages that do
  not particularly care about the underlying memory’s appearance.
- The internal process that translates `SliceBit` operations into access
  operations on underlying memory has been rewritten. Production of contended
  references to bare fundamentals is now forbidden, and all access is mediated
  through either atomic (default) or `Cell` types.
- Bit indexing is more firmly encoded in the type system, which reduces the load
  of runtime assertions.
- `SliceBit::as_slice` excludes partial edge elements. `BitBox` and `VecBit` do
  not.

### Removed

- `SliceBit::change_cursor` and `change_cursor_mut` allow incorrectly aliasing
  memory with different slice handles, because there is no way for them to
  compute the electrical positions they govern and then construct new indices in
  the target `Cursor` that match those positions.

  Example:

  ```rust
  use vecbit::prelude::*;
  let mut elt = 0u8;
  let bits = elt.bits_mut::<BigEndian>();
  let (head, tail) = bits.split_at_mut(4);
  let tail = tail.change_cursor_mut::<LittleEndian>();
  ```

  `head` now points at the first four bits in big-endian order, and tail at the
  last four bits in little-endian order, and these indices all map to the high
  nibble of `elt`. `head` and `tail` mutably alias.

  These functions are retained in `BitBox` and `VecBit`, as those types do not
  allow memory contention.

## 0.15.2

### Changed

The `vecbit![bit; rep]` construction macro has its implementation rewritten to
be much faster. This fault was reported by GitHub user [@caelunshun] in
[Issue #28].

## 0.15.1

### Removed

The `Send` implementation on `SliceBit` is removed when the `atomic` feature is
disabled.

While the `x86` architecture provides hardware guarantees that a
read/modify/write instruction sequence will update all other views of the
referent data, this is a property of the specific underlying machine and *not*
a property of the Rust abstract machine as interpreted by the compiler and LLVM.
As such, while `&mut SliceBit` references that alias the same underlying memory
element will not collide with each other in practice, they still must use atomic
operations in order to satisfy the Rust abstract machine.

The atomic feature is provided by default, and users must explicitly disable it
in order to disable atomic instruction access and thus remove the `Send` impl
allowing `&mut SliceBit` to cross threads.

Because this change does not affect the default interface exported by the crate,
I have decided to make this a patch release rather than bump the minor version.

## 0.15.0

### Changed

The minimum compiler version was increased to `1.36.0`, which stabilized the
`alloc` crate. As such, the `#![feature(alloc)]` flag has been removed from the
library, and usage as `--no-default-features --features alloc` may safely use
allocation on the stable compiler series.

As `alloc` is available on a stable compiler, the `alloc` *crate feature* has
been made a strict dependency of the `std` crate feature.

Use of `--no-default-features` continues to set the crate in `#![no_std]` mode,
with no allocation support. `--no-default-features --features alloc` adds a
dependency on `alloc`, and the allocating types. The `std` feature alone now
*only* adds operating-system interfaces, such as `io::Write`.

`std` depends on `alloc`, so using the `std` feature will pull in allocation.

## 0.14.0

### Added

- `add_assign_reverse` on `SliceBit` and `VecBit`, and `add_reverse` on
  `BitBox` and `VecBit`.

  These methods perform left-to-right addition, propagating the carry from the
  0th bit in the sequence to the nth. On `SliceBit`, `add_assign_reverse`
  returns the carry-out bit. On `VecBit`, `add_assign_reverse` and `add_reverse`
  push the carry-out to the right end of the vector.

  This feature was requested in [Issue #16], by GitHub user [@GeorgeGkas].

## 0.13.0

### Changed

- The `BitPtr<T>` internal representation replaced the elements/tail tuple with
  a bit-length counter. Most of the changes as a result of this were purely
  internal, but as it affected the `Serde` representation, this was moved to a
  new version.

## 0.12.0

### Added

- `SliceBit::at` simulates a write reference to a single bit. It creates an
  instance of `slice::BitGuard`, which holds a mutable reference to the
  requested bit and a `bool` slot. `BitGuard` implements `Deref` and `DerefMut`
  to its local `bool`, and writes its local `bool` value to the specified bit in
  `Drop`.

  This allows writing the following:

  ```rust
  *slice.at(index) = some_bit();
  ```

  as equivalent to

  ```rust
  slice.set(index, some_bit());
  ```

  Note that binding the value produced by `SliceBit::at` will cause the write to
  occur when that binding *goes out of scope*, not in the assigning statement.

  ```rust
  let slot = slice.at(index);
  *index = some_bit();
  //  write has not yet occurred in `slot`
  //  ... more work
  //  <- write occurs HERE
  ```

  In practice, this should not be an issue, since the rules for mutable borrows
  mean that the original slice is not observable until the slot value produced
  by `.at()` goes out of scope.

- **SEE THE RENAME BELOW.** The `Bits` and `BitsMut` traits provide reference
  conversion from many Rust fundamental types to `SliceBit` regions. `Bits` is
  analagous to `AsRef`, and `BitsMut` to `AsMut`. These traits are implemented
  on the `BitStore` fundamentals, slices of them, and arrays up to 32.

- `SliceBit::get_unchecked` and `SliceBit::set_unchecked` perform read and write
  actions without any bounds checking to ensure the index is within the slice
  bounds. This allows faster work in tight loops where the index is already
  checked against the slice length. These methods are, of course, incredibly
  unsafe, as they are raw memory access with no safeguards to ensure the read or
  write is within bounds.

### Changed

- `VecBit::retain` changed its function argument from `(bool) -> bool` to
  `(usize, bool) -> bool`, and passes the index as well as the value.
- `Display` implementations of the `BitIdx` and `BitPos` types now just defer to
  the interior number, and do not write their own type.
- `SliceBit::as_ptr` and `::as_mut_ptr` now return the null pointer if they are
  the empty slice, rather than a dangling pointer.
- The trait formerly known as `Bits` in all previous versions is now `BitStore`,
  and the module `bits` is renamed to `store`. Only the `Bits` → `BitStore`
  rename affects public API.
- Rewrote the README to better describe all the recent work.
- The documentation examples use the new `as_bitslice` methods instead of the
  much less pleasant `Into` conversions to create `SliceBit` handles. This also
  serves to demonstrate the new favored method to access regions as bit slices.

## 0.11.3

[Issue #15]: Incorrect validity check in `BitIdx::span`; excluded tail indices
which were used in `VecBit::push`, inducing false `panic!` events. Thanks to
GitHub user [@schomatis] for the report.

## 0.11.2

### Added

- `BitBox` and `VecBit` implement [`Sync`], as discussion with [@ratorx] and
  more careful reading of the documentation for `Sync` has persuaded me that
  this is sound.

## 0.11.1

[Issue #12]: I left in an `eprintln!` statement from debugging
`SliceBit::set_all`. Thanks to GitHub user [@koushiro] for the report.

## 0.11.0

This contains the last (planned) compiler version upgrade, to `1.34.0`, and the
last major feature add before `1.0`: Serde-powered de/serialization.

Deserialization is not possible without access to an allocator, so it is behind
a feature gate, `serde`, which depends on the `alloc` feature.

`SliceBit`, `BitBox`, and `VecBit` all support serialization, and `BitBox` and
`VecBit` support deserialization

### Added

- `serde` feature to serialize `SliceBit`, `BitBox`, and `VecBit`, and
  deserialize `BitBox` and `VecBit`.
- `change_cursor<D>` method on `SliceBit`, `BitBox`, and `VecBit`, which enable
  changing the element traversal order on a data set without modifying that
  data. This is useful for working with slices that have their cursor type
  erased, such as crossing serialization or foreign-language boundaries.
- The internal `atomic` module and `Atomic` trait permit atomic access to
  elements for the `Bits` trait to use when performing bit set operations. This
  is not exposed to the public API.
- Internal domain models for the memory regions governed by `BitPtr`. These
  models provide improved logical support for manipulating bit sequences with as
  little inefficiency as possible.
- `BitPtr::bare_parts` and `BitPtr::region_data` internal APIs for accessing
  components of the pointer structure.
- Clippy is now part of the development routine.
- `bitbox!` macro wraps `vecbit!` to freeze the produced vector.

### Changed

- The internal `Bits` trait uses a `const fn` stabilized in `1.33.0` in order to
  compute type information, rather than requiring explicit statements in the
  implementations. It now uses synchronized access to elements for write
  operations, to prevent race conditions between adjacent bit slices that
  overlap in an element.
- The internal `BitPtr` representation had its bit pattern rules modified. There
  is now only one empty-slice region representation, and the pointer is able to
  index one more element than it previously could. In addition, `BitPtr::tail()`
  produces `0` when empty, rather than `T::BITS`, allowing for more correct
  values in `serde` de/serialization.

### Removed

- `BitPtr::set_head` and `BitPtr::set_tail`: in practice, `::new` and
  `::new_unchecked` were used at all potential use sites for these functions, as
  they are more powerful and better validated.
- `BitPtr::head_elt`, `BitPtr::body_elts`, and `BitPtr::tail_elt` were
  superseded by the `domain` module. Their public use is better served by
  the `AsRef` trait.
- `BitPtr::is_full`: removed for being never used in the library, and not an
  interesting query.

### Issues Resolved

- [Issue #9] revealed a severe logic error in the construction of bit masks in
  `Bits::set_at`. Thanks to GitHub user [@torce] for the bug report!
- [Issue #10] revealed a logic error in the construction of bit vectors from bit
  slices which did not begin at the front of an element.

  `VecBit::from_bitslice` cloned the entire underlying `&[T]` of the source
  `SliceBit`, which is incorrect, as `VecBit` currently cannot support offset
  head cursors. The correct behavior is to use `<VecBit as FromIterator<bool>>`
  to collect the source slice into a fresh `VecBit`.

  It may be possible in the future to permit offset head cursors in `BitBox` and
  `VecBit`.

  Thanks to GitHub user [@overminder] for the bug report!
- `SliceBit::set_all` had a bug where fully spanned elements were zeroed, rather
  than filled with the requested bit. This was only detected when the
  subtraction example in the `README` code sample broke. Resolution: add a
  function to the `Bits` trait which fills an element with a bit, producing all
  zero or all one.

## 0.10.2

Bugfix for [Issue #8]. This provides explicit implementations of the threading
traits `Send` and `Sync`. These traits were formerly automatically implemented;
the implementation change in `0.10.0` appears to have removed the automatic
impls.

`SliceBit` is both `Send` and `Sync`, as it is unowned memory. `BitBox` and
`VecBit` are `Send` but not `Sync`, as they are owned memory.

Thanks to GitHub user [@ratorx] for the report!

## 0.10.1

Bugfix for [Issue #7]. `SliceBit::count_ones` and `SliceBit::count_zeros`
counted the total number of bits present in a slice, not the number of bits set
or unset, when operating inside a single element.

The small case used `.map().count()`, but the large case correctly used
`.map().filter().count()`. The missing `.filter()` call, to remove unset or set
bits from the counting, was the cause of the bug.

Thanks to GitHub user [@geq1t] for the report!

## 0.10.0

This version was a complete rewrite of the entire crate. The minimum compiler
version has been upgraded to `1.31.0`. The crate is written against the Rust
2018 edition of the language. It will be a `1.0` release after polishing.

### Added

- `BitPtr` custom pointer representation. This is the most important component
  of the rewrite, and what enabled the expanded feature set and API surface.
  This structure allows `SliceBit` and `VecBit` to have head cursors at any bit,
  not just at the front edge of an element. This allows the crate to support
  arbitrary range slicing and slice splitting, and in turn greatly expand the
  usability of the slice and vector types.

  The `BitPtr` type is wholly crate-internal, and renders the `&SliceBit` and
  `VecBit` handle types ***wholly incompatible*** with standard Rust slice and
  vector handles. With great power comes great responsibility to never, ever,
  interchange these types through any means except the provided translation API.

- Range indexing and more powerful iteration. Bit-precision addressing allows
  arbitrary subslices and enables more of the slice API from `core`.

### Changed

- Almost everything has been rewritten. The git diff for this version is
  horrifying.

- Formatting traits better leverage the builtin printing structures available
  from `core::fmt`, and are made available on `no_std`.

### Removed

- `u64` is only usable as the storage type on 64-bit systems; it has 32-bit
  alignment on 32-bit systems and as such is unusable there.

## 0.9.0

### Changed

- The trait `Endian` has been renamed to `Cursor`, and all type variables
  `E: Endian` have been renamed to `C: Cursor`.

- The `Bits` trait is no longer bound by `Default`.

## 0.8.0

### Added

- `std` and `alloc` features, which can be disabled for use in `#![no_std]`
  libraries. This was implemented by Robert Habermeier, `rphmeier@gmail.com`.

  Note that the `SliceBit` tests and all the examples are disabled when the
  `alloc` feature is not present. They will function normally when `alloc` is
  present but `std` is not.

### Changed

- Compute `Bits::WIDTH` as `size_of::<Self>() * 8` instead of `1 << Bits::INDX`.

## 0.7.0

### Added

- `examples/readme.rs` tracks the contents of the example code in `README.md`.
  It will continue to do so until the `external_doc` feature stabilizes so that
  the contents of the README can be included in the module documentation of
  `src/lib.rs`.

- Officially use the Rust community code of conduct.

- README sections describe why a user might want this library, and what makes it
  different than `bit-vec`.

### Changed

- Update minimum Rust version to `1.30.0`.

  Internally, this permits use of `std` rather than `::std`. This compiler
  edition does not change *intra-crate* macro usage. Clients at `1.30.0` and
  above no longer need `#[macro_use]` above `extern crate vecbit;`, and are able
  to import the `vecbit!` macro directly with `use vecbit::vecbit;` or
  `use vecbit::prelude::*`.

  Implementation note: References to literals stabilized at *some* point between
  `1.20.0` and `1.30.0`, so the static bool items used for indexing are no
  longer needed.

- Include numeric arithmetic as well as set arithmetic in the README.

## 0.6.0

### Changed

- Update minimum Rust version to `1.25.0` in order to use nested imports.
- Fix logic in `Endian::prev`, and re-enabled edge tests.
- Pluralize `SliceBit::count_one()` and `SliceBit::count_zero()` function names.
- Fix documentation and comments.
- Consolidate implementation of `vecbit!` to not use any other macros.

## 0.5.0

### Added

- `VecBit` and `SliceBit` implement `Hash`.

- `VecBit` fully implements addition, negation, and subtraction.

- `SliceBit` implements in-place addition and negation.
  - `impl AddAssign for SliceBit`
  - `impl Neg for &mut SliceBit`

  This distinction is required in order to match the expectations of the
  arithmetic traits and the realities of immovable `SliceBit`.

- `SliceBit` offers `.all()`, `.any()`, `.not_all()`, `.not_any()`, and
  `.some()` methods to perform n-ary Boolean logic.
  - `.all()` tests if all bits are set high
  - `.any()` tests if any bits are set high (includes `.all()`)
  - `.not_all()` tests if any bits are set low (includes `.not_all()`)
  - `.not_any()` tests if all bits are set low
  - `.some()` tests if any bits are high and any are low (excludes `.all()` and
    `.not_all()`)

- `SliceBit` can count how many bits are set high or low with `.count_one()` and
  `.count_zero()`.

## 0.4.0

### Added

`SliceBit::for_each` provides mutable iteration over a slice. It yields each
successive `(index: usize, bit: bool)` pair to a closure, and stores the return
value of that closure at the yielded index.

`VecBit` now implements `Eq` and `Ord` against other `VecBit`s. It is impossible
at this time to make `VecBit` generic over anything that is `Borrow<SliceBit>`,
which would allow comparisons over different ownership types. The declaration

```rust
impl<A, B, C, D, E> PartialEq<C> for VecBit<A, B>
where A: Endian,
    B: Bits,
    C: Borrow<SliceBit<D, E>>,
    D: Endian,
    E: Bits,
{
    fn eq(&self, rhs: E) { … }
}
```

is impossible to write, so `VecBit == SliceBit` will be rejected.

As with many other traits on `VecBit`, the implementations are just a thin
wrapper over the corresponding `SliceBit` implementations.

### Changed

Refine the API documentation. Rust guidelines recommend imperative rather than
descriptive summaries for function documentation, which largely meant stripping
the trailing -s from the first verb in each function document.

I also moved the example code from the trait-level documentation to the
function-level documentation, so that it would show up an `type::func` in the
`rustdoc` output rather than just `type`. This makes it much clearer what is
being tested.

### Removed

`VecBit` methods `iter` and `raw_len` moved to `SliceBit` in `0.3.0` but were
not removed in that release.

The remaining debugging `eprintln!` calls have been stripped.

## 0.3.0

Split `VecBit` off into `SliceBit` wherever possible.

### Added

- The `SliceBit` type is the `[T]` to `VecBit`'s `Vec<T>`. `VecBit` now `Deref`s
  to it, and has offloaded all the work that does not require managing allocated
  memory.
- Almost all of the public API on both types has documentation and example code.

### Changed

- The implementations of left- ard right- shift are now faster.
- `VecBit` can `Borrow` and `Deref` down to `SliceBit`, and offloads as much
  work as possible to it.
- `Clone` is more intelligent.

## 0.2.0

Improved the `vecbit!` macro.

### Changed

- `vecbit!` takes more syntaxes to better match `vec!`, and has better
  runtime performance. The increased static memory used by `vecbit!` should be
  more than counterbalanced by the vastly better generated code.

## 0.1.0

Initial implementation and release.

### Added

- `Endian` and `Bits` traits
- `VecBit` type with basic `Vec` idioms and parallel trait implementations
- `vecbit!` generator macro

[@GeorgeGkas]: https://github.com/GeorgeGkas
[@caelunshun]: https://github.com/caelunshun
[@geq1t]: https://github.com/geq1t
[@koushiro]: https://github.com/koushiro
[@overminder]: https://github.com/overminder
[@ratorx]: https://github.com/ratorx
[@schomatis]: https://github.com/schomatis
[@torce]: https://github.com/torce
[Issue #7]: https://github.com/sunjay/vecbit/issues/7
[Issue #8]: https://github.com/sunjay/vecbit/issues/8
[Issue #9]: https://github.com/sunjay/vecbit/issues/9
[Issue #10]: https://github.com/sunjay/vecbit/issues/10
[Issue #12]: https://github.com/sunjay/vecbit/issues/12
[Issue #15]: https://github.com/sunjay/vecbit/issues/15
[Issue #16]: https://github.com/sunjay/vecbit/issues/16
[Issue #28]: https://github.com/sunjay/vecbit/issues/28
[`Sync`]: https://doc.rust-lang.org/stable/core/marker/trait.Sync.html
[kac]: https://keepachangelog.com/en/1.0.0/
