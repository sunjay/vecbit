/*! `VecBit` structure

This module holds the main working type of the library. Clients can use
`SliceBit` directly, but `VecBit` is much more useful for most work.

The `SliceBit` module discusses the design decisions for the separation between
slice and vector types.
!*/

#![cfg(any(feature = "alloc", feature = "std"))]

use crate::{
	boxed::BitBox,
	cursor::{
		Cursor,
		Local,
	},
	indices::Indexable,
	pointer::BitPtr,
	slice::SliceBit,
	store::{
		BitStore,
		Word,
	},
};

#[cfg(feature = "alloc")]
use alloc::{
	borrow::{
		Borrow,
		BorrowMut,
		ToOwned,
	},
	boxed::Box,
	vec::Vec,
};

use core::{
	clone::Clone,
	cmp::{
		Eq,
		Ord,
		Ordering,
		PartialEq,
		PartialOrd,
	},
	convert::{
		AsMut,
		AsRef,
		From,
	},
	default::Default,
	fmt::{
		self,
		Debug,
		Display,
		Formatter,
	},
	hash::{
		Hash,
		Hasher,
	},
	iter::{
		self,
		DoubleEndedIterator,
		ExactSizeIterator,
		Extend,
		FromIterator,
		FusedIterator,
		Iterator,
		IntoIterator,
	},
	marker::{
		PhantomData,
		Send,
		Sync,
	},
	mem,
	ops::{
		Add,
		AddAssign,
		BitAnd,
		BitAndAssign,
		BitOr,
		BitOrAssign,
		BitXor,
		BitXorAssign,
		Deref,
		DerefMut,
		Drop,
		Index,
		IndexMut,
		Range,
		RangeBounds,
		RangeFrom,
		RangeFull,
		RangeInclusive,
		RangeTo,
		RangeToInclusive,
		Neg,
		Not,
		Shl,
		ShlAssign,
		Shr,
		ShrAssign,
		Sub,
		SubAssign,
	},
	ptr::{
		self,
		NonNull,
	},
	slice,
};

#[cfg(feature = "std")]
use std::{
	io::{
		self,
		Write,
	},
};

/** A compact [`Vec`] of bits, whose cursor and storage type can be customized.

`VecBit` is a newtype wrapper over `Vec`, and as such is exactly three words in
size on the stack.

# Examples

```rust
use vecbit::prelude::*;

let mut bv: VecBit = VecBit::new();
bv.push(false);
bv.push(true);

assert_eq!(bv.len(), 2);
assert_eq!(bv[0], false);

assert_eq!(bv.pop(), Some(true));
assert_eq!(bv.len(), 1);

bv.set(0, true);
assert_eq!(bv[0], true);

bv.extend([0u8, 1, 0].iter().map(|n| *n != 0u8));
for bit in &*bv {
  println!("{}", bit);
}
assert_eq!(bv, vecbit![1, 0, 1, 0]);
```

The [`vecbit!`] macro is provided to make initialization more convenient.

```rust
use vecbit::prelude::*;

let mut bv = vecbit![0, 1, 2, 3];
bv.push(false);
assert_eq!(bv, vecbit![0, 1, 1, 1, 0]);
```

It can also initialize each element of a `VecBit<_, T>` with a given value. This
may be more efficient than performing allocation and initialization in separate
steps, especially when initializing a vector of zeros:

```rust
use vecbit::prelude::*;

let bv = vecbit![0; 15];
assert_eq!(bv, vecbit![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

// The following is equivalent, but potentially slower:
let mut bv1: VecBit = VecBit::with_capacity(15);
bv1.resize(15, false);
```

Use a `VecBit<T>` as an efficient stack:

```rust
use vecbit::prelude::*;
let mut stack: VecBit = VecBit::new();

stack.push(false);
stack.push(true);
stack.push(true);

while let Some(top) = stack.pop() {
  //  Prints true, true, false
  println!("{}", top);
}
```

# Indexing

The `VecBit` type allows you to access values by index, because it implements
the [`Index`] trait. An example will be more explicit:

```rust
use vecbit::prelude::*;

let bv = vecbit![0, 0, 1, 1];
println!("{}", bv[1]); // it will display 'false'
```

However, be careful: if you try to access an index which isn’t in the `VecBit`,
your software will panic! You cannot do this:

```rust,should_panic
use vecbit::prelude::*;

let bv = vecbit![0, 1, 0, 1];
println!("{}", bv[6]); // it will panic!
```

In conclusion: always check if the index you want to get really exists before
doing it.

# Slicing

A `VecBit` is growable. A [`SliceBit`], on the other hand, is fixed size. To get
a bit slice, use `&`. Example:

```rust
use vecbit::prelude::*;
fn read_bitslice(slice: &SliceBit) {
	// use slice
}

let bv = vecbit![0, 1];
read_bitslice(&bv);

// … and that’s all!
// you can also do it like this:
let bs : &SliceBit = &bv;
```

In Rust, it’s more common to pass slices as arguments rather than vectors when
you do not want to grow or shrink it. The same goes for [`Vec`] and [`&[]`], and
[`String`] and [`&str`].

# Capacity and Reallocation

The capacity of a bit vector is the amount of space allocated for any future
bits that will be added onto the vector. This is not to be confused with the
*length* of a vector, which specifies the number of live, useful bits within the
vector. If a vector’s length exceeds its capacity, its capacity will
automatically be increased, but its storage elements will have to be
reallocated.

For example, a bit vector with capacity 10 and length 0 would be an allocated,
but uninhabited, vector, with space for ten more bits. Pushing ten or fewer bits
onto the vector will not change its capacity or cause reallocation to occur.
However, if the vector’s length is increased to eleven, it will have to
reallocate, which can be slow. For this reason, it is recommended to use
[`VecBit::with_capacity`] whenever possible to specify how big the bit vector is
expected to get.

# Guarantees

Due to its incredibly fundamental nature, `VecBit` makes a lot of guarantees
about its design. This ensures that it is as low-overhead as possible in the
general case, and can be correctly manipulated in fundamental ways by `unsafe`
code.

Most fundamentally, `VecBit` is and always will be a `([`BitPtr`], capacity)`
doublet. No more, no less. The order of these fields is unspecified, and you
should **only** interact with the members through the provided APIs. Note that
`BitPtr` is ***not directly manipulable***, and must ***never*** be written or
interpreted as anything but opaque binary data by user code.

When a `VecBit` has allocated memory, then the memory to which it points is on
the heap (as defined by the allocator Rust is configured to use by default), and
its pointer points to [`len`] initialized bits in order of the [`Cursor`] type
parameter, followed by `capacity - len` logically uninitialized bits.

`VecBit` will never perform a “small optimization” where elements are stored in
its handle representation, for two reasons:

- It would make it more difficult for user code to correctly manipulate a
  `VecBit`. The contents of the `VecBit` would not have a stable address if the
  handle were moved, and it would be more difficult to determine if a `VecBit`
  had allocated memory.

- It would penalize the general, heap-allocated, case by incurring a branch on
  every access.

`VecBit` will never automatically shrink itself, even if it is emptied. This
ensures that no unnecessary allocations or deallocations occur. Emptying a
`VecBit` and then refilling it to the same length will incur no calls to the
allocator. If you wish to free up unused memory, use [`shrink_to_fit`].

## Erasure

`VecBit` will not specifically overwrite any data that is removed from it, nor
will it specifically preserve it. Its uninitialized memory is scratch space that
may be used however the implementation desires, and must not be relied upon as
stable. Do not rely on removed data to be erased for security purposes. Even if
you drop a `VecBit`, its buffer may simply be reused for other data structures
in your program. Even if you zero a `VecBit`’s memory first, that may not
actually occur if the optimizer does not consider this an observable side
effect. There is one case that will never break, however: using `unsafe` to
construct a `[T]` slice over the `VecBit`’s capacity, and writing to the excess
space, then increasing the length to match, is always valid.

# Type Parameters

- `C: Cursor`: An implementor of the [`Cursor`] trait. This type is used to
  convert semantic indices into concrete bit positions in elements, and store or
  retrieve bit values from the storage type.
- `T: BitStore`: An implementor of the [`BitStore`] trait: `u8`, `u16`, `u32`,
  or `u64` (64-bit systems only). This is the actual type in memory that the
  vector will use to store data.

# Safety

The `VecBit` handle has the same *size* as standard Rust `Vec` handles, but it
is ***extremely binary incompatible*** with them. Attempting to treat
`VecBit<_, T>` as `Vec<T>` in any manner except through the provided APIs is
***catastrophically*** unsafe and unsound.

[`SliceBit`]: ../struct.SliceBit.html
[`VecBit::with_capacity`]: #method.with_capacity
[`BitStore`]: ../trait.BitStore.html
[`Cursor`]: ../trait.Cursor.html
[`Index`]: https://doc.rust-lang.org/stable/std/ops/trait.Index.html
[`String`]: https://doc.rust-lang.org/stable/std/string/struct.String.html
[`Vec`]: https://doc.rust-lang.org/stable/std/vec/struct.Vec.html
[`vecbit!`]: ../macro.vecbit.html
[`clear_on_drop`]: https://docs.rs/clear_on_drop
[`len`]: #method.len
[`shrink_to_fit`]: #method.shrink_to_fit
[`&str`]: https://doc.rust-lang.org/stable/std/primitive.str.html
[`&[]`]: https://doc.rust-lang.org/stable/std/primitive.slice.html
**/
#[repr(C)]
pub struct VecBit<C = Local, T = Word>
where C: Cursor, T: BitStore {
	/// Phantom `Cursor` member to satisfy the constraint checker.
	_cursor: PhantomData<C>,
	/// Slice pointer over the owned memory.
	pointer: BitPtr<T>,
	/// The number of *elements* this vector has allocated.
	capacity: usize,
}

