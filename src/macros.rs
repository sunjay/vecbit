/*! Utility macros for constructing data structures and implementing bulk types.

The only public macro is `vecbit`; this module also provides convenience macros
for code generation.
!*/

/** Construct a `VecBit` out of a literal array in source code, like `vec!`.

`vecbit!` can be invoked in a number of ways. It takes the name of a `Cursor`
implementation, the name of a `BitStore`-implementing fundamental, and zero or
more fundamentals (integer, floating-point, or boolean) which are used to build
the bits. Each fundamental literal corresponds to one bit, and is considered to
represent `1` if it is any other value than exactly zero.

`vecbit!` can be invoked with no specifiers, a `Cursor` specifier, or a `Cursor`
and a `BitStore` specifier. It cannot be invoked with a `BitStore` specifier but
no `Cursor` specifier, due to overlap in how those tokens are matched by the
macro system.

Like `vec!`, `vecbit!` supports bit lists `[0, 1, …]` and repetition markers
`[1; n]`.

# Notes

The bit list syntax `vecbit![expr, expr, expr...]` currently produces an
`&[bool]` slice of the initial pattern, which is written into the final
artifact’s static memory and may consume excessive space.

The repetition syntax `bitec![expr; count]` currently zeros its allocated buffer
before setting the first `count` bits to `expr`. This may result in a
performance penalty when using `vecbit![1; N]`, as the allocation will be zeroed
and then a subset will be set high.

This behavior is currently required to maintain compatibility with `serde`
expectations that dead bits are zero. As the `serdes` module removes those
expectations, the repetition syntax implementation may speed up.

# Examples

```rust
use vecbit::prelude::*;

vecbit![BigEndian, u8; 0, 1];
vecbit![LittleEndian, u8; 0, 1,];
vecbit![BigEndian; 0, 1];
vecbit![LittleEndian; 0, 1,];
vecbit![0, 1];
vecbit![0, 1,];
vecbit![BigEndian, u8; 1; 5];
vecbit![LittleEndian; 0; 5];
vecbit![1; 5];
```
**/
#[cfg(feature = "alloc")]
#[macro_export]
macro_rules! vecbit {
	//  vecbit![ endian , type ; 0 , 1 , … ]
	( $cursor:path , $bits:ty ; $( $val:expr ),* ) => {
		vecbit![ __bv_impl__ $cursor , $bits ; $( $val ),* ]
	};
	//  vecbit![ endian , type ; 0 , 1 , … , ]
	( $cursor:path , $bits:ty ; $( $val:expr , )* ) => {
		vecbit![ __bv_impl__ $cursor , $bits ; $( $val ),* ]
	};

	//  vecbit![ endian ; 0 , 1 , … ]
	( $cursor:path ; $( $val:expr ),* ) => {
		vecbit![ __bv_impl__ $cursor , $crate::store::Word ; $( $val ),* ]
	};
	//  vecbit![ endian ; 0 , 1 , … , ]
	( $cursor:path ; $( $val:expr , )* ) => {
		vecbit![ __bv_impl__ $cursor , $crate::store::Word ; $( $val ),* ]
	};

	//  vecbit![ 0 , 1 , … ]
	( $( $val:expr ),* ) => {
		vecbit![ __bv_impl__ $crate::cursor::Local , $crate::store::Word ; $( $val ),* ]
	};
	//  vecbit![ 0 , 1 , … , ]
	( $( $val:expr , )* ) => {
		vecbit![ __bv_impl__ $crate::cursor::Local , $crate::store::Word ; $( $val ),* ]
	};

	//  vecbit![ endian , type ; bit ; rep ]
	( $cursor:path , $bits:ty ; $val:expr ; $rep:expr ) => {
		vecbit![ __bv_impl__ $cursor , $bits ; $val; $rep ]
	};
	//  vecbit![ endian ; bit ; rep ]
	( $cursor:path ; $val:expr ; $rep:expr ) => {
		vecbit![ __bv_impl__ $cursor , $crate::store::Word ; $val ; $rep ]
	};
	//  vecbit![ bit ; rep ]
	( $val:expr ; $rep:expr ) => {
		vecbit![ __bv_impl__ $crate::cursor::Local , $crate::store::Word ; $val ; $rep ]
	};

	//  GitHub issue #25 is to make this into a proc-macro that produces the
	//  correct memory slab at compile time.

	( __bv_impl__ $cursor:path , $bits:ty ; $( $val:expr ),* ) => {{
		let init: &[bool] = &[ $( $val != 0 ),* ];
		let mut bv = $crate::vec::VecBit::<$cursor, $bits>::with_capacity(
			init.len(),
		);
		bv.extend(init.iter().copied());
		bv
	}};

	//  `[$val; $rep]` can just allocate a slab of at least `$rep` bits and then
	//  use `.set_all` to force them to `$val`. This is much faster than
	//  collecting from a bitstream.

	( __bv_impl__ $cursor:path , $bits:ty ; $val:expr ; $rep:expr ) => {{
		let mut bv = $crate::vec::VecBit::<$cursor, $bits>::with_capacity($rep);
		bv.set_elements(0);
		unsafe { bv.set_len($rep); }
		let one = $val != 0;
		if one {
			bv.set_all(one);
		}
		bv
	}};
}

