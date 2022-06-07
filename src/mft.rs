use std::sync::Arc;

use tap::vfile::VFileBuilder;
use tap::zerovfile::ZeroVFileBuilder;
use tap::memoryvfile::MemoryVFileBuilder;

use crate::mftentry::MftEntry;
use crate::error::NtfsError;
use crate::ntfs::NtfsNode;

use anyhow::Result;

/**
 *  MftEntries
 *  This can be used to get the different MftEntry 
 */

#[derive(Debug)]
pub struct MftEntries
{
  partition_builder : Option<Arc<dyn VFileBuilder>>, //parent builder == fs
  zero_builder : Option<Arc<dyn VFileBuilder>>, //use for sparse non-resident 
  mft_record_size : u32,
  sector_size : u16,
  cluster_size : Option<u32>, //use for non-resident fixup size
  master_mft_builder : Arc<dyn VFileBuilder>,
  number_of_entry : u64,
  master_mft_entry : Option<MftEntry>,
}

impl MftEntries 
{
  pub fn from_partition(partition_builder : Arc<dyn VFileBuilder>,  mft_logical_cluster_number : u64, cluster_size : u32, sector_size : u16, mft_record_size : u32) -> Result<MftEntries>
  {
    //check value bound
    if mft_record_size == 0
    {
      return Err(NtfsError::MftRecordSize{}.into())
    }

    

    let master_mft_offset = mft_logical_cluster_number * cluster_size as u64;
    let zero_builder = Arc::new(ZeroVFileBuilder{});

    let master_mft_entry = MftEntry::from_offset(master_mft_offset, Some(partition_builder.clone()), partition_builder.clone(), Some(zero_builder.clone()), mft_record_size, sector_size, Some(cluster_size))?;
    let master_mft_builder = master_mft_entry.data_attribute()?;
    let master_mft_builder = MemoryVFileBuilder::new(master_mft_builder.clone())?;//Use in memory cache of MFT

    let number_of_entry = master_mft_builder.size() / mft_record_size as u64;

    Ok(MftEntries{
      partition_builder : Some(partition_builder),
      zero_builder : Some(zero_builder), //used only for non-resident
      mft_record_size,
      cluster_size : Some(cluster_size), //used only for non-resident
      sector_size, 
      master_mft_builder,
      number_of_entry,
      master_mft_entry : Some(master_mft_entry),
    })
  }

  pub fn from_master_mft(master_mft_builder : Arc<dyn VFileBuilder>, sector_size : Option<u16>, mft_record_size : Option<u32>) -> Result<MftEntries>
  {
    let master_mft_builder = MemoryVFileBuilder::new(master_mft_builder.clone())?;//Use in memory cache of MFT

    let sector_size = sector_size.unwrap_or(512);

    let mft_record_size = match mft_record_size
    {
      Some(mft_record_size) => mft_record_size,
      None =>  MftEntry::from_offset(0, None, master_mft_builder.clone(), Some(Arc::new(ZeroVFileBuilder{})), 4096, sector_size, Some(4096))?.allocated_size,
    };

    let master_mft_builder_size = master_mft_builder.size();

    match mft_record_size
    {
      0 => Err(NtfsError::MftRecordSize.into()),
      _ => Ok(MftEntries{
        partition_builder : None,
        zero_builder : None,
        mft_record_size,
        cluster_size : None,
        sector_size,  
        master_mft_builder,
        number_of_entry : master_mft_builder_size / mft_record_size as u64,
        master_mft_entry : None,
      })
    }
  }

  pub fn count(&self) -> u64
  {
    self.number_of_entry
  }

  pub fn master_mft(&self) -> Option<NtfsNode> 
  {
    let mut node = match &self.master_mft_entry
    {
      Some(master_mft_entry) => NtfsNode::from_entry(0, master_mft_entry, self),
      None => return None,
    };

    if !node.is_empty()
    {
      return Some(node.remove(0))
    }
    None
  }

  //create an iterator XXX 
  pub fn entry(&self, entry_id : u64) -> Result<MftEntry> 
  {
    MftEntry::from_offset(entry_id * self.mft_record_size as u64, self.partition_builder.clone(), self.master_mft_builder.clone(), self.zero_builder.clone(), self.mft_record_size, self.sector_size, self.cluster_size)
  }
}