impl<C, T> VecBit<C, T>
where C: Cursor, T: BitStore {
	/// Constructs a new, empty, `VecBit<C, T>`.
	///
	/// The vector does not allocate until bits are written into it.
	///
	/// # Returns
	///
	/// An empty, unallocated, `VecBit` handle.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let bv: VecBit = VecBit::new();
	/// assert!(bv.is_empty());
	/// assert_eq!(bv.capacity(), 0);
	/// ```
	pub fn new() -> Self {
		Self {
			_cursor: PhantomData,
			pointer: BitPtr::empty(),
			capacity: 0,
		}
	}

	/// Constructs a new, empty, `VecBit<T>` with the specified capacity.
	///
	/// The new vector will be able to hold at least `capacity` elements before
	/// it reallocates. If `capacity` is `0`, it will not allocate.
	///
	/// # Parameters
	///
	/// - `capacity`: The minimum number of bits that the new vector will need
	///   to be able to hold.
	///
	/// # Returns
	///
	/// An empty vector with at least the given capacity.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let bv: VecBit = VecBit::with_capacity(10);
	/// assert!(bv.is_empty());
	/// assert!(bv.capacity() >= 10);
	/// ```
	pub fn with_capacity(capacity: usize) -> Self {
		//  Find the number of elements needed to store the requested capacity
		//  of bits.
		let (cap, _) = 0u8.idx::<T>().span(capacity);
		//  Acquire a region of memory large enough for that element number.
		let (ptr, cap) = {
			let v = Vec::with_capacity(cap);
			let (ptr, cap) = (v.as_ptr(), v.capacity());
			mem::forget(v);
			(ptr, cap)
		};
		//  Take ownership of that region as an owned BitPtr
		Self {
			_cursor: PhantomData,
			pointer: BitPtr::uninhabited(ptr),
			capacity: cap,
		}
	}

	/// Constructs a `VecBit` from a single element.
	///
	/// The produced `VecBit` will span the element, and include all bits in it.
	///
	/// # Parameters
	///
	/// - `elt`: The source element.
	///
	/// # Returns
	///
	/// A `VecBit` over the provided element.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let bv = VecBit::<BigEndian, u8>::from_element(5);
	/// assert_eq!(bv.count_ones(), 2);
	/// ```
	pub fn from_element(elt: T) -> Self {
		Self::from_vec({
			let mut v = Vec::with_capacity(1);
			v.push(elt);
			v
		})
	}

	/// Constructs a `VecBit` from a slice of elements.
	///
	/// The produced `VecBit` will span the provided slice.
	///
	/// # Parameters
	///
	/// - `slice`: The source elements to copy into the new `VecBit`.
	///
	/// # Returns
	///
	/// A `VecBit` set to the provided slice values.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let src = [5, 10];
	/// let bv = VecBit::<BigEndian, u8>::from_slice(&src[..]);
	/// assert!(bv[5]);
	/// assert!(bv[7]);
	/// assert!(bv[12]);
	/// assert!(bv[14]);
	/// ```
	pub fn from_slice(slice: &[T]) -> Self {
		SliceBit::<C, T>::from_slice(slice).to_owned()
	}

	/// Consumes a `Vec<T>` and creates a `VecBit<C, T>` from it.
	///
	/// # Parameters
	///
	/// - `vec`: The source vector whose memory will be used.
	///
	/// # Returns
	///
	/// A new `VecBit` using the `vec` `Vec`’s memory.
	///
	/// # Panics
	///
	/// Panics if the source vector would cause the `VecBit` to overflow
	/// capacity.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let bv = VecBit::<BigEndian, u8>::from_vec(vec![1, 2, 4, 8]);
	/// assert_eq!(
	///   "[00000001, 00000010, 00000100, 00001000]",
	///   &format!("{}", bv),
	/// );
	/// ```
	pub fn from_vec(vec: Vec<T>) -> Self {
		let len = vec.len();
		assert!(
			len <= BitPtr::<T>::MAX_ELTS,
			"Vector length {} overflows {}",
			len,
			BitPtr::<T>::MAX_ELTS,
		);
		let bs = SliceBit::<C, T>::from_slice(&vec[..]);
		let pointer = bs.bitptr();
		let capacity = vec.capacity();
		mem::forget(vec);
		Self {
			_cursor: PhantomData,
			pointer,
			capacity,
		}
	}

	/// Clones a `&SliceBit` into a `VecBit`.
	///
	/// # Parameters
	///
	/// - `slice`
	///
	/// # Returns
	///
	/// A `VecBit` containing the same bits as the source slice.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let bs = [0u8, !0].bits::<BigEndian>();
	/// let bv = VecBit::from_bitslice(bs);
	/// assert_eq!(bv.len(), 16);
	/// assert!(bv.some());
	/// ```
	pub fn from_bitslice(slice: &SliceBit<C, T>) -> Self {
		Self::from_iter(slice.iter())
	}

	/// Converts a frozen `BitBox` allocation into a growable `VecBit`.
	///
	/// This does not copy or reallocate.
	///
	/// # Parameters
	///
	/// - `slice`: A `BitBox` to be thawed.
	///
	/// # Returns
	///
	/// A growable collection over the original memory of the slice.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let bv = VecBit::from_boxed_bitslice(bitbox![0, 1]);
	/// assert_eq!(bv.len(), 2);
	/// assert!(bv.some());
	/// ```
	pub fn from_boxed_bitslice(slice: BitBox<C, T>) -> Self {
		let bitptr = slice.bitptr();
		mem::forget(slice);
		unsafe { Self::from_raw_parts(bitptr, bitptr.elements()) }
	}

	/// Creates a new `VecBit<C, T>` directly from the raw parts of another.
	///
	/// # Parameters
	///
	/// - `pointer`: The `BitPtr<T>` to use.
	/// - `capacity`: The number of `T` elements *allocated* in that slab.
	///
	/// # Returns
	///
	/// A `VecBit` over the given slab of memory.
	///
	/// # Safety
	///
	/// This is ***highly*** unsafe, due to the number of invariants that aren’t
	/// checked:
	///
	/// - `pointer` needs to have been previously allocated by some allocating
	///   type.
	/// - `pointer`’s `T` needs to have the same size ***and alignment*** as it
	///   was initially allocated.
	/// - `pointer`’s element count needs to be less than or equal to the
	///   original allocation capacity.
	/// - `capacity` needs to be the original allocation capacity for the
	///   vector. This is *not* the value produced by `.capacity()`.
	///
	/// Violating these ***will*** cause problems, like corrupting the handle’s
	/// concept of memory, the allocator’s internal data structures, and the
	/// sanity of your program. It is ***absolutely*** not safe to construct a
	/// `VecBit` whose `T` differs from the type used for the initial
	/// allocation.
	///
	/// The ownership of `pointer` is effectively transferred to the
	/// `VecBit<C, T>` which may then deallocate, reallocate, or modify the
	/// contents of the referent slice at will. Ensure that nothing else uses
	/// the pointer after calling this function.
	pub unsafe fn from_raw_parts(pointer: BitPtr<T>, capacity: usize) -> Self {
		Self {
			_cursor: PhantomData,
			pointer,
			capacity,
		}
	}

	/// Returns the number of bits the vector can hold without reallocating.
	///
	/// # Parameters
	///
	/// - `&self`
	///
	/// # Returns
	///
	/// The number of bits that the vector can hold before reallocating.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let bv: VecBit = VecBit::with_capacity(10);
	/// assert!(bv.is_empty());
	/// assert!(bv.capacity() >= 10);
	/// ```
	pub fn capacity(&self) -> usize {
		self.capacity
			.checked_mul(T::BITS as usize)
			.expect("Vector capacity overflow")
	}

	/// Reserves capacity for at least `additional` more bits to be inserted.
	///
	/// The collection may reserve more space to avoid frequent reallocations.
	/// After calling `reserve`, capacity will be greater than or equal to
	/// `self.len() + additional`. Does nothing if the capacity is already
	/// sufficient.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `additional`: The number of extra bits to be granted space.
	///
	/// # Panics
	///
	/// Panics if the new capacity would overflow the vector’s limits.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut bv = vecbit![1; 5];
	/// assert!(bv.capacity() >= 5);
	/// bv.reserve(10);
	/// assert!(bv.capacity() >= 15);
	/// ```
	pub fn reserve(&mut self, additional: usize) {
		let newlen = self.len() + additional;
		assert!(
			newlen <= BitPtr::<T>::MAX_BITS,
			"Capacity overflow: {} exceeds {}",
			newlen,
			BitPtr::<T>::MAX_BITS,
		);
		//  Compute the number of additional elements needed to store the
		//  requested number of additional bits.
		let (e, _) = self.pointer.tail().span(additional);
		self.do_unto_vec(|v| v.reserve(e));
	}

	/// Reserves the minimum capacity for at least `additional` more bits.
	///
	/// After calling `reserve_exact`, the capacity will be greater than or
	/// equal to `self.len() + additional`. Does nothing if the capacity is
	/// already sufficient.
	///
	/// Note that the allocator may give the collection more space than it
	/// requests. Therefore, the capacity cannot be relied upon to be precisely
	/// minimal. Prefer `reserve` if future insertions are expected.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `additional`: The number of extra bits to be granted space.
	///
	/// # Panics
	///
	/// Panics if the new capacity would overflow the vector’s limits.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut bv = vecbit![1; 5];
	/// assert!(bv.capacity() >= 5);
	/// bv.reserve_exact(10);
	/// assert!(bv.capacity() >= 15);
	/// ```
	pub fn reserve_exact(&mut self, additional: usize) {
		let newlen = self.len() + additional;
		assert!(
			newlen <= BitPtr::<T>::MAX_BITS,
			"Capacity overflow: {} exceeds {}",
			newlen,
			BitPtr::<T>::MAX_BITS,
		);
		//  Compute the number of additional elements needed to store the
		//  requested number of additional bits.
		let (e, _) = self.pointer.tail().span(additional);
		self.do_unto_vec(|v| v.reserve_exact(e));
	}

	/// Shrinks the capacity of the vector as much as possible.
	///
	/// It will drop down as close as possible to the length, but the allocator
	/// may still inform the vector that there is space for bits.
	///
	/// This does not modify the contents of the memory store! It will not zero
	/// any memory that had been used and then removed from the vector’s live
	/// count.
	///
	/// # Parameters
	///
	/// - `&mut self`
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut bv = vecbit![1; 100];
	/// let cap = bv.capacity();
	/// bv.truncate(10);
	/// bv.shrink_to_fit();
	/// assert!(bv.capacity() <= cap);
	/// ```
	pub fn shrink_to_fit(&mut self) {
		self.do_unto_vec(Vec::shrink_to_fit);
	}

	/// Shortens the vector, keeping the first `len` bits and dropping the rest.
	///
	/// If `len` is greater than the vector’s current length, this has no
	/// effect.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `len`: The new length of the vector.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut bv = vecbit![1; 15];
	/// bv.truncate(10);
	/// assert_eq!(bv.len(), 10);
	///
	/// bv.truncate(15);
	/// assert_eq!(bv.len(), 10);
	/// ```
	pub fn truncate(&mut self, len: usize) {
		if len < self.len() {
			unsafe { self.bitptr_mut().set_len(len); }
		}
	}

	/// Produces a `SliceBit` containing the entire vector.
	///
	/// Equivalent to `&s[..]`.
	///
	/// # Parameters
	///
	/// - `&self`
	///
	/// # Returns
	///
	/// A `SliceBit` over the vector.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let bv = vecbit![0, 1, 1, 0];
	/// let bs = bv.as_bitslice();
	/// ```
	pub fn as_bitslice(&self) -> &SliceBit<C, T> {
		self.pointer.into_bitslice()
	}

	/// Produces a mutable `SliceBit` containing the entire vector.
	///
	/// Equivalent to `&mut s[..]`.
	///
	/// # Parameters
	///
	/// - `&mut self`
	///
	/// # Returns
	///
	/// A mutable `SliceBit` over the vector.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut bv = vecbit![0, 1, 1, 0];
	/// let bs = bv.as_mut_bitslice();
	/// ```
	pub fn as_mut_bitslice(&mut self) -> &mut SliceBit<C, T> {
		self.pointer.into_bitslice_mut()
	}

	/// Accesses the vector’s backing store as an element slice.
	///
	/// Unlike `SliceBit`’s method of the same name, this includes the partial
	/// edges, as `VecBit` forbids fragmentation that leads to contention.
	///
	/// # Parameters
	///
	/// - `&self`
	///
	/// # Returns
	///
	/// The slice of all live elements in the backing storage, including the
	/// partial edges if present.
	pub fn as_slice(&self) -> &[T] {
		self.bitptr().as_slice()
	}

	/// Accesses the vector’s backing store as an element slice.
	///
	/// Unlike `SliceBit`’s method of the same name, this includes the partial
	/// edges, as `VecBit` forbids fragmentation that leads to contention.
	///
	/// # Parameters
	///
	/// - `&mut self`
	///
	/// # Returns
	///
	/// The slice of all live elements in the backing storage, including the
	/// partial edges if present.
	pub fn as_mut_slice(&mut self) -> &mut [T] {
		self.bitptr().as_mut_slice()
	}

	/// Sets the length of the vector.
	///
	/// This unconditionally sets the size of the vector, without modifying its
	/// contents. It is up to the caller to ensure that the vector’s buffer can
	/// hold the new size.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `len`: The new length of the vector. This must be less than the
	///   maximum number of bits that the vector can hold.
	///
	/// # Panics
	///
	/// This panics if `len` overflows the vector's intrinsic *or allocated*
	/// capacities.
	///
	/// # Safety
	///
	/// The caller must ensure that the new length is sound for the vector.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut bv: VecBit = VecBit::with_capacity(15);
	/// assert!(bv.is_empty());
	/// unsafe { bv.set_len(10) };
	/// assert_eq!(bv.len(), 10);
	/// ```
	pub unsafe fn set_len(&mut self, len: usize) {
		assert!(
			len <= BitPtr::<T>::MAX_BITS,
			"Capacity overflow: {} overflows maximum length {}",
			len,
			BitPtr::<T>::MAX_BITS,
		);
		assert!(
			len <= self.capacity(),
			"Capacity overflow: {} overflows allocation size {}",
			len,
			self.capacity(),
		);
		self.bitptr_mut().set_len(len);
	}

	/// Removes a bit from the vector and returns it.
	///
	/// The removed bit is replaced by the last bit in the vector.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `index`: The index whose bit is to be returned, and replaced by the
	///   tail.
	///
	/// # Returns
	///
	/// The bit at the requested index.
	///
	/// # Panics
	///
	/// Panics if the index is out of bounds.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut bv = vecbit![0, 0, 0, 0, 1];
	/// assert!(!bv[2]);
	/// assert_eq!(bv.len(), 5);
	/// assert!(!bv.swap_remove(2));
	/// assert!(bv[2]);
	/// assert_eq!(bv.len(), 4);
	/// ```
	pub fn swap_remove(&mut self, index: usize) -> bool {
		let len = self.len();
		assert!(index < len, "Index {} out of bounds: {}", index, len);
		self.swap(index, len - 1);
		self.pop()
			.expect("VecBit::swap_remove cannot fail after index validation")
	}

	/// Inserts a bit at a position, shifting all bits after it to the right.
	///
	/// Note that this is `O(n)` runtime.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `index`: The position at which to insert. This may be any value from
	///   `0` up to *and including* `self.len()`. At `self.len()`, it is
	///   equivalent to calling `self.push(value)`.
	/// - `value`: The bit to be inserted.
	///
	/// # Panics
	///
	/// Panics if `index` is greater than the length.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut bv = vecbit![0, 0, 0, 0];
	/// bv.insert(2, true);
	/// assert_eq!(bv, vecbit![0, 0, 1, 0, 0]);
	/// bv.insert(5, true);
	/// assert_eq!(bv, vecbit![0, 0, 1, 0, 0, 1]);
	/// ```
	pub fn insert(&mut self, index: usize, value: bool) {
		let len = self.len();
		assert!(index <= len, "Index {} is out of bounds: {}", index, len);
		self.push(value);
		self[index ..].rotate_right(1);
	}

	/// Removes and returns the bit at position `index`, shifting all bits after
	/// it to the left.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `index`: The position whose bit is to be removed. This must be in the
	///   domain `0 .. self.len()`.
	///
	/// # Returns
	///
	/// The bit at the requested index.
	///
	/// # Panics
	///
	/// Panics if `index` is out of bounds for the vector.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut bv = vecbit![0, 0, 1, 0, 0];
	/// assert!(bv.remove(2));
	/// assert_eq!(bv, vecbit![0, 0, 0, 0]);
	/// ```
	pub fn remove(&mut self, index: usize) -> bool {
		let len = self.len();
		assert!(index < len, "Index {} is out of bounds: {}", index, len);
		self[index ..].rotate_left(1);
		self.pop()
			.expect("VecBit::remove cannot fail after index validation")
	}

	/// Retains only the bits that pass the predicate.
	///
	/// This removes all bits `b` where `f(e)` returns `false`. This method
	/// operates in place and preserves the order of the retained bits. Because
	/// it is in-place, it operates in `O(n²)` time.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `pred`: The testing predicate for each bit.
	///
	/// # Type Parameters
	///
	/// - `F: FnMut(usize, bool) -> bool`: A function that can be invoked on
	///   each bit, returning whether the bit should be kept or not. Receives
	///   the index (following [`SliceBit::for_each`]) to provide additional
	///   context to determine whether the entry satisfies the condition.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut bv = vecbit![0, 1, 0, 1, 0, 1];
	/// bv.retain(|_, b| b);
	/// assert_eq!(bv, vecbit![1, 1, 1]);
	/// ```
	///
	/// [`SliceBit::for_each`]: ../slice/struct.SliceBit.html#method.for_each
	pub fn retain<F>(&mut self, mut pred: F)
	where F: FnMut(usize, bool) -> bool {
		for n in (0 .. self.len()).rev() {
			if !pred(n, self[n]) {
				self.remove(n);
			}
		}
	}

	/// Appends a bit to the back of the vector.
	///
	/// If the vector is at capacity, this may cause a reallocation.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `value`: The bit value to append.
	///
	/// # Panics
	///
	/// This will panic if the push will cause the vector to allocate above
	/// `BitPtr<T>` or machine capacity.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut bv: VecBit = VecBit::new();
	/// assert!(bv.is_empty());
	/// bv.push(true);
	/// assert_eq!(bv.len(), 1);
	/// assert!(bv[0]);
	/// ```
	pub fn push(&mut self, value: bool) {
		let len = self.len();
		assert!(
			len <= BitPtr::<T>::MAX_BITS,
			"Capacity overflow: {} >= {}",
			len,
			BitPtr::<T>::MAX_BITS,
		);
		//  If self is empty *or* tail is at the back edge of an element, push
		//  an element onto the vector.
		if self.is_empty() || *self.pointer.tail() == T::BITS {
			self.do_unto_vec(|v| v.push(0.into()));
		}
		//  At this point, it is always safe to increment the tail, and then
		//  write to the newly live bit.
		unsafe { self.bitptr_mut().incr_tail() };
		self.set(len, value);
	}

	/// Removes the last bit from the collection, if present.
	///
	/// # Parameters
	///
	/// - `&mut self`
	///
	/// # Returns
	///
	/// If the vector is not empty, this returns the last bit; if it is empty,
	/// this returns `None`.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut bv: VecBit = VecBit::new();
	/// assert!(bv.is_empty());
	/// bv.push(true);
	/// assert_eq!(bv.len(), 1);
	/// assert!(bv[0]);
	///
	/// assert!(bv.pop().unwrap());
	/// assert!(bv.is_empty());
	/// assert!(bv.pop().is_none());
	/// ```
	pub fn pop(&mut self) -> Option<bool> {
		if self.is_empty() {
			return None;
		}
		let out = self[self.len() - 1];
		unsafe { self.bitptr_mut().decr_tail() };
		Some(out)
	}

	/// Moves all the elements of `other` into `self`, leaving `other` empty.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `other`: A `VecBit` of any order and storage type. Its bits are
	///   appended to `self`.
	///
	/// # Panics
	///
	/// Panics if the joined vector is too large.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut bv1 = vecbit![0; 10];
	/// let mut bv2 = vecbit![1; 10];
	/// bv1.append(&mut bv2);
	/// assert_eq!(bv1.len(), 20);
	/// assert!(bv1[10]);
	/// assert!(bv2.is_empty());
	/// ```
	pub fn append<D, U>(&mut self, other: &mut VecBit<D, U>)
	where D: Cursor, U: BitStore {
		self.extend(other.iter());
		other.clear();
	}

	/// Creates a draining iterator that removes the specified range from the
	/// vector and yields the removed bits.
	///
	/// # Notes
	///
	/// 1. The element range is removed, regardless of whether the iterator is
	///    consumed.
	/// 2. The amount of items removed from the vector if the draining iterator
	///    is leaked, is left unspecified.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `range`: any range literal, which is used to define the range of the
	///   vector that is drained.
	///
	/// # Returns
	///
	/// An iterator over the specified range.
	///
	/// # Panics
	///
	/// Panics if the range is ill-formed, or if it is beyond the vector bounds.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut bv = vecbit![0, 0, 1, 1, 1, 0, 0];
	/// assert_eq!(bv.len(), 7);
	/// for bit in bv.drain(2 .. 5) {
	///   assert!(bit);
	/// }
	/// assert!(bv.not_any());
	/// assert_eq!(bv.len(), 4);
	/// ```
	pub fn drain<R>(&mut self, range: R) -> Drain<C, T>
	where R: RangeBounds<usize> {
		use core::ops::Bound::*;
		let len = self.len();
		let from = match range.start_bound() {
			Included(&n) => n,
			Excluded(&n) => n + 1,
			Unbounded   => 0,
		};
		//  First index beyond the end of the drain.
		let upto = match range.end_bound() {
			Included(&n) => n + 1,
			Excluded(&n) => n,
			Unbounded    => len,
		};
		assert!(from <= upto, "The drain start must be below the drain end");
		assert!(upto <= len, "The drain end must be within the vector bounds");

		unsafe {
			let ranging: &SliceBit<C, T> = self
				.as_bitslice()[from .. upto]
				//  remove the lifetime and borrow awareness
				.bitptr()
				.into_bitslice();
			self.set_len(from);

			Drain {
				vecbit: NonNull::from(self),
				iter: ranging.iter(),
				tail_start: upto,
				tail_len: len - upto,
			}
		}
	}

	/// Clears the vector, removing all values.
	///
	/// Note that this method has no effect on the allocated capacity of the
	/// vector.
	///
	/// # Parameters
	///
	/// - `&mut self`
	///
	/// # Effects
	///
	/// Becomes an uninhabited slice.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut bv = vecbit![1; 30];
	/// assert_eq!(bv.len(), 30);
	/// assert!(bv.iter().all(|b| b));
	/// bv.clear();
	/// assert!(bv.is_empty());
	/// ```
	///
	/// After calling `clear()`, `bv` will no longer show raw memory, so the
	/// above test cannot show that the underlying memory is not altered. This
	/// is also an implementation detail on which you should not rely.
	pub fn clear(&mut self) {
		unsafe { self.set_len(0) }
	}

	/// Splits the collection into two at the given index.
	///
	/// Returns a newly allocated `Self`. `self` contains elements `[0, at)`,
	/// and the returned `Self` contains elements `[at, self.len())`.
	///
	/// Note that the capacity of `self` does not change.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `at`: The index at which to perform the split. This must be in the
	///   domain `0 ..= self.len()`. When it is `self.len()`, an empty vector is
	///   returned.
	///
	/// # Returns
	///
	/// A new `VecBit` containing all the elements from `at` onwards.
	///
	/// # Panics
	///
	/// Panics if `at` is beyond `self.len()`.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut bv1 = vecbit![0, 0, 0, 1, 1, 1];
	/// let bv2 = bv1.split_off(3);
	/// assert_eq!(bv1, vecbit![0, 0, 0]);
	/// assert_eq!(bv2, vecbit![1, 1, 1]);
	/// ```
	pub fn split_off(&mut self, at: usize) -> Self {
		let len = self.len();
		assert!(at <= len, "Index out of bounds: {} is beyond {}", at, len);
		match at {
			0 => unsafe {
				let out = Self::from_raw_parts(self.pointer, self.capacity);
				ptr::write(self, Self::new());
				out
			},
			n if n == len => Self::new(),
			_ => {
				let out = self.as_bitslice().iter().skip(at).collect();
				self.truncate(at);
				out
			},
		}
	}

	/// Resizes the `VecBit` in place so that `len` is equal to `new_len`.
	///
	/// If `new_len` is greater than `len`, then  the vector is extended by the
	/// difference, and filled with the provided value. If `new_len` is less
	/// than `len`, then the vector is just truncated.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `new_len`: The new length of the vector.
	/// - `value`: The fill value if the vector is to be extended.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut bv = vecbit![0; 4];
	/// bv.resize(8, true);
	/// assert_eq!(bv, vecbit![0, 0, 0, 0, 1, 1, 1, 1]);
	/// bv.resize(5, false);
	/// assert_eq!(bv, vecbit![0, 0, 0, 0, 1]);
	/// ```
	pub fn resize(&mut self, new_len: usize, value: bool) {
		let len = self.len();
		if new_len < len {
			self.truncate(new_len);
		}
		else if new_len > len {
			self.extend(iter::repeat(value).take(new_len - len));
		}
	}

	/// Creates a splicing iterator that exchanges the specified range for the
	/// `replacement` iterator, yielding the removed items. The range and its
	/// replacement do not need to be the same size.
	///
	/// # Notes
	///
	/// 1. The element range is removed and replaced even if the iterator
	///    produced by this method is not fully consumed.
	/// 2. It is unspecified how many bits are removed from the `VecBit` if the
	///    returned iterator is leaked.
	/// 3. The input iterator `replacement` is only consumed when the returned
	///    iterator is dropped.
	/// 4. This is optimal if:
	///    - the tail (elements in the `VecBit` after `range`) is empty,
	///    - `replace_with` yields fewer characters than `range`’s length,
	///    - the lower bound of `replacement.size_hint()` is exact.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `range`: A range of indices in the `VecBit` to pull out of the
	///   collection.
	/// - `replacement`: Something which can be used to provide new bits to
	///   replace the removed range.
	///
	/// The entirety of `replacement` will be inserted into the slot marked by
	/// `range`. If `replacement` is an infinite iterator, then this will hang,
	/// and crash your program.
	///
	/// # Returns
	///
	/// An iterator over the bits marked by `range`.
	///
	/// # Panics
	///
	/// Panics if the range is ill-formed, or extends past the end of the
	/// `VecBit`.
	///
	/// # Examples
	///
	/// This example starts with six bits of zero, and then splices out bits 2
	/// and 3 and replaces them with four bits of one.
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut bv = vecbit![0; 6];
	/// let bv2 = vecbit![1; 4];
	///
	/// let s = bv.splice(2 .. 4, bv2).collect::<VecBit>();
	/// assert_eq!(s.len(), 2);
	/// assert!(!s[0]);
	/// assert_eq!(bv, vecbit![0, 0, 1, 1, 1, 1, 0, 0]);
	/// ```
	pub fn splice<R, I>(
		&mut self,
		range: R,
		replacement: I,
	) -> Splice<C, T, <I as IntoIterator>::IntoIter>
	where R: RangeBounds<usize>, I: IntoIterator<Item=bool> {
		Splice {
			drain: self.drain(range),
			splice: replacement.into_iter(),
		}
	}

	/// Sets the backing storage to the provided element.
	///
	/// This unconditionally sets each allocated element in the backing storage
	/// to the provided value, without altering the `VecBit` length or capacity.
	/// It operates on the underlying `Vec`’s memory region directly, and will
	/// ignore the `VecBit`’s cursors.
	///
	/// This has the unobservable effect of setting the allocated, but dead,
	/// bits beyond the end of the vector’s *length*, up to its *capacity*.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `element`: The value to which each allocated element in the backing
	///   store will be set.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut bv = vecbit![Local, u8; 0; 10];
	/// assert_eq!(bv.as_slice(), &[0, 0]);
	/// bv.set_elements(0xA5);
	/// assert_eq!(bv.as_slice(), &[0xA5, 0xA5]);
	/// ```
	pub fn set_elements(&mut self, element: T) {
		self.do_unto_vec(|v| {
			let (ptr, cap) = (v.as_mut_ptr(), v.capacity());
			for elt in unsafe { slice::from_raw_parts_mut(ptr, cap) } {
				*elt = element;
			}
		})
	}

	/// Performs “reverse” addition (left to right instead of right to left).
	///
	/// This addition traverses the addends from left to right, performing
	/// the addition at each index and writing the sum into `self`.
	///
	/// If `addend` expires before `self` does, `addend` is zero-extended and
	/// the carry propagates through the rest of `self`. If `self` expires
	/// before `addend`, then `self` is zero-extended and the carry propagates
	/// through the rest of `addend`, growing `self` until `addend` expires.
	///
	/// An infinite `addend` will cause unbounded memory growth until the vector
	/// overflows and panics.
	///
	/// # Parameters
	///
	/// - `self`
	/// - `addend: impl IntoIterator<Item=bool>`: A stream of bits to add into
	///   `self`, from left to right.
	///
	/// # Returns
	///
	/// The sum vector of `self` and `addend`.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let a = vecbit![0, 1, 0, 1];
	/// let b = vecbit![0, 0, 1, 1];
	/// let c = a.add_reverse(b);
	/// assert_eq!(c, vecbit![0, 1, 1, 0, 1]);
	/// ```
	pub fn add_reverse<I>(mut self, addend: I) -> Self
	where I: IntoIterator<Item=bool> {
		self.add_assign_reverse(addend);
		self
	}

	/// Performs “reverse” addition (left to right instead of right to left).
	///
	/// This addition traverses the addends from left to right, performing
	/// the addition at each index and writing the sum into `self`.
	///
	/// If `addend` expires before `self` does, `addend` is zero-extended and
	/// the carry propagates through the rest of `self`. If `self` expires
	/// before `addend`, then `self` is zero-extended and the carry propagates
	/// through the rest of `addend`, growing `self` until `addend` expires.
	///
	/// An infinite `addend` will cause unbounded memory growth until the vector
	/// overflows and panics.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `addend: impl IntoIterator<Item=bool>`: A stream of bits to add into
	///   `self`, from left to right.
	///
	/// # Effects
	///
	/// `self` may grow as a result of the final carry-out bit being `1` and
	/// pushed onto the right end.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut a = vecbit![0, 1, 0, 1];
	/// let     b = vecbit![0, 0, 1, 1];
	/// a.add_assign_reverse(&b);
	/// assert_eq!(a, vecbit![0, 1, 1, 0, 1]);
	/// ```
	pub fn add_assign_reverse<I>(&mut self, addend: I)
	where I: IntoIterator<Item=bool> {
		//  Set up iteration over the addend
		let mut addend = addend.into_iter().fuse();
		//  Delegate to the `SliceBit` implementation for the initial addition.
		//  If `addend` expires first, it zero-extends; if `self` expires first,
		//  `addend` will still have its remnant for the next stage.
		let mut c = self.as_mut_bitslice().add_assign_reverse(addend.by_ref());
		//  If `addend` still has bits to provide, zero-extend `self` and add
		//  them in.
		for b in addend {
			let (y, z) = crate::rca1(false, b, c);
			self.push(y);
			c = z;
		}
		if c {
			self.push(true);
		}
	}

	/// Changes the cursor type on the vector handle, without changing its
	/// contents.
	///
	/// # Parameters
	///
	/// - `self`
	///
	/// # Returns
	///
	/// An equivalent vector handle with a new cursor type. The contents of the
	/// backing storage are unchanged.
	///
	/// To reorder the bits in memory, drain this vector into a new handle with
	/// the desired cursor type.
	pub fn change_cursor<D>(self) -> VecBit<D, T>
	where D: Cursor {
		let (bp, cap) = (self.pointer, self.capacity);
		mem::forget(self);
		unsafe { VecBit::from_raw_parts(bp, cap) }
	}

	/// Degrades a `VecBit` to a `BitBox`, freezing its size.
	///
	/// # Parameters
	///
	/// - `self`
	///
	/// # Returns
	///
	/// Itself, with its size frozen and ungrowable.
	pub fn into_boxed_bitslice(self) -> BitBox<C, T> {
		let pointer = self.pointer;
		//  Convert the Vec allocation into a Box<[T]> allocation
		mem::forget(self.into_boxed_slice());
		unsafe { BitBox::from_raw(pointer) }
	}

	/// Degrades a `VecBit` to a standard boxed slice.
	///
	/// # Parameters
	///
	/// - `self`
	///
	/// # Returns
	///
	/// A boxed slice of the data the `VecBit` had owned.
	pub fn into_boxed_slice(self) -> Box<[T]> {
		self.into_vec().into_boxed_slice()
	}

	/// Degrades a `VecBit` to a standard `Vec`.
	///
	/// # Parameters
	///
	/// - `self`
	///
	/// # Returns
	///
	/// The plain vector underlying the `VecBit`.
	pub fn into_vec(self) -> Vec<T> {
		let slice = self.pointer.as_mut_slice();
		let out = unsafe {
			Vec::from_raw_parts(slice.as_mut_ptr(), slice.len(), self.capacity)
		};
		mem::forget(self);
		out
	}

	/// Gets the raw `BitPtr` powering the vector.
	///
	/// # Parameters
	///
	/// - `&self`
	///
	/// # Returns
	///
	/// The underlying `BitPtr` for the vector.
	pub(crate) fn bitptr(&self) -> BitPtr<T> {
		self.pointer
	}

	/// Gives write access to the `BitPtr` structure powering the vector.
	///
	/// # Parameters
	///
	/// - `&mut self`
	///
	/// # Returns
	///
	/// A mutable reference to the interior `BitPtr`.
	pub(crate) fn bitptr_mut(&mut self) -> &mut BitPtr<T> {
		&mut self.pointer
	}

	/// Permits a function to modify the `Vec<T>` underneath a `VecBit<_, T>`.
	///
	/// This produces a `Vec<T>` structure referring to the same data region as
	/// the `VecBit<_, T>`, allows a function to mutably view it, and then
	/// forgets the `Vec<T>` after the function concludes.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `func`: A function which receives a mutable borrow to the `Vec<T>`
	///   underlying the `VecBit<_, T>`.
	///
	/// # Type Parameters
	///
	/// - `F: FnOnce(&mut Vec<T>) -> R`: Any callable object (function or
	///   closure) which receives a mutable borrow of a `Vec<T>`.
	///
	/// - `R`: The return value from the called function or closure.
	fn do_unto_vec<F, R>(&mut self, func: F) -> R
	where F: FnOnce(&mut Vec<T>) -> R {
		let slice = self.pointer.as_mut_slice();
		let mut v = unsafe {
			Vec::from_raw_parts(slice.as_mut_ptr(), slice.len(), self.capacity)
		};
		let out = func(&mut v);
		//  The only change is that the pointer might relocate. The region data
		//  will remain untouched. Vec guarantees it will never produce an
		//  invalid pointer.
		unsafe { self.bitptr_mut().set_pointer(v.as_ptr()); }
		// self.pointer = unsafe { BitPtr::new_unchecked(v.as_ptr(), e, h, t) };
		self.capacity = v.capacity();
		mem::forget(v);
		out
	}

	/// Permits a function to view the `Vec<T>` underneath a `VecBit<_, T>`.
	///
	/// This produces a `Vec<T>` structure referring to the same data region as
	/// the `VecBit<_, T>`, allows a function to immutably view it, and then
	/// forgets the `Vec<T>` after the function concludes.
	///
	/// # Parameters
	///
	/// - `&self`
	/// - `func`: A function which receives an immutable borrow to the `Vec<T>`
	///   underlying the `VecBit<_, T>`.
	///
	/// # Returns
	///
	/// The return value of `func`.
	///
	/// # Type Parameters
	///
	/// - `F: FnOnce(&Vec<T>)`: Any callable object (function or closure) which
	///   receives an immutable borrow of a `Vec<T>` and returns nothing.
	///
	/// # Safety
	///
	/// This produces an empty `Vec<T>` if the `VecBit<_, T>` is empty.
	fn do_with_vec<F, R>(&self, func: F) -> R
	where F: FnOnce(&Vec<T>) -> R {
		let slice = self.pointer.as_mut_slice();
		let v: Vec<T> = unsafe {
			Vec::from_raw_parts(slice.as_mut_ptr(), slice.len(), self.capacity)
		};
		let out = func(&v);
		mem::forget(v);
		out
	}
}