/** Construct a `BitBox` out of a literal array in source code, like `vecbit!`.

This has exactly the same syntax as [`vecbit!`], and in fact is a thin wrapper
around `vecbit!` that calls `.into_boxed_slice()` on the produced `VecBit` to
freeze it.

[`vecbit!`]: #macro.vecbit
**/
#[cfg(feature = "alloc")]
#[macro_export]
macro_rules! bitbox {
	//  bitbox![ endian , type ; 0 , 1 , … ]
	( $cursor:path , $bits:ty ; $( $val:expr ),* ) => {
		vecbit![ $cursor , $bits ; $( $val ),* ].into_boxed_bitslice()
	};
	//  bitbox![ endian , type ; 0 , 1 , … , ]
	( $cursor:path , $bits:ty ; $( $val:expr , )* ) => {
		vecbit![ $cursor , $bits ; $( $val ),* ].into_boxed_bitslice()
	};

	//  bitbox![ endian ; 0 , 1 , … ]
	( $cursor:path ; $( $val:expr ),* ) => {
		vecbit![ $cursor , $crate::store::Word ; $( $val ),* ].into_boxed_bitslice()
	};
	//  bitbox![ endian ; 0 , 1 , … , ]
	( $cursor:path ; $( $val:expr , )* ) => {
		vecbit![ $cursor , $crate::store::Word ; $( $val ),* ].into_boxed_bitslice()
	};

	//  bitbox![ 0 , 1 , … ]
	( $( $val:expr ),* ) => {
		vecbit![ $crate::cursor::Local , $crate::store::Word ; $( $val ),* ].into_boxed_bitslice()
	};
	//  bitbox![ 0 , 1 , … , ]
	( $( $val:expr , )* ) => {
		vecbit![ $crate::cursor::Local , $crate::store::Word ; $( $val ),* ].into_boxed_bitslice()
	};

	//  bitbox![ endian , type ; bit ; rep ]
	( $cursor:path , $bits:ty ; $val:expr ; $rep:expr ) => {
		vecbit![ $cursor , $bits ; $val; $rep ].into_boxed_bitslice()
	};
	//  bitbox![ endian ; bit ; rep ]
	( $cursor:path ; $val:expr ; $rep:expr ) => {
		vecbit![ $cursor , $crate::store::Word ; $val ; $rep ].into_boxed_bitslice()
	};
	//  bitbox![ bit ; rep ]
	( $val:expr ; $rep:expr ) => {
		vecbit![ $crate::cursor::Local , $crate::store::Word ; $val ; $rep ].into_boxed_bitslice()
	};
}

#[doc(hidden)]
macro_rules! __bitslice_shift {
	( $( $t:ty ),+ ) => { $(
		#[doc(hidden)]
		impl<C, T >core::ops::ShlAssign<$t>
		for $crate::prelude::SliceBit<C,T>
		where C: $crate::cursor::Cursor, T: $crate::store::BitStore {
			fn shl_assign(&mut self, shamt: $t) {
				core::ops::ShlAssign::<usize>::shl_assign(
					self,
					shamt as usize,
				)
			}
		}

		#[doc(hidden)]
		impl<C, T> core::ops::ShrAssign<$t>
		for $crate::prelude::SliceBit<C,T>
		where C: $crate::cursor::Cursor, T: $crate::store::BitStore {
			fn shr_assign(&mut self,shamt: $t){
				core::ops::ShrAssign::<usize>::shr_assign(
					self,
					shamt as usize,
				)
			}
		}
	)+ };
}

