use std::sync::Arc;

use tap::vfile::VFileBuilder;
use tap::mappedvfile::{MappedVFileBuilder, FileRanges};

use crate::attributes::bitmap::Bitmap;


pub fn freespace_builder(builder : Arc<dyn VFileBuilder>, parent_builder : Arc<dyn VFileBuilder>, cluster_size : u64) -> Arc<dyn VFileBuilder>
{
  let bitmap = Bitmap::new(builder).unwrap();

  let mut current_offset = 0;
  let mut file_ranges = FileRanges::new();

  for cluster_range in bitmap.iter()
  {
    let offset = cluster_range.start*cluster_size;
    let size = (1 + cluster_range.end-cluster_range.start) * cluster_size;

    file_ranges.push(current_offset..current_offset + size, offset, parent_builder.clone()); 

    current_offset += size;
  }
 
  Arc::new(MappedVFileBuilder::new(file_ranges))
}


pub struct Unallocated
{
}

impl Unallocated
{
  pub fn new() -> Self
  {
    //create free space 
    Unallocated{}
  }
}

impl Default for Unallocated 
{
  fn default() -> Self 
  {
    Self::new()
  }
}
