use std::sync::Arc;
use std::io::SeekFrom;
use std::io::Seek;
use std::io::Read;

use tap::vfile::VFileBuilder;
use tap::mappedvfile::{MappedVFileBuilder,FileRanges};

use crate::error::NtfsError;
use crate::mft::MftEntries;
use crate::ntfsattributes::{NtfsAttribute, NtfsAttributes, NtfsAttributeType};
use crate::attributecontent::MftAttributeContent;
use crate::attributecontent::pad_u64;
use crate::attributes::standard::StandardInformation;
use crate::attributes::filename::FileName;
use crate::attributes::list::AttributeList;
use crate::attributes::volume::{VolumeName, VolumeInformation};

use anyhow::Result;
use byteorder::{ByteOrder, LittleEndian};


/**
 *  MFTEntry
 */
pub const MFT_SIGNATURE_FILE : u32 = 0x454C4946; //FILE
pub const MFT_SIGNATURE_BAAD : u32 = 0x44414142; //BAAD

#[derive(Debug)]
pub struct MftEntry
{
  pub partition_builder : Option<Arc<dyn VFileBuilder>>,
  pub mft_builder : Arc<dyn VFileBuilder>, //partition or full mft file 
  pub zero_builder : Option<Arc<dyn VFileBuilder>>,
  pub offset : u64,
  pub record_size : u32,
  pub signature : u32,
  pub fixup_array_offset : u16,
  pub fixup_array_entry_count : u16,
  pub lsn : u64,
  pub sequence : u16,
  pub link_count : u16,
  pub first_attribute_offset : u16,
  pub flags : u16,
  pub used_size : u32,
  pub allocated_size : u32,
  pub file_reference_id : u64,
  pub file_reference_sequence : u16,
  pub next_attribute_id : u16,
  pub sector_size : u16,
  pub cluster_size : Option<u32>,
}

impl MftEntry
{
  pub fn from_offset(offset : u64, partition_builder : Option<Arc<dyn VFileBuilder>>, mft_builder : Arc<dyn VFileBuilder>, zero_builder : Option<Arc<dyn VFileBuilder>>, record_size : u32, sector_size : u16, cluster_size : Option<u32>) -> Result<MftEntry>
  {
    let mut file = mft_builder.open()?;

    file.seek(SeekFrom::Start(offset))?;

    //let offset = file.tell(); //we get our absolute offset 
    let mut data = [0;42]; 
    file.read_exact(&mut data)?;
    //first 3 u8 contain the jmp code

    let signature = LittleEndian::read_u32(&data[0..4]);

    //if (signature != MFT_SIGNATURE_FILE) // && signature != MFT_SIGNATURE_BAAD) 
    //{
      //return Err(NtfsError::MftInvalidSignature.into())
    //}
    //XXX if is baad set as deleted

    let fixup_array_offset = LittleEndian::read_u16(&data[4..6]);
    let fixup_array_entry_count = LittleEndian::read_u16(&data[6..8]);
    let fixup_array_entry_count = if fixup_array_entry_count > 0
    {
      fixup_array_entry_count - 1
    }
    else
    {
      fixup_array_entry_count
    };

    let lsn = LittleEndian::read_u64(&data[8..16]);
    let sequence = LittleEndian::read_u16(&data[16..18]);
    let link_count = LittleEndian::read_u16(&data[18..20]);
    let first_attribute_offset = LittleEndian::read_u16(&data[20..22]);
    let flags = LittleEndian::read_u16(&data[22..24]);
    let used_size = LittleEndian::read_u32(&data[24..28]);
    if used_size == 0xffffffff
    {
      return Err(NtfsError::MftUnusedEntry{}.into());
    }
    let allocated_size = LittleEndian::read_u32(&data[28..32]);
    //let file_reference_to_base_record = LittleEndian::read_u64(&data[32..40]);
    let file_reference_id = pad_u64(&data[32..38]); 
    let file_reference_sequence = LittleEndian::read_u16(&data[38..40]); 
    let next_attribute_id = LittleEndian::read_u16(&data[40..42]);

    let mft_entry = MftEntry{
        partition_builder,
        mft_builder,
        zero_builder,
        offset,
        record_size,
        signature, 
        fixup_array_offset,
        fixup_array_entry_count,
        lsn,
        sequence,
        link_count,
        first_attribute_offset,
        flags,
        used_size,
        allocated_size,
        //file_reference_to_base_record,
        file_reference_id,
        file_reference_sequence,
        next_attribute_id,
        sector_size,
        cluster_size,
    };

    Ok(mft_entry)
  }