#[cfg(feature = "alloc")]
#[doc(hidden)]
macro_rules! __vecbit_shift {
	( $( $t:ty ),+ ) => { $(
		#[doc(hidden)]
		impl<C, T> core::ops::Shl<$t>
		for $crate::vec::VecBit<C, T>
		where C: $crate::cursor::Cursor, T: $crate::store::BitStore {
			type Output = <Self as core::ops::Shl<usize>>::Output;

			fn shl(self, shamt: $t) -> Self::Output {
				core::ops::Shl::<usize>::shl(self, shamt as usize)
			}
		}

		#[doc(hidden)]
		impl<C, T> core::ops::ShlAssign<$t>
		for $crate::vec::VecBit<C, T>
		where C: $crate::cursor::Cursor, T: $crate::store::BitStore {
			fn shl_assign(&mut self, shamt: $t) {
				core::ops::ShlAssign::<usize>::shl_assign(
					self,
					shamt as usize,
				)
			}
		}

		#[doc(hidden)]
		impl<C, T> core::ops::Shr<$t>
		for $crate::vec::VecBit<C, T>
		where C: $crate::cursor::Cursor, T: $crate::store::BitStore {
			type Output = <Self as core::ops::Shr<usize>>::Output;

			fn shr(self, shamt: $t) -> Self::Output {
				core::ops::Shr::<usize>::shr(self, shamt as usize)
			}
		}

		#[doc(hidden)]
		impl<C, T> core::ops::ShrAssign<$t>
		for $crate::vec::VecBit<C, T>
		where C: $crate::cursor::Cursor, T: $crate::store::BitStore {
			fn shr_assign(&mut self, shamt: $t) {
				core::ops::ShrAssign::<usize>::shr_assign(
					self,
					shamt as usize,
				)
			}
		}
	)+ };
}

#[cfg(all(test, any(feature = "alloc", feature = "std")))]
mod tests {
	#[allow(unused_imports)]
	use crate::cursor::{
		BigEndian,
		LittleEndian,
	};

	#[test]
	fn compile_vecbit_macros() {
		vecbit![0, 1];
		vecbit![BigEndian; 0, 1];
		vecbit![LittleEndian; 0, 1];
		vecbit![BigEndian, u8; 0, 1];
		vecbit![LittleEndian, u8; 0, 1];
		vecbit![BigEndian, u16; 0, 1];
		vecbit![LittleEndian, u16; 0, 1];
		vecbit![BigEndian, u32; 0, 1];
		vecbit![LittleEndian, u32; 0, 1];
		vecbit![BigEndian, u64; 0, 1];
		vecbit![LittleEndian, u64; 0, 1];

		vecbit![1; 70];
		vecbit![BigEndian; 0; 70];
		vecbit![LittleEndian; 1; 70];
		vecbit![BigEndian, u8; 0; 70];
		vecbit![LittleEndian, u8; 1; 70];
		vecbit![BigEndian, u16; 0; 70];
		vecbit![LittleEndian, u16; 1; 70];
		vecbit![BigEndian, u32; 0; 70];
		vecbit![LittleEndian, u32; 1; 70];
		vecbit![BigEndian, u64; 0; 70];
		vecbit![LittleEndian, u64; 1; 70];
	}

	#[test]
	fn compile_bitbox_macros() {
		bitbox![0, 1];
		bitbox![BigEndian; 0, 1];
		bitbox![LittleEndian; 0, 1];
		bitbox![BigEndian, u8; 0, 1];
		bitbox![LittleEndian, u8; 0, 1];
		bitbox![BigEndian, u16; 0, 1];
		bitbox![LittleEndian, u16; 0, 1];
		bitbox![BigEndian, u32; 0, 1];
		bitbox![LittleEndian, u32; 0, 1];
		bitbox![BigEndian, u64; 0, 1];
		bitbox![LittleEndian, u64; 0, 1];

		bitbox![1; 70];
		bitbox![BigEndian; 0; 70];
		bitbox![LittleEndian; 1; 70];
		bitbox![BigEndian, u8; 0; 70];
		bitbox![LittleEndian, u8; 1; 70];
		bitbox![BigEndian, u16; 0; 70];
		bitbox![LittleEndian, u16; 1; 70];
		bitbox![BigEndian, u32; 0; 70];
		bitbox![LittleEndian, u32; 1; 70];
		bitbox![BigEndian, u64; 0; 70];
		bitbox![LittleEndian, u64; 1; 70];
	}
}