/// Signifies that `SliceBit` is the borrowed form of `VecBit`.
impl<C, T> Borrow<SliceBit<C, T>> for VecBit<C, T>
where C: Cursor, T: BitStore {
	/// Borrows the `VecBit` as a `SliceBit`.
	///
	/// # Parameters
	///
	/// - `&self`
	///
	/// # Returns
	///
	/// A borrowed `SliceBit` of the vector.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	/// use std::borrow::Borrow;
	///
	/// let bv = vecbit![0; 13];
	/// let bs: &SliceBit = bv.borrow();
	/// assert!(!bs[10]);
	/// ```
	fn borrow(&self) -> &SliceBit<C, T> {
		self.as_bitslice()
	}
}

/// Signifies that `SliceBit` is the borrowed form of `VecBit`.
impl<C, T> BorrowMut<SliceBit<C, T>> for VecBit<C, T>
where C: Cursor, T: BitStore {
	/// Mutably borrows the `VecBit` as a `SliceBit`.
	///
	/// # Parameters
	///
	/// - `&mut self`
	///
	/// # Returns
	///
	/// A mutably borrowed `SliceBit` of the vector.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	/// use std::borrow::BorrowMut;
	///
	/// let mut bv = vecbit![0; 13];
	/// let bs: &mut SliceBit = bv.borrow_mut();
	/// assert!(!bs[10]);
	/// bs.set(10, true);
	/// assert!(bs[10]);
	/// ```
	fn borrow_mut(&mut self) -> &mut SliceBit<C, T> {
		self.as_mut_bitslice()
	}
}

