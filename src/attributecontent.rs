use std::sync::Arc;
use std::io::SeekFrom;

use tap::vfile::{VFile, VFileBuilder};
use tap::mappedvfile::{MappedVFileBuilder,FileRanges};

use crate::attribute::{MftAttribute};
use crate::error::NtfsError;

use anyhow::Result;
use byteorder::{ByteOrder, LittleEndian};

#[inline]
pub fn pad_u64(data : &[u8]) -> u64
{
  let mut padded = [0; 8];

  padded[..data.len()].clone_from_slice(data);
  LittleEndian::read_u64(&padded[0..8])
}

#[inline]
pub fn pad_i64(data : &[u8]) -> i64
{
  let mut padded = match (data[data.len() - 1] as i8) < 0
  {
    true =>  [0xff; 8],
    false => [0; 8],
  };

  padded[..data.len()].clone_from_slice(data);
  LittleEndian::read_i64(&padded[0..8])
}

#[derive(Debug)]
pub struct MftAttributeContent
{
  pub offset : u32,
  pub mft_attribute : MftAttribute,
  pub mft_entry_builder : Arc<dyn VFileBuilder>,
  pub partition_builder: Option<Arc<dyn VFileBuilder>>,
  pub zero_builder: Option<Arc<dyn VFileBuilder>>,
  pub cluster_size : Option<u32>,
}

impl MftAttributeContent
{
  pub fn from_file<T : VFile>(file : &mut T, offset : u32, mft_entry_builder : Arc<dyn VFileBuilder>, partition_builder : &Option<Arc<dyn VFileBuilder>>, zero_builder : &Option<Arc<dyn VFileBuilder>>, cluster_size : Option<u32>) -> Result<Self>
  {
     let mft_attribute = MftAttribute::from_file(file, offset)?;
     let zero_builder = zero_builder.as_ref().cloned();

     match partition_builder
     {
       Some(partition_builder) => Ok(MftAttributeContent{offset, mft_attribute, mft_entry_builder, partition_builder : Some(partition_builder.clone()), zero_builder, cluster_size}),
       None => Ok(MftAttributeContent{offset, mft_attribute, mft_entry_builder, partition_builder : None, zero_builder, cluster_size}),
     }
  }

  pub fn builder(&self) -> Result<Arc<dyn VFileBuilder>> 
  {
    match &self.mft_attribute.data
    {
      ResidentType::Resident(resident) => Ok(self.resident_builder(resident)?),
      ResidentType::NonResident(non_resident) => 
        match &self.partition_builder
        {
           Some(partition_builder) =>  Ok(self.non_resident_builder(non_resident, partition_builder.clone())?),
           None => Err(NtfsError::NonResidentData{}.into()),
        }
    }
  }

  fn resident_builder(&self, resident : &Resident) -> Result<Arc<dyn VFileBuilder>>
  {
    let mut file_ranges = FileRanges::new();
  
    let offset = self.offset as u64 + resident.content_offset as u64;
    let content_size = resident.content_size as u64;
    let range = 0.. content_size;
    
    if offset > self.mft_entry_builder.size()
    {
      return Err(NtfsError::ResidentAttributeOffsetTooLarge.into())
    }
    if offset + content_size > self.mft_entry_builder.size()
    {
      return Err(NtfsError::ResidentAttributeContentTooLarge.into())
    }

    file_ranges.push(range, offset, self.mft_entry_builder.clone());
    Ok(Arc::new(MappedVFileBuilder::new(file_ranges)))
  }

  fn non_resident_builder(&self, non_resident : &NonResident, partition_builder : Arc<dyn VFileBuilder>) -> Result<Arc<dyn VFileBuilder>>
  {
    let zero_builder = match &self.zero_builder
    {
      Some(zero_builder) => zero_builder,
      None => return Err(NtfsError::NonResidentAttributeZeroBuilder.into()), 
    };

    let cluster_size = match self.cluster_size
    {
      Some(cluster_size) => cluster_size,
      None => return Err(NtfsError::NonResidentAttributeClusterSize.into()),
    };

    let mut file_ranges = FileRanges::new();
    let mut total_size : u64 = non_resident.vnc_start * cluster_size as u64;
    for run in non_resident.runs.iter()
    {
      if run.offset == 0 //sparse
      {
        let range = total_size..total_size + (run.length as u64 * cluster_size as u64);
        file_ranges.push(range, 0 , zero_builder.clone());
      }
      else
      {
        let run_offset = run.offset as u64;
        let run_length = run.length as u64;
        let cluster_size = cluster_size as u64;

        let range = total_size..total_size + (run_length * cluster_size);

        if run_offset * cluster_size > partition_builder.size()
        {
          return Err(NtfsError::NonResidentAttributeOffsetTooLarge.into()) 
        }
        //check if range is valid before pushing !
        file_ranges.push(range, run_offset * cluster_size, partition_builder.clone());
      }
      total_size += run.length * cluster_size as u64;
    }
    Ok(Arc::new(MappedVFileBuilder::new(file_ranges)))
  }
}

