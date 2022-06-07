use std::io::SeekFrom;

use tap::vfile::{VFile, read_utf16_exact};

use crate::error::NtfsError;
use crate::ntfsattributes::NtfsAttributeType;
use crate::attributecontent::{Resident, NonResident, ResidentType};

use anyhow::Result;
use byteorder::{ByteOrder, LittleEndian};
use num_traits::FromPrimitive;


#[derive(Debug)]
pub struct MftAttribute
{
  pub type_id           : NtfsAttributeType,
  pub length            : u32,
  pub non_resident_flag : u8,
  pub name_size         : u8,
  pub name_offset       : u16,
  pub flags             : u16,
  pub id                : u16,
  pub name              : Option<String>,
  pub data              : ResidentType,
}
impl MftAttribute
{
  pub fn from_file<T : VFile>(mut file : &mut T, offset : u32) -> Result<MftAttribute>
  {
    file.seek(SeekFrom::Start(offset as u64))?;
    let mut data = [0;16]; 
    file.read_exact(&mut data)?;

    let type_id = LittleEndian::read_u32(&data[0..4]);
    if type_id == 0xffffffff
    {
      return Err(NtfsError::MftAttributesEnd.into()); //return specific error as need to be catched
    }

    let type_id = match NtfsAttributeType::from_u32(type_id) 
    {
      Some(attribute_type) => attribute_type,
      None => return Err(NtfsError::MftAttributeUnknownType(type_id).into())
    };

    let length = LittleEndian::read_u32(&data[4..8]);
    let non_resident_flag = data[8];
    let name_size = data[9];
    let name_offset = LittleEndian::read_u16(&data[10..12]);
    let flags = LittleEndian::read_u16(&data[12..15]);
    let id = LittleEndian::read_u16(&data[14..16]);

    let data = match non_resident_flag
    {
      0 => ResidentType::Resident(Resident::from_file(&mut file)?),
      1 => ResidentType::NonResident(NonResident::from_file(&mut file, offset)?),
      _ => return Err(NtfsError::MftAttributeDataType.into()),
    };

    let name = match name_size 
    {
      0 => None, 
      size => { file.seek(SeekFrom::Start(offset as u64 + name_offset as u64))?;
                Some(read_utf16_exact(file, size as usize * 2)?) }
    };

    Ok(MftAttribute{
      name, 
      type_id,
      length,
      non_resident_flag,
      name_size,
      name_offset,
      flags, 
      id,
      data,
    })
  }

  pub fn is_compressed(&self) -> bool
  {
    match self.data
    {
      ResidentType::Resident(_) => false,
      ResidentType::NonResident(_) => (self.flags & 0x0001) == 0x0001,
    }
  }

  pub fn is_encrypted(&self) -> bool
  {
    match self.data
    {
      ResidentType::Resident(_) => false,
      ResidentType::NonResident(_) => (self.flags & 0x4000) == 0x4000,
    }
  }

  pub fn is_sparse(&self) -> bool
  {
    match self.data
    {
      ResidentType::Resident(_) => false,
      ResidentType::NonResident(_) => (self.flags & 0x8000) == 0x8000,
    }
  }
}