impl<C, T> Clone for VecBit<C, T>
where C: Cursor, T: BitStore {
	fn clone(&self) -> Self {
		let new_vec = self.do_with_vec(Clone::clone);
		let capacity = new_vec.capacity();
		let mut pointer = self.pointer;
		unsafe { pointer.set_pointer(new_vec.as_ptr()); }
		mem::forget(new_vec);
		Self {
			_cursor: PhantomData,
			pointer, // unsafe { BitPtr::new_unchecked(ptr, e, h, t) },
			capacity,
		}
	}

	fn clone_from(&mut self, other: &Self) {
		let slice = other.pointer.as_slice();
		self.clear();
		//  Copy the other data region into the underlying vector, then grab its
		//  pointer and capacity values.
		let (ptr, capacity) = self.do_unto_vec(|v| {
			v.copy_from_slice(slice);
			(v.as_ptr(), v.capacity())
		});
		//  Copy the other `BitPtr<T>`,
		let mut pointer = other.pointer;
		//  Then set it to aim at the copied pointer.
		unsafe { pointer.set_pointer(ptr); }
		//  And set the new pointer/capacity.
		self.pointer = pointer;
		self.capacity = capacity;
	}
}

impl<C, T> Eq for VecBit<C, T>
where C: Cursor, T: BitStore {}

impl<C, T> Ord for VecBit<C, T>
where C: Cursor, T: BitStore {
	fn cmp(&self, rhs: &Self) -> Ordering {
		self.as_bitslice().cmp(rhs.as_bitslice())
	}
}

/** Tests if two `VecBit`s are semantically — not bitwise — equal.

It is valid to compare two vectors of different cursor or element types.

The equality condition requires that they have the same number of stored bits
and that each pair of bits in semantic order are identical.
**/
impl<A, B, C, D> PartialEq<VecBit<C, D>> for VecBit<A, B>
where A: Cursor, B: BitStore, C: Cursor, D: BitStore {
	/// Performs a comparison by `==`.
	///
	/// # Parameters
	///
	/// - `&self`
	/// - `rhs`: The other vector to compare.
	///
	/// # Returns
	///
	/// Whether the vectors compare equal.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let l: VecBit<LittleEndian, u16> = vecbit![LittleEndian, u16; 0, 1, 0, 1];
	/// let r: VecBit<BigEndian, u32> = vecbit![BigEndian, u32; 0, 1, 0, 1];
	/// assert!(l == r);
	/// ```
	///
	/// This example uses the same types to prove that raw, bitwise, values are
	/// not used for equality comparison.
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let l: VecBit<BigEndian, u8> = vecbit![BigEndian, u8; 0, 1, 0, 1];
	/// let r: VecBit<LittleEndian, u8> = vecbit![LittleEndian, u8; 0, 1, 0, 1];
	///
	/// assert_eq!(l, r);
	/// assert_ne!(l.as_slice(), r.as_slice());
	/// ```
	fn eq(&self, rhs: &VecBit<C, D>) -> bool {
		self.as_bitslice().eq(rhs.as_bitslice())
	}
}

impl<A, B, C, D> PartialEq<SliceBit<C, D>> for VecBit<A, B>
where A: Cursor, B: BitStore, C: Cursor, D: BitStore {
	fn eq(&self, rhs: &SliceBit<C, D>) -> bool {
		self.as_bitslice().eq(rhs)
	}
}

impl<A, B, C, D> PartialEq<&SliceBit<C, D>> for VecBit<A, B>
where A: Cursor, B: BitStore, C: Cursor, D: BitStore {
	fn eq(&self, rhs: &&SliceBit<C, D>) -> bool {
		self.as_bitslice().eq(*rhs)
	}
}

/** Compares two `VecBit`s by semantic — not bitwise — ordering.

The comparison sorts by testing each index for one vector to have a set bit
where the other vector has an unset bit. If the vectors are different, the
vector with the set bit sorts greater than the vector with the unset bit.

If one of the vectors is exhausted before they differ, the longer vector is
greater.
**/
impl<A, B, C, D> PartialOrd<VecBit<C, D>> for VecBit<A, B>
where A: Cursor, B: BitStore, C: Cursor, D: BitStore {
	/// Performs a comparison by `<` or `>`.
	///
	/// # Parameters
	///
	/// - `&self`
	/// - `rhs`: The other vector to compare.
	///
	/// # Returns
	///
	/// The relative ordering of the two vectors.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let a = vecbit![0, 1, 0, 0];
	/// let b = vecbit![0, 1, 0, 1];
	/// let c = vecbit![0, 1, 0, 1, 1];
	/// assert!(a < b);
	/// assert!(b < c);
	/// ```
	fn partial_cmp(&self, rhs: &VecBit<C, D>) -> Option<Ordering> {
		self.as_bitslice().partial_cmp(rhs.as_bitslice())
	}
}