  pub fn contents(&self) -> Vec<MftAttributeContent>
  {
    let mut contents = Vec::new();
    let mft_entry = self.to_builder();
    let mut file = match mft_entry.open()
    {
      Ok(file) => file,
      Err(_) => return contents,
    };
    let mut offset : u32 = self.first_attribute_offset as u32;

    while offset < self.used_size
    {
       //entry builder for resident, whole dump builder for nonresident
       let content = match MftAttributeContent::from_file(&mut file, offset, mft_entry.clone(), &self.partition_builder, &self.zero_builder, self.cluster_size)
       {
         Ok(content) => content,
         Err(_err) => break, //XXX catch end of attribute here to stop
       };

       let mft_attribute_length = content.mft_attribute.length;
       contents.push(content);
       if mft_attribute_length == 0
       {    
          break
       }
       offset += mft_attribute_length;
    }

    contents
  }

  fn content_to_attribute(&self, content : MftAttributeContent, mft_entries : Option<&MftEntries>) ->Vec<NtfsAttribute>
  {
    let mut attributes : Vec<NtfsAttribute> = Vec::new();
    let builder = match content.builder()
    {
      Ok(builder) => builder,
      //Happen if we read a non-resident on a MFT (XXX use specific error)
      Err(_err)=> return Vec::new(),
    };

    match &content.mft_attribute.type_id
    {
      NtfsAttributeType::StandardInformation => if let Ok(attribute) =  StandardInformation::new(builder)
      {
        attributes.push(NtfsAttribute::StandardInformation(attribute));
      },
      NtfsAttributeType::FileName => if let Ok(attribute) = FileName::new(builder)
      {
        attributes.push(NtfsAttribute::FileName(attribute));
      },
      NtfsAttributeType::Data => attributes.push(NtfsAttribute::Data(content)),
      NtfsAttributeType::VolumeName => if let Ok(attribute) = VolumeName::new(builder)
      {
        attributes.push(NtfsAttribute::VolumeName(attribute));
      },
      NtfsAttributeType::VolumeInformation => if let Ok(attribute) =  VolumeInformation::new(builder)
      {
        attributes.push(NtfsAttribute::VolumeInformation(attribute));
      },
      //NtfsAttributeType::Bitmap => match Bitmap::new(&content)
      //{
        //Ok(attribute) => attributes.push(NtfsAttribute::Bitmap(attribute)),
        //Err(_) => (),
      //}
      NtfsAttributeType::AttributeList => if let Ok(items)  = AttributeList::new(builder)
      {
        for item in items
        {
          if let Some(mft_entries) = mft_entries
          {
            if let Ok(entry) = mft_entries.entry(item.mft_entry_id)
            {
              for content in entry.contents()
              {
                //if attribute id == itemid && attribute vnc start (or is non resident) 
                if item.id == content.mft_attribute.id 
                {
                  let attribute = self.content_to_attribute(content, Some(mft_entries));
                  attributes.extend(attribute);
                }
              }
            }
          }
        }
      },
      _ => (),
    };
    attributes 
  }

  //return an iterator ?
  pub fn read_attributes(&self, mft_entries : Option<&MftEntries>) -> NtfsAttributes 
  {
    NtfsAttributes::new(self.contents().into_iter().flat_map(|content| self.content_to_attribute(content, mft_entries)).collect())
  }

  pub fn data_attribute(&self) -> Result<Arc<dyn VFileBuilder>>
  {
    for attribute_content in self.read_attributes(None).attributes.iter()
    {
      match &attribute_content
      {
        //error if we use MFT has we don't handle non-resident attribute
        NtfsAttribute::Data(data) => return data.builder(),
        _ => continue,
      }
    }
    Err(NtfsError::MftAttributeNotFound("data").into())
  }

  pub fn size(&self) -> u64
  {
    self.record_size as u64
  }

  pub fn is_valid(&self) -> bool
  {
    self.signature != MFT_SIGNATURE_FILE && self.signature != MFT_SIGNATURE_BAAD
  }

  pub fn is_used(&self) -> bool
  {
    self.flags & 0x1 != 0
  }

  pub fn is_directory(&self) -> bool
  {
    self.flags & 0x2 != 0 
  }

  pub fn to_builder(&self) -> Arc<dyn VFileBuilder>
  {
    let mut file_ranges = FileRanges::new();
    let mut offset : u64 = 0;
    let sector_size = self.sector_size as u64;

    while offset < self.size()
    {
      if self.size() - offset >= sector_size
      {
        let range = offset..offset + (sector_size - 2);
        let start = self.offset + offset;
        file_ranges.push(range, start, self.mft_builder.clone());
        
        offset +=  sector_size - 2;

        let range = offset..offset + 2;
        let start =  self.offset + self.fixup_array_offset as u64 + 2 + (2 * (offset / sector_size));          
        file_ranges.push(range, start, self.mft_builder.clone());
        offset += 2;
      }
      else
      {
        //XXX check if ok 
        let range = offset..self.size() - offset;
        let start = self.offset + offset;
        file_ranges.push(range, start, self.mft_builder.clone());
        offset += self.size() - offset;
      }
    }

    Arc::new(MappedVFileBuilder::new(file_ranges))
  }
}
