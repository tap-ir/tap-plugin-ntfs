use std::sync::Arc;
use std::io::Read;

use tap::value::Value;
use tap::reflect::ReflectStruct;
use tap::datetime::WindowsTimestamp;
use tap::vfile::{VFileBuilder, read_utf16_exact};
use tap_derive::Reflect;

use anyhow::Result;
use byteorder::{ByteOrder, LittleEndian};
use num_traits::FromPrimitive;
use chrono::{DateTime, Utc};

use crate::attributecontent::pad_u64;
use crate::attributes::FileAttributes;
use crate::error::NtfsError;

#[derive(FromPrimitive, Clone, Copy, Debug, PartialOrd, PartialEq)]
#[repr(u8)]
pub enum NameSpace
{
  Posix = 0,
  Win32 = 1,
  Dos = 2,
  DosWin32 = 3,
}

#[derive(Debug, Reflect, Clone)]
pub struct FileName 
{
  #[reflect(skip)]
  pub file_name : String,
  pub parent_mft_entry_id : u64,
  #[reflect(skip)]
  pub parent_sequence : u16,
  pub creation_time : DateTime<Utc>,
  pub modification_time : DateTime<Utc>,
  pub mft_modification_time : DateTime<Utc>,
  pub accessed_time : DateTime<Utc>,
  #[reflect(skip)]
  pub allocated_size : u64, 
  #[reflect(skip)]
  pub real_size : u64,
  #[reflect(skip)]
  pub flags : FileAttributes, 
  #[reflect(skip)]
  pub reparse_value : u32,
  #[reflect(skip)]
  pub name_length : u8,
  #[reflect(skip)]
  pub name_space : NameSpace,
}

impl FileName 
{
  pub fn new(content : Arc<dyn VFileBuilder>) -> Result<Self>
  {
    //let _size = content.size(); check size ?
    let mut file = content.open()?;

    let mut data = [0;66]; 
    file.read_exact(&mut data)?;

    let parent_mft_entry_id = pad_u64(&data[0..6]);
    let parent_sequence = LittleEndian::read_u16(&data[6..8]);
    let creation_time = WindowsTimestamp(LittleEndian::read_u64(&data[8..16])).to_datetime()?;
    let modification_time = WindowsTimestamp(LittleEndian::read_u64(&data[16..24])).to_datetime()?;
    let mft_modification_time  = WindowsTimestamp(LittleEndian::read_u64(&data[24..32])).to_datetime()?;
    let accessed_time = WindowsTimestamp(LittleEndian::read_u64(&data[32..40])).to_datetime()?;
    let allocated_size = LittleEndian::read_u64(&data[40..48]);
    let real_size = LittleEndian::read_u64(&data[48..56]);
    let flags = FileAttributes::from_bits_truncate(LittleEndian::read_u32(&data[56..60]));
    let reparse_value = LittleEndian::read_u32(&data[60..64]);
    let name_length = data[64];

    let name_space = NameSpace::from_u8(data[65]).ok_or(NtfsError::MftAttributeUnknownNameSpace(data[65]))?;

    if (name_length as u64) * 2 > content.size() - 66//check if > size - offset ?
    {
      return Err(NtfsError::MftAttributeNameSpaceInvalidSize.into())
    }

    //we prefer to return error if we have an invalid filename 
    //and consider the full structure as invalid
    let file_name = read_utf16_exact(&mut file, (name_length as usize) * 2)?; 

    Ok(FileName{
      file_name,
      parent_mft_entry_id,
      parent_sequence,
      creation_time,
      modification_time,
      mft_modification_time,
      accessed_time,
      allocated_size, 
      real_size,
      flags,
      reparse_value,
      name_length,
      name_space,
    })
  }
}