impl<A, B, C, D> PartialOrd<SliceBit<C, D>> for VecBit<A, B>
where A: Cursor, B: BitStore, C: Cursor, D: BitStore {
	fn partial_cmp(&self, rhs: &SliceBit<C, D>) -> Option<Ordering> {
		self.as_bitslice().partial_cmp(rhs)
	}
}

impl<A, B, C, D> PartialOrd<&SliceBit<C, D>> for VecBit<A, B>
where A: Cursor, B: BitStore, C: Cursor, D: BitStore {
	fn partial_cmp(&self, rhs: &&SliceBit<C, D>) -> Option<Ordering> {
		self.as_bitslice().partial_cmp(*rhs)
	}
}

impl<C, T> AsMut<SliceBit<C, T>> for VecBit<C, T>
where C: Cursor, T: BitStore {
	fn as_mut(&mut self) -> &mut SliceBit<C, T> {
		self.as_mut_bitslice()
	}
}

/// Gives write access to all live elements in the underlying storage, including
/// the partially-filled tail.
impl<C, T> AsMut<[T]> for VecBit<C, T>
where C: Cursor, T: BitStore {
	fn as_mut(&mut self) -> &mut [T] {
		self.as_mut_slice()
	}
}

impl<C, T> AsRef<SliceBit<C, T>> for VecBit<C, T>
where C: Cursor, T: BitStore {
	fn as_ref(&self) -> &SliceBit<C, T> {
		self.as_bitslice()
	}
}

/// Gives read access to all live elements in the underlying storage, including
/// the partially-filled tail.
impl<C, T> AsRef<[T]> for VecBit<C, T>
where C: Cursor, T: BitStore {
	/// Accesses the underlying store.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let bv = vecbit![BigEndian, u8; 0, 0, 0, 0, 0, 0, 0, 0, 1];
	/// assert_eq!(&[0, 0b1000_0000], bv.as_slice());
	/// ```
	fn as_ref(&self) -> &[T] {
		self.as_slice()
	}
}

impl<C, T> From<&SliceBit<C, T>> for VecBit<C, T>
where C: Cursor, T: BitStore {
	fn from(src: &SliceBit<C, T>) -> Self {
		Self::from_bitslice(src)
	}
}

/** Builds a `VecBit` out of a slice of `bool`.

This is primarily for the `vecbit!` macro; it is not recommended for general
use.
**/
impl<C, T> From<&[bool]> for VecBit<C, T>
where C: Cursor, T: BitStore {
	fn from(src: &[bool]) -> Self {
		src.iter().cloned().collect()
	}
}

impl<C, T> From<BitBox<C, T>> for VecBit<C, T>
where C: Cursor, T: BitStore {
	fn from(src: BitBox<C, T>) -> Self {
		Self::from_boxed_bitslice(src)
	}
}

impl<C, T> From<&[T]> for VecBit<C, T>
where C: Cursor, T: BitStore {
	fn from(src: &[T]) -> Self {
		Self::from_slice(src)
	}
}

impl<C, T> From<Box<[T]>> for VecBit<C, T>
where C: Cursor, T: BitStore {
	fn from(src: Box<[T]>) -> Self {
		Self::from_boxed_bitslice(BitBox::from_boxed_slice(src))
	}
}

impl<C, T> Into<Box<[T]>> for VecBit<C, T>
where C: Cursor, T: BitStore {
	fn into(self) -> Box<[T]> {
		self.into_boxed_slice()
	}
}

/** Builds a `VecBit` out of a `Vec` of elements.

This moves the memory as-is from the source buffer into the new `VecBit`. The
source buffer will be unchanged by this operation, so you don't need to worry
about using the correct cursor type.
**/
impl<C, T> From<Vec<T>> for VecBit<C, T>
where C: Cursor, T: BitStore {
	fn from(src: Vec<T>) -> Self {
		Self::from_vec(src)
	}
}

impl<C, T> Into<Vec<T>> for VecBit<C, T>
where C: Cursor, T: BitStore {
	fn into(self) -> Vec<T> {
		self.into_vec()
	}
}

impl<C, T> Default for VecBit<C, T>
where C: Cursor, T: BitStore {
	fn default() -> Self {
		Self::new()
	}
}

/** Prints the `VecBit` for debugging.

The output is of the form `VecBit<C, T> [ELT, *]`, where `<C, T>` is the cursor
and element type, with square brackets on each end of the bits and all the live
elements in the vector printed in binary. The printout is always in semantic
order, and may not reflect the underlying store. To see the underlying store,
use `format!("{:?}", self.as_slice());` instead.

The alternate character `{:#?}` prints each element on its own line, rather than
separated by a space.
**/
impl<C, T> Debug for VecBit<C, T>
where C: Cursor, T: BitStore {
	/// Renders the `VecBit` type header and contents for debug.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let bv = vecbit![LittleEndian, u16;
	///   0, 1, 0, 1, 0, 0, 0, 0, 1, 1, 1, 1, 0, 1, 0, 1
	/// ];
	/// assert_eq!(
	///   "VecBit<LittleEndian, u16> [0101000011110101]",
	///   &format!("{:?}", bv)
	/// );
	/// ```
	fn fmt(&self, f: &mut Formatter) -> fmt::Result {
		f.write_str("VecBit<")?;
		f.write_str(C::TYPENAME)?;
		f.write_str(", ")?;
		f.write_str(T::TYPENAME)?;
		f.write_str("> ")?;
		Display::fmt(&**self, f)
	}
}

/** Prints the `VecBit` for displaying.

This prints each element in turn, formatted in binary in semantic order (so the
first bit seen is printed first and the last bit seen printed last). Each
element of storage is separated by a space for ease of reading.

The alternate character `{:#}` prints each element on its own line.

To see the in-memory representation, use `AsRef` to get access to the raw
elements and print that slice instead.
**/
impl<C, T> Display for VecBit<C, T>
where C: Cursor, T: BitStore {
	/// Renders the `VecBit` contents for display.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let bv = vecbit![BigEndian, u8; 0, 1, 0, 0, 1, 0, 1, 1, 0, 1];
	/// assert_eq!("[01001011, 01]", &format!("{}", bv));
	/// ```
	fn fmt(&self, f: &mut Formatter) -> fmt::Result {
		Display::fmt(&**self, f)
	}
}

/// Writes the contents of the `VecBit`, in semantic bit order, into a hasher.
impl<C, T> Hash for VecBit<C, T>
where C: Cursor, T: BitStore {
	/// Writes each bit of the `VecBit`, as a full `bool`, into the hasher.
	///
	/// # Parameters
	///
	/// - `&self`
	/// - `hasher`: The hashing pool into which the vector is written.
	fn hash<H: Hasher>(&self, hasher: &mut H) {
		<SliceBit<C, T> as Hash>::hash(self, hasher)
	}
}

#[cfg(feature = "std")]
impl<C, T> Write for VecBit<C, T>
where C: Cursor, T: BitStore {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		use std::cmp;
		let amt = cmp::min(buf.len(), BitPtr::<T>::MAX_BITS - self.len());
		self.extend(<&SliceBit<C, u8>>::from(buf));
		Ok(amt)
	}

	fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

/** Extends a `VecBit` with the contents of another bitstream.

At present, this just calls `.push()` in a loop. When specialization becomes
available, it will be able to more intelligently perform bulk moves from the
source into `self` when the source is `SliceBit`-compatible.
**/
impl<C, T> Extend<bool> for VecBit<C, T>
where C: Cursor, T: BitStore {
	/// Extends a `VecBit` from another bitstream.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `src`: A source bitstream.
	///
	/// # Type Parameters
	///
	/// - `I: IntoIterator<Item=bool>`: The source bitstream with which to
	///   extend `self`.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut bv = vecbit![BigEndian, u8; 0; 4];
	/// bv.extend(vecbit![1; 4]);
	/// assert_eq!(0x0F, bv.as_slice()[0]);
	/// ```
	fn extend<I: IntoIterator<Item=bool>>(&mut self, src: I) {
		let iter = src.into_iter();
		match iter.size_hint() {
			(_, Some(hi)) => self.reserve(hi),
			(lo, None) => self.reserve(lo),
		}
		iter.for_each(|b| self.push(b));
	}
}

/// Permits the construction of a `VecBit` by using `.collect()` on an iterator
/// of `bool`.
impl<C, T> FromIterator<bool> for VecBit<C, T>
where C: Cursor, T: BitStore {
	/// Collects an iterator of `bool` into a vector.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// use std::iter::repeat;
	/// let bv: VecBit<BigEndian, u8> = repeat(true)
	///   .take(4)
	///   .chain(repeat(false).take(4))
	///   .collect();
	/// assert_eq!(bv.as_slice()[0], 0xF0);
	/// ```
	fn from_iter<I: IntoIterator<Item=bool>>(src: I) -> Self {
		let iter = src.into_iter();
		let mut bv = match iter.size_hint() {
			| (_, Some(len))
			| (len, _)
			=> Self::with_capacity(len),
		};
		for bit in iter {
			bv.push(bit);
		}
		bv
	}
}

/** Produces an iterator over all the bits in the vector.

This iterator follows the ordering in the vector type, and implements
`ExactSizeIterator`, since `VecBit`s always know exactly how large they are, and
`DoubleEndedIterator`, since they have known ends.
**/
impl<C, T> IntoIterator for VecBit<C, T>
where C: Cursor, T: BitStore {
	type Item = bool;
	type IntoIter = IntoIter<C, T>;

	/// Iterates over the vector.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let bv = vecbit![BigEndian, u8; 1, 1, 1, 1, 0, 0, 0, 0];
	/// let mut count = 0;
	/// for bit in bv {
	///   if bit { count += 1; }
	/// }
	/// assert_eq!(count, 4);
	/// ```
	fn into_iter(self) -> Self::IntoIter {
		IntoIter {
			region: self.pointer,
			vecbit: self,
		}
	}
}

impl<'a, C, T> IntoIterator for &'a VecBit<C, T>
where C: Cursor, T: 'a + BitStore {
	type Item = bool;
	type IntoIter = <&'a SliceBit<C, T> as IntoIterator>::IntoIter;

	fn into_iter(self) -> Self::IntoIter {
		<&'a SliceBit<C, T> as IntoIterator>::into_iter(self)
	}
}

/// `VecBit` is safe to move across thread boundaries, as is `&mut VecBit`.
unsafe impl<C, T> Send for VecBit<C, T>
where C: Cursor, T: BitStore {}

/// `&VecBit` is safe to move across thread boundaries.
unsafe impl<C, T> Sync for VecBit<C, T>
where C: Cursor, T: BitStore {}

/** Adds two `VecBit`s together, zero-extending the shorter.

`VecBit` addition works just like adding numbers longhand on paper. The first
bits in the `VecBit` are the highest, so addition works from right to left, and
the shorter `VecBit` is assumed to be extended to the left with zero.

The output `VecBit` may be one bit longer than the longer input, if addition
overflowed.

Numeric arithmetic is provided on `VecBit` as a convenience. Serious numeric
computation on variable-length integers should use the `num_bigint` crate
instead, which is written specifically for that use case. `VecBit`s are not
intended for arithmetic, and `vecbit` makes no guarantees about sustained
correctness in arithmetic at this time.
**/
impl<C, T> Add for VecBit<C, T>
where C: Cursor, T: BitStore {
	type Output = Self;

	/// Adds two `VecBit`s.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let a = vecbit![0, 1, 0, 1];
	/// let b = vecbit![0, 0, 1, 1];
	/// let s = a + b;
	/// assert_eq!(vecbit![1, 0, 0, 0], s);
	/// ```
	///
	/// This example demonstrates the addition of differently-sized `VecBit`s,
	/// and will overflow.
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let a = vecbit![1; 4];
	/// let b = vecbit![1; 1];
	/// let s = b + a;
	/// assert_eq!(vecbit![1, 0, 0, 0, 0], s);
	/// ```
	fn add(mut self, addend: Self) -> Self::Output {
		self += addend;
		self
	}
}

