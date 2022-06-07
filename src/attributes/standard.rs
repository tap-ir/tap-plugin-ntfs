use std::sync::Arc;
use std::io::Read;

use tap::vfile::VFileBuilder;
use tap::reflect::{ReflectStruct};
use tap::value::Value;
use tap::datetime::WindowsTimestamp;
use tap_derive::Reflect;

use anyhow::Result;
use byteorder::{ByteOrder, LittleEndian};
use chrono::{DateTime, Utc};

use crate::error::NtfsError;
use crate::attributes::FileAttributes;

#[derive(Debug, Reflect, Clone)]
pub struct StandardInformation
{
  pub creation_time : DateTime<Utc>,
  pub altered_time : DateTime<Utc>,
  pub mft_altered_time : DateTime<Utc>,
  pub accessed_time : DateTime<Utc>,
  #[reflect(skip)]
  pub flags : FileAttributes,
  pub version_maximum_number : u32,
  pub version_number : u32,
  pub class_id : u32,

  #[reflect(skip)]
  pub owner_id : Option<u32>,
  #[reflect(skip)]
  pub security_id : Option<u32>,
  #[reflect(skip)]
  pub quota_charged : Option<u64>,
  #[reflect(skip)]
  pub usn : Option<u64>,
}

impl StandardInformation
{
  pub fn new(content : Arc<dyn VFileBuilder>) -> Result<Self>
  {
    let size = content.size();
    
    if size < 48 && size != 72
    {
      return Err(NtfsError::MftAttributeStandardInvalidSize.into())
    };
   
    let mut file = content.open()?;

    let mut data = [0;48]; 
    file.read_exact(&mut data)?;

    let creation_time = WindowsTimestamp(LittleEndian::read_u64(&data[0..8])).to_datetime()?;
    let altered_time  = WindowsTimestamp(LittleEndian::read_u64(&data[8..16])).to_datetime()?;
    let mft_altered_time  = WindowsTimestamp(LittleEndian::read_u64(&data[16..24])).to_datetime()?;
    let accessed_time = WindowsTimestamp(LittleEndian::read_u64(&data[24..32])).to_datetime()?;
    let flags = FileAttributes::from_bits_truncate(LittleEndian::read_u32(&data[32..36]));
    let version_maximum_number = LittleEndian::read_u32(&data[36..40]);
    let version_number = LittleEndian::read_u32(&data[40..44]);
    let class_id = LittleEndian::read_u32(&data[44..48]);

    if size == 72
    {
      let mut data = [0; 24];
      file.read_exact(&mut data)?;

      let owner_id = Some(LittleEndian::read_u32(&data[0..4]));
      let security_id = Some(LittleEndian::read_u32(&data[4..8]));
      let quota_charged = Some(LittleEndian::read_u64(&data[8..16]));
      let usn =  Some(LittleEndian::read_u64(&data[16..24]));

      Ok(StandardInformation{
        creation_time,
        altered_time,
        mft_altered_time,
        accessed_time,
        flags,
        version_maximum_number,
        version_number,
        class_id,
        owner_id,
        security_id,
        quota_charged,
        usn,
      })
    }
    else
    {
      Ok(StandardInformation{
        creation_time,
        altered_time,
        mft_altered_time,
        accessed_time,
        flags,
        version_maximum_number,
        version_number,
        class_id,
        owner_id : None,
        security_id : None,
        quota_charged : None,
        usn : None,
      })
    }
  }
}