#[derive(Debug)]
pub enum ResidentType
{
  Resident(Resident),
  NonResident(NonResident),
}

/**
 *  Resident & Non Resident 
 */
#[derive(Debug)]
pub struct Resident
{
  pub content_size      : u32,
  pub content_offset    : u16,
}

impl Resident
{
  pub fn from_file<T : VFile>(file : &mut T) -> Result<Self>
  {
    let mut data = [0;6];
    file.read_exact(&mut data)?;
    
    let content_size = LittleEndian::read_u32(&data[0..4]);
    let content_offset = LittleEndian::read_u16(&data[4..6]);
    Ok(Resident{content_size, content_offset})
  }
}

#[derive(Debug)]
pub struct RunList
{
  pub offset : i64,
  pub length : u64,
}

#[derive(Debug)]
pub struct NonResident
{
  pub vnc_start                : u64,
  pub vnc_end                  : u64,
  pub run_list_offset          : u16, 
  pub compression_block_size   : u16, 
  pub unused                   : u32,
  pub content_allocated_size   : u64,
  pub content_actual_size      : u64,
  pub content_initialized_size : u64,
  pub runs                     : Vec<RunList>,
}

impl NonResident
{
  pub fn from_file<T : VFile>(file : &mut T, offset : u32) -> Result<Self>
  {
    let mut data = [0;48];
    file.read_exact(&mut data)?;

    let vnc_start = LittleEndian::read_u64(&data[0..8]);
    let vnc_end = LittleEndian::read_u64(&data[8..16]);
    let run_list_offset = LittleEndian::read_u16(&data[16..18]);
    let compression_block_size = LittleEndian::read_u16(&data[18..20]);
    let unused = LittleEndian::read_u32(&data[20..24]);
    let content_allocated_size = LittleEndian::read_u64(&data[24..32]);
    let content_actual_size = LittleEndian::read_u64(&data[32..40]);
    let content_initialized_size = LittleEndian::read_u64(&data[40..48]);

    
    file.seek(SeekFrom::Start(offset as u64 + run_list_offset as u64))?;
    let mut runs : Vec<RunList> = Vec::new();
    let mut run_previous_offset : i64 = 0;

    loop
    {
      let mut byte  = [0; 1];
      file.read_exact(&mut byte)?;

      //The first byte is split into two nibbles (4-bit values). The low-order bits tell you the number of bytes in the run length; the high-order bits tell you the numer of bytes in the offset to the run.
      let length_size = byte[0] & 0xf;
      let offset_size = byte[0] >> 4;
      if offset_size > 8
      {
        break
      }
      if length_size > 8
      {
        break
      }

      if length_size == 0
      {
        break
      }
    
      let mut run_length = vec![0; length_size as usize];
      file.read_exact(&mut run_length)?;
      let run_length = pad_u64(&run_length);

      let run_offset = match offset_size
      {
        0 => vec![0; 0],
        _ => {
          let mut run_offset = vec![0; offset_size as usize];
          file.read_exact(&mut run_offset)?;
          run_offset
        }
      };

      let run_offset = match run_offset.len()
      {
        0 => 0,
        _ => pad_i64(&run_offset),
      };

      if run_length == 0
      {
        break
      }
      run_previous_offset += run_offset;

      let run_list = match run_offset 
      {
        0 =>  RunList{offset : 0, length : run_length },
        _ => RunList{offset : run_previous_offset, length : run_length},
      };
      runs.push(run_list);
    }

    Ok(NonResident{
        vnc_start,
        vnc_end,
        run_list_offset,
        compression_block_size,
        unused,
        content_allocated_size,
        content_actual_size,
        content_initialized_size,
        runs,
    })
  }
}