/** Adds another `VecBit` into `self`, zero-extending the shorter.

`VecBit` addition works just like adding numbers longhand on paper. The first
bits in the `VecBit` are the highest, so addition works from right to left, and
the shorter `VecBit` is assumed to be extended to the left with zero.

The output `VecBit` may be one bit longer than the longer input, if addition
overflowed.

Numeric arithmetic is provided on `VecBit` as a convenience. Serious numeric
computation on variable-length integers should use the `num_bigint` crate
instead, which is written specifically for that use case. `VecBit`s are not
intended for arithmetic, and `vecbit` makes no guarantees about sustained
correctness in arithmetic at this time.
**/
impl<C, T> AddAssign for VecBit<C, T>
where C: Cursor, T: BitStore {
	/// Adds another `VecBit` into `self`.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut a = vecbit![1, 0, 0, 1];
	/// let b = vecbit![0, 1, 1, 1];
	/// a += b;
	/// assert_eq!(a, vecbit![1, 0, 0, 0, 0]);
	/// ```
	fn add_assign(&mut self, mut addend: Self) {
		use core::iter::repeat;
		//  If the other vec is longer, swap them before continuing.
		if addend.len() > self.len() {
			mem::swap(self, &mut addend);
		}
		//  Now that self.len() >= addend.len(), proceed with addition.
		let mut c = false;
		let mut stack = VecBit::<C, T>::with_capacity(self.len());
		let addend = addend.into_iter().rev().chain(repeat(false));
		for (a, b) in self.iter().rev().zip(addend) {
			let (y, z) = crate::rca1(a, b, c);
			stack.push(y);
			c = z;
		}
		//  If the carry made it to the end, push it.
		if c {
			stack.push(true);
		}
		//  Unwind the stack into `self`.
		self.clear();
		self.extend(stack.into_iter().rev());
	}
}

/** Performs the Boolean `AND` operation between each element of a `VecBit` and
anything that can provide a stream of `bool` values (such as another `VecBit`,
or any `bool` generator of your choice). The `VecBit` emitted will have the
length of the shorter sequence of bits -- if one is longer than the other, the
extra bits will be ignored.
**/
impl<C, T, I> BitAnd<I> for VecBit<C, T>
where C: Cursor, T: BitStore, I: IntoIterator<Item=bool> {
	type Output = Self;

	/// `AND`s a vector and a bitstream, producing a new vector.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let lhs = vecbit![BigEndian, u8; 0, 1, 0, 1];
	/// let rhs = vecbit![BigEndian, u8; 0, 0, 1, 1];
	/// let and = lhs & rhs;
	/// assert_eq!("[0001]", &format!("{}", and));
	/// ```
	fn bitand(mut self, rhs: I) -> Self::Output {
		self &= rhs;
		self
	}
}

/** Performs the Boolean `AND` operation in place on a `VecBit`, using a stream
of `bool` values as the other bit for each operation. If the other stream is
shorter than `self`, `self` will be truncated when the other stream expires.
**/
impl<C, T, I> BitAndAssign<I> for VecBit<C, T>
where C: Cursor, T: BitStore, I: IntoIterator<Item=bool> {
	/// `AND`s another bitstream into a vector.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut src  = vecbit![BigEndian, u8; 0, 1, 0, 1];
	///         src &= vecbit![BigEndian, u8; 0, 0, 1, 1];
	/// assert_eq!("[0001]", &format!("{}", src));
	/// ```
	fn bitand_assign(&mut self, rhs: I) {
		// let mut len = 0;
		// for (idx, other) in (0 .. self.len()).zip(rhs.into_iter()) {
		// 	let val = self[idx] & other;
		// 	self.set(idx, val);
		// 	len += 1;
		// }
		let len = rhs.into_iter()
			.take(self.len())
			.enumerate()
			.flat_map(|(i, r)| self.get(i).map(|l| self.set(i, l & r)))
			.count();
		self.truncate(len);
	}
}

/** Performs the Boolean `OR` operation between each element of a `VecBit` and
anything that can provide a stream of `bool` values (such as another `VecBit`,
or any `bool` generator of your choice). The `VecBit` emitted will have the
length of the shorter sequence of bits -- if one is longer than the other, the
extra bits will be ignored.
**/
impl<C, T, I> BitOr<I> for VecBit<C, T>
where C: Cursor, T: BitStore, I: IntoIterator<Item=bool> {
	type Output = Self;

	/// `OR`s a vector and a bitstream, producing a new vector.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let lhs = vecbit![0, 1, 0, 1];
	/// let rhs = vecbit![0, 0, 1, 1];
	/// let or  = lhs | rhs;
	/// assert_eq!("[0111]", &format!("{}", or));
	/// ```
	fn bitor(mut self, rhs: I) -> Self::Output {
		self |= rhs;
		self
	}
}

/** Performs the Boolean `OR` operation in place on a `VecBit`, using a stream
of `bool` values as the other bit for each operation. If the other stream is
shorter than `self`, `self` will be truncated when the other stream expires.
**/
impl<C, T, I> BitOrAssign<I> for VecBit<C, T>
where C: Cursor, T: BitStore, I: IntoIterator<Item=bool> {
	/// `OR`s another bitstream into a vector.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut src  = vecbit![0, 1, 0, 1];
	///         src |= vecbit![0, 0, 1, 1];
	/// assert_eq!("[0111]", &format!("{}", src));
	/// ```
	fn bitor_assign(&mut self, rhs: I) {
		// let mut len = 0;
		// for (idx, other) in (0 .. self.len()).zip(rhs.into_iter()) {
		// 	let val = self[idx] | other;
		// 	self.set(idx, val);
		// 	len += 1;
		// }
		let len = rhs.into_iter()
			.take(self.len())
			.enumerate()
			.flat_map(|(i, r)| self.get(i).map(|l| self.set(i, l | r)))
			.count();
		self.truncate(len);
	}
}

/** Performs the Boolean `XOR` operation between each element of a `VecBit` and
anything that can provide a stream of `bool` values (such as another `VecBit`,
or any `bool` generator of your choice). The `VecBit` emitted will have the
length of the shorter sequence of bits -- if one is longer than the other, the
extra bits will be ignored.
**/
impl<C, T, I> BitXor<I> for VecBit<C, T>
where C: Cursor, T: BitStore, I: IntoIterator<Item=bool> {
	type Output = Self;

	/// `XOR`s a vector and a bitstream, producing a new vector.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let lhs = vecbit![0, 1, 0, 1];
	/// let rhs = vecbit![0, 0, 1, 1];
	/// let xor = lhs ^ rhs;
	/// assert_eq!("[0110]", &format!("{}", xor));
	/// ```
	fn bitxor(mut self, rhs: I) -> Self::Output {
		self ^= rhs;
		self
	}
}

/** Performs the Boolean `XOR` operation in place on a `VecBit`, using a stream
of `bool` values as the other bit for each operation. If the other stream is
shorter than `self`, `self` will be truncated when the other stream expires.
**/
impl<C, T, I> BitXorAssign<I> for VecBit<C, T>
where C: Cursor, T: BitStore, I: IntoIterator<Item=bool> {
	/// `XOR`s another bitstream into a vector.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut src  = vecbit![0, 1, 0, 1];
	///         src ^= vecbit![0, 0, 1, 1];
	/// assert_eq!("[0110]", &format!("{}", src));
	/// ```
	fn bitxor_assign(&mut self, rhs: I) {
		// let mut len = 0;
		// for (idx, other) in (0 .. self.len()).zip(rhs.into_iter()) {
		// 	let val = self[idx] ^ other;
		// 	self.set(idx, val);
		// 	len += 1;
		// }
		let len = rhs.into_iter()
			.take(self.len())
			.enumerate()
			.flat_map(|(i, r)| self.get(i).map(|l| self.set(i, l ^ r)))
			.count();
		self.truncate(len);
	}
}

/** Reborrows the `VecBit` as a `SliceBit`.

This mimics the separation between `Vec<T>` and `[T]`.
**/
impl<C, T> Deref for VecBit<C, T>
where C: Cursor, T: BitStore {
	type Target = SliceBit<C, T>;

	/// Dereferences `&VecBit` down to `&SliceBit`.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let bv: VecBit = vecbit![1; 4];
	/// let bref: &SliceBit = &bv;
	/// assert!(bref[2]);
	/// ```
	fn deref(&self) -> &Self::Target {
		self.as_bitslice()
	}
}

/** Mutably reborrows the `VecBit` as a `SliceBit`.

This mimics the separation between `Vec<T>` and `[T]`.
**/
impl<C, T> DerefMut for VecBit<C, T>
where C: Cursor, T: BitStore {
	/// Dereferences `&mut VecBit` down to `&mut SliceBit`.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut bv: VecBit = vecbit![0; 6];
	/// let bref: &mut SliceBit = &mut bv;
	/// assert!(!bref[5]);
	/// bref.set(5, true);
	/// assert!(bref[5]);
	/// ```
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.as_mut_bitslice()
	}
}

/// Readies the underlying storage for Drop.
impl<C, T> Drop for VecBit<C, T>
where C: Cursor, T: BitStore {
	/// Rebuild the interior `Vec` and let it run the deallocator.
	fn drop(&mut self) {
		let bp = mem::replace(&mut self.pointer, BitPtr::empty());
		//  Build a Vec<T> out of the elements, and run its destructor.
		let (ptr, cap) = (bp.pointer(), self.capacity);
		drop(unsafe { Vec::from_raw_parts(ptr.w(), 0, cap) });
	}
}

/// Gets the bit at a specific index. The index must be less than the length of
/// the `VecBit`.
impl<C, T> Index<usize> for VecBit<C, T>
where C: Cursor, T: BitStore {
	type Output = bool;

	/// Looks up a single bit by semantic count.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let bv = vecbit![BigEndian, u8; 0, 0, 0, 0, 0, 0, 0, 0, 1, 0];
	/// assert!(!bv[7]); // ---------------------------------^  |  |
	/// assert!( bv[8]); // ------------------------------------^  |
	/// assert!(!bv[9]); // ---------------------------------------^
	/// ```
	///
	/// If the index is greater than or equal to the length, indexing will
	/// panic.
	///
	/// The below test will panic when accessing index 1, as only index 0 is
	/// valid.
	///
	/// ```rust,should_panic
	/// use vecbit::prelude::*;
	///
	/// let mut bv: VecBit = VecBit::new();
	/// bv.push(true);
	/// bv[1];
	/// ```
	fn index(&self, cursor: usize) -> &Self::Output {
		&self.as_bitslice()[cursor]
	}
}

impl<C, T> Index<Range<usize>> for VecBit<C, T>
where C: Cursor, T: BitStore {
	type Output = SliceBit<C, T>;

	fn index(&self, range: Range<usize>) -> &Self::Output {
		&self.as_bitslice()[range]
	}
}

impl<C, T> IndexMut<Range<usize>> for VecBit<C, T>
where C: Cursor, T: BitStore {
	fn index_mut(&mut self, range: Range<usize>) -> &mut Self::Output {
		&mut self.as_mut_bitslice()[range]
	}
}

impl<C, T> Index<RangeFrom<usize>> for VecBit<C, T>
where C: Cursor, T: BitStore {
	type Output = SliceBit<C, T>;

	fn index(&self, range: RangeFrom<usize>) -> &Self::Output {
		&self.as_bitslice()[range]
	}
}

impl<C, T> IndexMut<RangeFrom<usize>> for VecBit<C, T>
where C: Cursor, T: BitStore {
	fn index_mut(&mut self, range: RangeFrom<usize>) -> &mut Self::Output {
		&mut self.as_mut_bitslice()[range]
	}
}

impl<C, T> Index<RangeFull> for VecBit<C, T>
where C: Cursor, T: BitStore {
	type Output = SliceBit<C, T>;

	fn index(&self, _: RangeFull) -> &Self::Output {
		self.as_bitslice()
	}
}

impl<C, T> IndexMut<RangeFull> for VecBit<C, T>
where C: Cursor, T: BitStore {
	fn index_mut(&mut self, _: RangeFull) -> &mut Self::Output {
		self.as_mut_bitslice()
	}
}

impl<C, T> Index<RangeInclusive<usize>> for VecBit<C, T>
where C: Cursor, T: BitStore {
	type Output = SliceBit<C, T>;

	fn index(&self, range: RangeInclusive<usize>) -> &Self::Output {
		&self.as_bitslice()[range]
	}
}

impl<C, T> IndexMut<RangeInclusive<usize>> for VecBit<C, T>
where C: Cursor, T: BitStore {
	fn index_mut(&mut self, range: RangeInclusive<usize>) -> &mut Self::Output {
		&mut self.as_mut_bitslice()[range]
	}
}

impl<C, T> Index<RangeTo<usize>> for VecBit<C, T>
where C: Cursor, T: BitStore {
	type Output = SliceBit<C, T>;

	fn index(&self, range: RangeTo<usize>) -> &Self::Output {
		&self.as_bitslice()[range]
	}
}

impl<C, T> IndexMut<RangeTo<usize>> for VecBit<C, T>
where C: Cursor, T: BitStore {
	fn index_mut(&mut self, range: RangeTo<usize>) -> &mut Self::Output {
		&mut self.as_mut_bitslice()[range]
	}
}

impl<C, T> Index<RangeToInclusive<usize>> for VecBit<C, T>
where C: Cursor, T: BitStore {
	type Output = SliceBit<C, T>;

	fn index(&self, range: RangeToInclusive<usize>) -> &Self::Output {
		&self.as_bitslice()[range]
	}
}

impl<C, T> IndexMut<RangeToInclusive<usize>> for VecBit<C, T>
where C: Cursor, T: BitStore {
	fn index_mut(&mut self, range: RangeToInclusive<usize>) -> &mut Self::Output {
		&mut self.as_mut_bitslice()[range]
	}
}

