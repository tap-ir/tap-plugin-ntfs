use crate::attributecontent::MftAttributeContent;
use crate::attributes::bitmap::Bitmap;
use crate::attributes::list::AttributeListItem;
use crate::attributes::standard::StandardInformation;
use crate::attributes::filename::{FileName, NameSpace};
use crate::attributes::volume::{VolumeName, VolumeInformation};

#[derive(Debug, Clone, FromPrimitive, ToPrimitive, PartialOrd, PartialEq)]
#[repr(u32)]
pub enum NtfsAttributeType {
    StandardInformation = 16_u32,
    AttributeList = 32_u32,
    FileName = 48_u32,
    ObjectId = 64_u32,  //not implemented
    SecurityDescriptor = 80_u32, //not implemented
    VolumeName = 96_u32,
    VolumeInformation = 112_u32,
    Data = 128_u32,
    IndexRoot = 144_u32, //not implemented
    IndexAllocation = 160_u32, //not implemented
    Bitmap = 176_u32,
    ReparsePoint = 192_u32, //$SYMBOLIC_LINK to implem
    EaInformation = 208_u32, //not implemented
    EA = 224_u32,  //not implemented
    ProperySet = 240_u32,  //not implemented
    LoggedUtilityStream = 246_u32, //not implemented
}

#[derive(Debug)]
pub enum NtfsAttribute
{
  StandardInformation(StandardInformation),
  FileName(FileName),
  Data(MftAttributeContent),
  AttributeList(Vec<AttributeListItem>),
  VolumeName(VolumeName),
  VolumeInformation(VolumeInformation),
  Bitmap(Bitmap),
  Unknown(MftAttributeContent),
}

pub struct NtfsAttributes
{
  pub attributes : Vec<NtfsAttribute>
}

impl NtfsAttributes
{
  pub fn new(attributes : Vec<NtfsAttribute>) -> Self
  {
    NtfsAttributes{ attributes }
  }

  pub fn find_standard_info(&self) -> Vec<StandardInformation>
  {
    let mut attributes = Vec::new();

    for attribute in self.attributes.iter()
    {
      match &attribute
      {
        NtfsAttribute::StandardInformation(info) => attributes.push(info.clone()),
        _ => continue,
      }
    }

    attributes
  }

  pub fn find_datas(&self) -> Vec<&MftAttributeContent>
  {
    let mut attributes = Vec::new();

    for attribute in self.attributes.iter()
    {
      match &attribute
      {
        NtfsAttribute::Data(data) => attributes.push(data),
        _ => continue,
      }

    }

    attributes
  }

  pub fn find_filename(&self) -> Option<FileName>
  {
    let mut file_name = None;
    let mut name_space : Option<NameSpace> = None;

    for attribute in self.attributes.iter()
    {
      match attribute
      {
        NtfsAttribute::FileName(filename) => 
        {
          if let Some(_current_name_space) = name_space //XXX current_name_space is unused ? 
          {
            if filename.name_space == NameSpace::Win32 || filename.name_space == NameSpace::DosWin32
            {
              name_space = Some(filename.name_space);
              file_name = Some(filename.clone());
            }
          }
          else
          {
            name_space = Some(filename.name_space);
            file_name =  Some(filename.clone());
          }
        }
        _ => continue,
      }
    }
    file_name
  }
}
