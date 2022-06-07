#![allow(dead_code)]
use std::sync::Arc;
use std::io::SeekFrom;
use std::io::Seek;

use tap::vfile::{VFileBuilder, read_utf16_exact};

use anyhow::Result;

#[derive(Debug)]
pub struct VolumeInformation
{
  version : String,
  major   : u8,
  minor   : u8,
  //flags :
}

//XXX add as node attribute 
impl VolumeInformation 
{
  pub fn new(content : Arc<dyn VFileBuilder>) -> Result<Self>
  {
    let mut file = content.open()?;

    file.seek(SeekFrom::Start(8))?;
    let mut data = [0;4]; 
    file.read_exact(&mut data)?;

    let major = data[0];
    let minor = data[1];

    let version = match major
    {
      1 => match minor
      {
        1 => "1.1 (Windows NT4)".into(), 
        2 => "1.2 (Windows NT4)".into(),
        _ => format!("1.{}", minor),
      }
      2 => format!("{}:{} (Windows 200 Beta)", major, minor),
      3 => match minor
      {
        0 => "3.0 (Windows 2000)".into(), 
        1 => "3.1 (Windows XP, 2003, Vista)".into(),
        _ => format!("3.{}", minor),
      }
      _ => format!("{}.{}", major, minor),
    };

    Ok(VolumeInformation{
      version,
      major,
      minor,
    })
  }
}

//XXX add as node attribute 
#[derive(Debug)]
pub struct VolumeName
{
  name : String,
}

impl VolumeName
{
  pub fn new(content : Arc<dyn VFileBuilder>) -> Result<Self>
  {
    let mut file = content.open()?;

    //check size XXX (at least mft entry size)
    let name = read_utf16_exact(&mut file, content.size() as usize)?;

    Ok(VolumeName{ name })
  }
}