/** 2’s-complement negation of a `VecBit`.

In 2’s-complement, negation is defined as bit-inversion followed by adding one.

Numeric arithmetic is provided on `VecBit` as a convenience. Serious numeric
computation on variable-length integers should use the `num_bigint` crate
instead, which is written specifically for that use case. `VecBit`s are not
intended for arithmetic, and `vecbit` makes no guarantees about sustained
correctness in arithmetic at this time.
**/
impl<C, T> Neg for VecBit<C, T>
where C: Cursor, T: BitStore {
	type Output = Self;

	/// Numerically negates a `VecBit` using 2’s-complement arithmetic.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let bv = vecbit![0, 1, 1];
	/// let ne = -bv;
	/// assert_eq!(ne, vecbit![1, 0, 1]);
	/// ```
	fn neg(mut self) -> Self::Output {
		//  An empty vector does nothing.
		//  Negative zero is zero. Without this check, -[0+] becomes[10+1].
		if self.is_empty() || self.not_any() {
			return self;
		}
		self = !self;
		self += VecBit::<C, T>::from_iter(iter::once(true));
		self
	}
}

/// Flips all bits in the vector.
impl<C, T> Not for VecBit<C, T>
where C: Cursor, T: BitStore {
	type Output = Self;

	/// Inverts all bits in the vector.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let bv: VecBit<BigEndian, u32> = VecBit::from(&[0u32] as &[u32]);
	/// let flip = !bv;
	/// assert_eq!(!0u32, flip.as_slice()[0]);
	/// ```
	fn not(mut self) -> Self::Output {
		let _ = self.as_mut_bitslice().not();
		self
	}
}

__vecbit_shift!(u8, u16, u32, u64, i8, i16, i32, i64);

/** Shifts all bits in the vector to the left – **DOWN AND TOWARDS THE FRONT**.

On fundamentals, the left-shift operator `<<` moves bits away from origin and
towards the ceiling. This is because we label the bits in a primitive with the
minimum on the right and the maximum on the left, which is big-endian bit order.
This increases the value of the primitive being shifted.

**THAT IS NOT HOW `BITVEC` WORKS!**

`VecBit` defines its layout with the minimum on the left and the maximum on the
right! Thus, left-shifting moves bits towards the **minimum**.

In BigEndian order, the effect in memory will be what you expect the `<<`
operator to do.

**In LittleEndian order, the effect will be equivalent to using `>>` on the**
**fundamentals in memory!**

# Notes

In order to preserve the effects in memory that this operator traditionally
expects, the bits that are emptied by this operation are zeroed rather than
left to their old value.

The length of the vector is decreased by the shift amount.

If the shift amount is greater than the length, the vector calls `clear()` and
zeroes its memory. This is *not* an error.
**/
impl<C, T> Shl<usize> for VecBit<C, T>
where C: Cursor, T: BitStore {
	type Output = Self;

	/// Shifts a `VecBit` to the left, shortening it.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let bv = vecbit![BigEndian, u8; 0, 0, 0, 1, 1, 1];
	/// assert_eq!("[000111]", &format!("{}", bv));
	/// assert_eq!(0b0001_1100, bv.as_slice()[0]);
	/// assert_eq!(bv.len(), 6);
	/// let ls = bv << 2usize;
	/// assert_eq!("[0111]", &format!("{}", ls));
	/// assert_eq!(0b0111_0000, ls.as_slice()[0]);
	/// assert_eq!(ls.len(), 4);
	/// ```
	fn shl(mut self, shamt: usize) -> Self::Output {
		self <<= shamt;
		self
	}
}

/** Shifts all bits in the vector to the left – **DOWN AND TOWARDS THE FRONT**.

On fundamentals, the left-shift operator `<<` moves bits away from origin and
towards the ceiling. This is because we label the bits in a primitive with the
minimum on the right and the maximum on the left, which is big-endian bit order.
This increases the value of the primitive being shifted.

**THAT IS NOT HOW `BITVEC` WORKS!**

`VecBit` defines its layout with the minimum on the left and the maximum on the
right! Thus, left-shifting moves bits towards the **minimum**.

In BigEndian order, the effect in memory will be what you expect the `<<`
operator to do.

**In LittleEndian order, the effect will be equivalent to using `>>` on the**
**fundamentals in memory!**

# Notes

In order to preserve the effects in memory that this operator traditionally
expects, the bits that are emptied by this operation are zeroed rather than left
to their old value.

The length of the vector is decreased by the shift amount.

If the shift amount is greater than the length, the vector calls `clear()` and
zeroes its memory. This is *not* an error.
**/
impl<C, T> ShlAssign<usize> for VecBit<C, T>
where C: Cursor, T: BitStore {
	/// Shifts a `VecBit` to the left in place, shortening it.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut bv = vecbit![LittleEndian, u8; 0, 0, 0, 1, 1, 1];
	/// assert_eq!("[000111]", &format!("{}", bv));
	/// assert_eq!(0b0011_1000, bv.as_slice()[0]);
	/// assert_eq!(bv.len(), 6);
	/// bv <<= 2;
	/// assert_eq!("[0111]", &format!("{}", bv));
	/// assert_eq!(0b0000_1110, bv.as_slice()[0]);
	/// assert_eq!(bv.len(), 4);
	/// ```
	fn shl_assign(&mut self, shamt: usize) {
		let len = self.len();
		if shamt >= len {
			self.set_all(false);
			self.clear();
			return;
		}
		for idx in shamt .. len {
			let val = self[idx];
			self.set(idx.saturating_sub(shamt), val);
		}
		let trunc = len.saturating_sub(shamt);
		for idx in trunc .. len {
			self.set(idx, false);
		}
		self.truncate(trunc);
	}
}

/** Shifts all bits in the vector to the right – **UP AND TOWARDS THE BACK**.

On fundamentals, the right-shift operator `>>` moves bits towards the origin and
away from the ceiling. This is because we label the bits in a primitive with the
minimum on the right and the maximum on the left, which is big-endian bit order.
This decreases the value of the primitive being shifted.

**THAT IS NOT HOW `BITVEC` WORKS!**

`VecBit` defines its layout with the minimum on the left and the maximum on the
right! Thus, right-shifting moves bits towards the **maximum**.

In BigEndian order, the effect in memory will be what you expect the `>>`
operator to do.

**In LittleEndian order, the effect will be equivalent to using `<<` on the**
**fundamentals in memory!**

# Notes

In order to preserve the effects in memory that this operator traditionally
expects, the bits that are emptied by this operation are zeroed rather than left
to their old value.

The length of the vector is increased by the shift amount.

If the new length of the vector would overflow, a panic occurs. This *is* an
error.
**/
impl<C, T> Shr<usize> for VecBit<C, T>
where C: Cursor, T: BitStore {
	type Output = Self;

	/// Shifts a `VecBit` to the right, lengthening it and filling the front
	/// with 0.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let bv = vecbit![BigEndian, u8; 0, 0, 0, 1, 1, 1];
	/// assert_eq!("[000111]", &format!("{}", bv));
	/// assert_eq!(0b0001_1100, bv.as_slice()[0]);
	/// assert_eq!(bv.len(), 6);
	/// let rs = bv >> 2usize;
	/// assert_eq!("[00000111]", &format!("{}", rs));
	/// assert_eq!(0b0000_0111, rs.as_slice()[0]);
	/// assert_eq!(rs.len(), 8);
	/// ```
	fn shr(mut self, shamt: usize) -> Self::Output {
		self >>= shamt;
		self
	}
}

/** Shifts all bits in the vector to the right – **UP AND TOWARDS THE BACK**.

On fundamentals, the right-shift operator `>>` moves bits towards the origin and
away from the ceiling. This is because we label the bits in a primitive with the
minimum on the right and the maximum on the left, which is big-endian bit order.
This decreases the value of the primitive being shifted.

**THAT IS NOT HOW `BITVEC` WORKS!**

`VecBit` defines its layout with the minimum on the left and the maximum on the
right! Thus, right-shifting moves bits towards the **maximum**.

In BigEndian order, the effect in memory will be what you expect the `>>`
operator to do.

**In LittleEndian order, the effect will be equivalent to using `<<` on the**
**fundamentals in memory!**

# Notes

In order to preserve the effects in memory that this operator traditionally
expects, the bits that are emptied by this operation are zeroed rather than left
to their old value.

The length of the vector is increased by the shift amount.

If the new length of the vector would overflow, a panic occurs. This *is* an
error.
**/
impl<C, T> ShrAssign<usize> for VecBit<C, T>
where C: Cursor, T: BitStore {
	/// Shifts a `VecBit` to the right in place, lengthening it and filling the
	/// front with 0.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let mut bv = vecbit![LittleEndian, u8; 0, 0, 0, 1, 1, 1];
	/// assert_eq!("[000111]", &format!("{}", bv));
	/// assert_eq!(0b0011_1000, bv.as_slice()[0]);
	/// assert_eq!(bv.len(), 6);
	/// bv >>= 2;
	/// assert_eq!("[00000111]", &format!("{}", bv));
	/// assert_eq!(0b1110_0000, bv.as_slice()[0]);
	/// assert_eq!(bv.len(), 8);
	/// ```
	fn shr_assign(&mut self, shamt: usize) {
		let old_len = self.len();
		for _ in 0 .. shamt {
			self.push(false);
		}
		for idx in (0 .. old_len).rev() {
			let val = self[idx];
			self.set(idx.saturating_add(shamt), val);
		}
		for idx in 0 .. shamt {
			self.set(idx, false);
		}
	}
}

/** Subtracts one `VecBit` from another assuming 2’s-complement encoding.

Subtraction is a more complex operation than addition. The bit-level work is
largely the same, but semantic distinctions must be made. Unlike addition, which
is commutative and tolerant of switching the order of the addends, subtraction
cannot swap the minuend (LHS) and subtrahend (RHS).

Because of the properties of 2’s-complement arithmetic, M - S is equivalent to M
+ (!S + 1). Subtraction therefore bitflips the subtrahend and adds one. This
may, in a degenerate case, cause the subtrahend to increase in length.

Once the subtrahend is stable, the minuend zero-extends its left side in order
to match the length of the subtrahend if needed (this is provided by the `>>`
operator).

When the minuend is stable, the minuend and subtrahend are added together by the
`<VecBit as Add>` implementation. The output will be encoded in 2’s-complement,
so a leading one means that the output is considered negative.

Interpreting the contents of a `VecBit` as an integer is beyond the scope of
this crate.

Numeric arithmetic is provided on `VecBit` as a convenience. Serious numeric
computation on variable-length integers should use the `num_bigint` crate
instead, which is written specifically for that use case. `VecBit`s are not
intended for arithmetic, and `vecbit` makes no guarantees about sustained
correctness in arithmetic at this time.
**/
impl<C, T> Sub for VecBit<C, T>
where C: Cursor, T: BitStore {
	type Output = Self;

	/// Subtracts one `VecBit` from another.
	///
	/// # Examples
	///
	/// Minuend larger than subtrahend, positive difference.
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let a = vecbit![1, 0];
	/// let b = vecbit![   1];
	/// let c = a - b;
	/// assert_eq!(vecbit![0, 1], c);
	/// ```
	///
	/// Minuend smaller than subtrahend, negative difference.
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let a = vecbit![   1];
	/// let b = vecbit![1, 0];
	/// let c = a - b;
	/// assert_eq!(vecbit![1, 1], c);
	/// ```
	///
	/// Subtraction from self is correctly handled.
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let a = vecbit![0, 1, 1, 0];
	/// let b = a.clone();
	/// let c = a - b;
	/// assert!(c.not_any(), "{:?}", c);
	/// ```
	fn sub(mut self, subtrahend: Self) -> Self::Output {
		self -= subtrahend;
		self
	}
}

/** Subtracts another `VecBit` from `self`, assuming 2’s-complement encoding.

The minuend is zero-extended, or the subtrahend sign-extended, as needed to
ensure that the vectors are the same width before subtraction occurs.

The `Sub` trait has more documentation on the subtraction process.

Numeric arithmetic is provided on `VecBit` as a convenience. Serious numeric
computation on variable-length integers should use the `num_bigint` crate
instead, which is written specifically for that use case. `VecBit`s are not
intended for arithmetic, and `vecbit` makes no guarantees about sustained
correctness in arithmetic at this time.
**/
impl<C, T> SubAssign for VecBit<C, T>
where C: Cursor, T: BitStore {
	/// Subtracts another `VecBit` from `self`.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let a = vecbit![0, 0, 0, 1];
	/// let b = vecbit![0, 0, 0, 0];
	/// let c = a - b;
	/// assert_eq!(c, vecbit![0, 0, 0, 1]);
	/// ```
	//  Note: in `a - b`, `a` is `self` and the minuend, `b` is the subtrahend
	fn sub_assign(&mut self, mut subtrahend: Self) {
		//  Test for a zero subtrahend. Subtraction of zero is the identity
		//  function, and can exit immediately.
		if subtrahend.not_any() {
			return;
		}
		//  Invert the subtrahend in preparation for addition
		subtrahend = -subtrahend;
		let (llen, rlen) = (self.len(), subtrahend.len());
		//  If the subtrahend is longer than the minuend, 0-extend the minuend.
		if rlen > llen {
			let diff = rlen - llen;
			*self >>= diff;
		}
		else {
			//  If the minuend is longer than the subtrahend, sign-extend the
			//  subtrahend.
			if llen > rlen {
				let diff = llen - rlen;
				let sign = subtrahend[0];
				subtrahend >>= diff;
				subtrahend[.. diff].set_all(sign);
			}
		}
		let old = self.len();
		*self += subtrahend;
		//  If the subtraction emitted a carry, remove it.
		if self.len() > old {
			*self <<= 1;
		}
	}
}

