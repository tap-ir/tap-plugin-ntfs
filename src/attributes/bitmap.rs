use std::sync::Arc;

use tap::vfile::VFileBuilder;

use anyhow::Result;


#[derive(Debug)]
pub struct Bitmap
{
}

impl Bitmap
{
  #[allow(clippy::needless_range_loop)]
  pub fn new(content : Arc<dyn VFileBuilder>) -> Result<Vec<std::ops::Range<u64>>>
  {
    let mut unallocated = Vec::new(); 
    let mut file = content.open()?;

    //check max size or read by chunk !
    let mut bitmap  = vec![0u8; content.size() as usize]; 
    file.read_exact(&mut bitmap)?;
   
    let mut cluster_start = 0;
    let mut cluster_end = 0;
    let mut current_cluster = 0;

    for idx in 0..bitmap.len()
    {
      let byte =  bitmap[idx];
      for i in 0..8
      {
        if (byte >> i) & 1 != 0
        {
          if cluster_start != 0
          {
            unallocated.push(cluster_start..cluster_end);
            cluster_start = 0;
            cluster_end = 0;
          }
        }
        else
        {
          if cluster_start == 0
          {
            cluster_start = current_cluster;
          }
          cluster_end = current_cluster;
        }
        current_cluster += 1;
      }
    }

    Ok(unallocated)
  }

}
