/*! `vecbit` Prelude

This collects the general public API into a single spot for inclusion, as
`use vecbit::prelude::*;`, without polluting the root namespace of the crate.
!*/

pub use crate::{
	bits::{
		Bits,
		BitsMut,
	},
	cursor::{
		Cursor,
		BigEndian,
		LittleEndian,
		Local,
	},
	fields::BitField,
	slice::SliceBit,
	store::{
		BitStore,
		Word,
	},
};

#[cfg(feature = "alloc")]
pub use crate::{
	bitbox,
	vecbit,
	boxed::BitBox,
	vec::VecBit,
};