/** State keeper for draining iteration.

# Type Parameters

- `C: Cursor`: The cursor type of the underlying vector.
- `T: 'a + BitStore`: The storage type of the underlying vector.

# Lifetimes

- `'a`: The lifetime of the underlying vector.
**/
pub struct Drain<'a, C, T>
where C: Cursor, T: 'a + BitStore {
	/// Pointer to the `VecBit` being drained.
	vecbit: NonNull<VecBit<C, T>>,
	/// Current remaining range to remove.
	iter: crate::slice::Iter<'a, C, T>,
	/// Index of the original vector tail to preserve.
	tail_start: usize,
	/// Length of the tail.
	tail_len: usize,
}

impl<'a, C, T> Drain<'a, C, T>
where C: Cursor, T: 'a + BitStore {
	/// Fills the drain span with another iterator.
	///
	/// If the stream exhausts before the drain is filled, then the tail
	/// elements move downwards; otherwise, the tail stays put and the drain is
	/// filled.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `stream`: The source of bits to fill into the drain.
	///
	/// # Returns
	///
	/// - `true` if the drain was filled before the `stream` exhausted.
	/// - `false` if the `stream` exhausted early, and the tail was moved down.
	///
	/// # Type Parameters
	///
	/// - `I: Iterator<Item=bool>`: A provider of bits.
	unsafe fn fill<I: Iterator<Item=bool>>(&mut self, stream: &mut I) -> bool {
		let bv = self.vecbit.as_mut();
		let drain_from = bv.len();
		let drain_upto = self.tail_start;

		for n in drain_from .. drain_upto {
			if let Some(bit) = stream.next() {
				bv.push(bit);
			}
			else {
				for (to, from) in (n .. n + self.tail_len).zip(drain_upto ..) {
					bv.swap(from, to);
				}
				self.tail_start = n;
				return false;
			}
		}
		true
	}

	/// Moves the tail span farther back in the vector.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `by`: The amount by which to move the tail span.
	unsafe fn move_tail(&mut self, by: usize) {
		let bv = self.vecbit.as_mut();
		bv.reserve(by);
		let new_tail = self.tail_start + by;
		let old_len = bv.len();
		let new_len = self.tail_start + self.tail_len + by;

		bv.set_len(new_len);
		for n in (0 .. self.tail_len).rev() {
			bv.swap(self.tail_start + n, new_tail + n);
		}
		bv.set_len(old_len);

		self.tail_start = new_tail;
	}
}

impl<'a, C, T> DoubleEndedIterator for Drain<'a, C, T>
where C: Cursor, T: 'a + BitStore {
	fn next_back(&mut self) -> Option<Self::Item> {
		self.iter.next_back()
	}
}

impl<'a, C, T> ExactSizeIterator for Drain<'a, C, T>
where C: Cursor, T: 'a + BitStore {}

impl<'a, C, T> FusedIterator for Drain<'a, C, T>
where C: Cursor, T: 'a + BitStore {}

impl<'a, C, T> Iterator for Drain<'a, C, T>
where C: Cursor, T: 'a + BitStore {
	type Item = bool;

	fn next(&mut self) -> Option<Self::Item> {
		self.iter.next()
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		self.iter.size_hint()
	}

	fn count(self) -> usize {
		self.len()
	}

	fn nth(&mut self, n: usize) -> Option<Self::Item> {
		self.iter.nth(n)
	}

	fn last(mut self) -> Option<Self::Item> {
		self.iter.next_back()
	}
}

impl<'a, C, T> Drop for Drain<'a, C, T>
where C: Cursor, T: 'a + BitStore {
	fn drop(&mut self) { unsafe {
		let bv: &mut VecBit<C, T> = self.vecbit.as_mut();
		//  Get the start of the drained span.
		let start = bv.len();
		//  Get the start of the remnant span.
		let tail = self.tail_start;
		let tail_len = self.tail_len;
		//  Get the full length of the vector,
		let full_len = tail + tail_len;
		//  And the length of the vector after the drain.
		let end_len = start + tail_len;
		//  Inflate the vector to include the remnant span,
		bv.set_len(full_len);
		//  Swap the remnant span down into the drained span,
		for (from, to) in (tail .. full_len).zip(start .. end_len) {
			bv.swap(from, to);
		}
		//  And deflate the vector to fit.
		bv.set_len(end_len);
	} }
}

/// A consuming iterator for `VecBit`.
#[repr(C)]
pub struct IntoIter<C, T>
where C: Cursor, T: BitStore {
	/// Owning descriptor for the allocation. This is not modified by iteration.
	vecbit: VecBit<C, T>,
	/// Descriptor for the live portion of the vector as iteration proceeds.
	region: BitPtr<T>,
}

impl<C, T> IntoIter<C, T>
where C: Cursor, T: BitStore {
	fn iterator(&self) -> <&SliceBit<C, T> as IntoIterator>::IntoIter {
		self.region.into_bitslice().into_iter()
	}
}

impl<C, T> DoubleEndedIterator for IntoIter<C, T>
where C: Cursor, T: BitStore {
	fn next_back(&mut self) -> Option<Self::Item> {
		let mut slice_iter = self.iterator();
		let out = slice_iter.next_back();
		self.region = slice_iter.bitptr();
		out
	}
}

impl<C, T> ExactSizeIterator for IntoIter<C, T>
where C: Cursor, T: BitStore {}

impl<C, T> FusedIterator for IntoIter<C, T>
where C: Cursor, T: BitStore {}

impl<C, T> Iterator for IntoIter<C, T>
where C: Cursor, T: BitStore {
	type Item = bool;

	/// Advances the iterator by one, returning the first bit in it (if any).
	///
	/// # Parameters
	///
	/// - `&mut self`
	///
	/// # Returns
	///
	/// The leading bit in the iterator, if any.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let bv = vecbit![1, 0];
	/// let mut iter = bv.iter();
	/// assert!(iter.next().unwrap());
	/// assert!(!iter.next().unwrap());
	/// assert!(iter.next().is_none());
	/// ```
	fn next(&mut self) -> Option<Self::Item> {
		let mut slice_iter = self.iterator();
		let out = slice_iter.next();
		self.region = slice_iter.bitptr();
		out
	}

	/// Hints at the number of bits remaining in the iterator.
	///
	/// Because the exact size is always known, this always produces
	/// `(len, Some(len))`.
	///
	/// # Parameters
	///
	/// - `&self`
	///
	/// # Returns
	///
	/// - `usize`: The minimum bits remaining.
	/// - `Option<usize>`: The maximum bits remaining.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	///
	/// let bv = vecbit![0, 1];
	/// let mut iter = bv.iter();
	/// assert_eq!(iter.size_hint(), (2, Some(2)));
	/// iter.next();
	/// assert_eq!(iter.size_hint(), (1, Some(1)));
	/// iter.next();
	/// assert_eq!(iter.size_hint(), (0, Some(0)));
	/// ```
	fn size_hint(&self) -> (usize, Option<usize>) {
		self.iterator().size_hint()
	}

	/// Counts how many bits are live in the iterator, consuming it.
	///
	/// You are probably looking to use this on a borrowed iterator rather than
	/// an owning iterator. See [`SliceBit`].
	///
	/// # Parameters
	///
	/// - `self`
	///
	/// # Returns
	///
	/// The number of bits in the iterator.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	/// let bv = vecbit![BigEndian, u8; 0, 1, 0, 1, 0];
	/// assert_eq!(bv.into_iter().count(), 5);
	/// ```
	///
	/// [`SliceBit`]: ../struct.SliceBit.html#method.iter
	fn count(self) -> usize {
		self.vecbit.len()
	}

	/// Advances the iterator by `n` bits, starting from zero.
	///
	/// # Parameters
	///
	/// - `&mut self`
	/// - `n`: The number of bits to skip, before producing the next bit after
	///   skips. If this overshoots the iterator’s remaining length, then the
	///   iterator is marked empty before returning `None`.
	///
	/// # Returns
	///
	/// If `n` does not overshoot the iterator’s bounds, this produces the `n`th
	/// bit after advancing the iterator to it, discarding the intermediate
	/// bits.
	///
	/// If `n` does overshoot the iterator’s bounds, this empties the iterator
	/// and returns `None`.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	/// let bv = vecbit![BigEndian, u8; 0, 0, 0, 1];
	/// let mut iter = bv.into_iter();
	/// assert_eq!(iter.len(), 4);
	/// assert!(iter.nth(3).unwrap());
	/// assert!(iter.nth(0).is_none());
	/// ```
	fn nth(&mut self, n: usize) -> Option<Self::Item> {
		let mut slice_iter = self.iterator();
		let out = slice_iter.nth(n);
		self.region = slice_iter.bitptr();
		out
	}

	/// Consumes the iterator, returning only the last bit.
	///
	/// # Examples
	///
	/// ```rust
	/// use vecbit::prelude::*;
	/// let bv = vecbit![BigEndian, u8; 0, 0, 0, 1];
	/// assert!(bv.into_iter().last().unwrap());
	/// ```
	///
	/// Empty iterators return `None`
	///
	/// ```rust
	/// use vecbit::prelude::*;
	/// assert!(vecbit![].into_iter().last().is_none());
	/// ```
	fn last(mut self) -> Option<Self::Item> {
		self.next_back()
	}
}

/** A splicing iterator for `VecBit`.

This removes a segment from the vector and inserts another bitstream into its
spot. Any bits from the original `VecBit` after the removed segment are kept,
after the inserted bitstream.

Only the removed segment is available for iteration.

# Type Parameters

- `I: Iterator<Item=bool>`: Any bitstream. This will be used to fill the
  removed span.
**/
pub struct Splice<'a, C, T, I>
where C: Cursor, T: 'a + BitStore, I: Iterator<Item=bool> {
	drain: Drain<'a, C, T>,
	splice: I,
}

impl<'a, C, T, I> DoubleEndedIterator for Splice<'a, C, T, I>
where C: Cursor, T: 'a + BitStore, I: Iterator<Item=bool> {
	fn next_back(&mut self) -> Option<Self::Item> {
		self.drain.next_back()
	}
}

impl<'a, C, T, I> ExactSizeIterator for Splice<'a, C, T, I>
where C: Cursor, T: 'a + BitStore, I: Iterator<Item=bool> {}

impl<'a, C, T, I> FusedIterator for Splice<'a, C, T, I>
where C: Cursor, T: 'a + BitStore, I: Iterator<Item=bool> {}

//  Forward iteration to the interior drain
impl<'a, C, T, I> Iterator for Splice<'a, C, T, I>
where C: Cursor, T: 'a + BitStore, I: Iterator<Item=bool> {
	type Item = bool;

	fn next(&mut self) -> Option<Self::Item> {
		//  If the drain produced a bit, then try to pull a bit from the
		//  replacement. If the replacement produced a bit, push it into the
		//  `VecBit` that the drain is managing. This works because the `Drain`
		//  type truncates the `VecBit` to the front of the region being
		//  drained, then tracks the remainder of the memory.
		self.drain.next().map(|bit| {
			if let Some(new_bit) = self.splice.next() {
				unsafe { self.drain.vecbit.as_mut() }.push(new_bit);
			}
			bit
		})
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		self.drain.size_hint()
	}

	fn count(self) -> usize {
		self.drain.len()
	}

	fn nth(&mut self, n: usize) -> Option<Self::Item> {
		self.drain.nth(n)
	}

	fn last(mut self) -> Option<Self::Item> {
		self.drain.next_back()
	}
}

impl<'a, C, T, I> Drop for Splice<'a, C, T, I>
where C: Cursor, T: 'a + BitStore, I: Iterator<Item=bool> {
	fn drop(&mut self) { unsafe {
		if self.drain.tail_len == 0 {
			self.drain.vecbit.as_mut().extend(self.splice.by_ref());
			return;
		}

		//  Fill the drained span from the splice. If this exhausts the splice,
		//  exit. Note that `Drain::fill` runs from the current `VecBit.len`
		//  value, so the fact that `Splice::next` attempts to push onto the
		//  vector is not a problem here.
		if !self.drain.fill(&mut self.splice) {
			return;
		}

		let (lower, _) = self.splice.size_hint();

		//  If the splice still has data, move the tail to make room for it and
		//  fill.
		if lower > 0 {
			self.drain.move_tail(lower);
			if !self.drain.fill(&mut self.splice) {
				return;
			}
		}

		let mut remnant = self.splice.by_ref().collect::<Vec<_>>().into_iter();
		if remnant.len() > 0 {
			self.drain.move_tail(remnant.len());
			self.drain.fill(&mut remnant);
		}
		//  Drain::drop does the rest
	} }
}
