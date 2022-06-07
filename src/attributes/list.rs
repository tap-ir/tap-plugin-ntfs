use std::sync::Arc;
use std::io::SeekFrom;
use std::io::Seek;

use tap::vfile::{VFile, VFileBuilder, read_utf16_exact};

use anyhow::Result;
use byteorder::{ByteOrder, LittleEndian};
use num_traits::FromPrimitive;
use seek_bufread::BufReader;

use crate::error::NtfsError;
use crate::attributecontent::pad_u64;
use crate::ntfsattributes::NtfsAttributeType;

#[derive(Debug)]
pub struct AttributeListItem
{
  pub name         : Option<String>,
  pub type_id      : NtfsAttributeType,
  pub size         : u16,
  pub name_size    : u8,
  pub name_offset  : u8,
  pub vnc_start    : u64,
  pub mft_entry_id : u64,
  pub sequence     : u16,
  pub id           : u16,
}

impl AttributeListItem 
{
  pub fn new<T : VFile>(mut file : &mut T) -> Result<Self>
  {
    let mut data = [0;26]; 
    file.read_exact(&mut data)?;

    let type_id = LittleEndian::read_u32(&data[0..4]);
    if type_id == 0xffffffff
    {
      return Err(NtfsError::MftAttributeListEnd.into()); //return specific error 
    }

    let type_id = match NtfsAttributeType::from_u32(type_id) 
    {
      Some(attribute_type) => attribute_type,
      None => return Err(NtfsError::MftAttributeUnknownType(type_id).into())
    };


    let size = LittleEndian::read_u16(&data[4..6]);
    let name_size = data[6];
    let name_offset = data[7];
    let vnc_start = LittleEndian::read_u64(&data[8..16]);
    let mft_entry_id = pad_u64(&data[16..22]);
    let sequence = LittleEndian::read_u16(&data[22..24]); 
    let id = LittleEndian::read_u16(&data[24..26]);

    let name = match name_size 
    {
      0 => None, 
      size => { file.seek(SeekFrom::Start(name_offset as u64))?;
                Some(read_utf16_exact(&mut file, size as usize)?) }
    };

    Ok(AttributeListItem{
      name,
      type_id,
      size,
      name_size,
      name_offset,
      vnc_start,
      mft_entry_id,
      sequence,
      id,
    })
  }
}

#[derive(Debug)]
pub struct AttributeList
{
}

impl AttributeList
{
  pub fn new(content : Arc<dyn VFileBuilder>)-> Result<Vec<AttributeListItem>>
  {
    let file = content.open()?;
    let mut file = BufReader::new(file);

    let mut attributes = Vec::new();
    let mut previous_offset;

    while file.tell()? < content.size()// && previous_offset < file.tell()?
    {
      previous_offset = file.tell()?; 
      match AttributeListItem::new(&mut file)
      {
        Ok(attribute) => {
                            file.seek(SeekFrom::Start(previous_offset + attribute.size as u64))?;
                            attributes.push(attribute); 
                         }
        Err(_err) => break,
      }
    }

    Ok(attributes)
  }
}
