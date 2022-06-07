use std::sync::Arc;
use std::io::SeekFrom;

use tap::vfile::{VFile, VFileBuilder};
use tap::reflect::{ReflectStruct};
use tap::value::Value;
use tap::mappedvfile::{MappedVFileBuilder,FileRanges};
use tap::node::Node;
use tap_derive::Reflect;
use tap::attribute::Attributes;

use anyhow::Result;
use byteorder::{ByteOrder, LittleEndian};

use crate::error::NtfsError;

#[derive(Debug, Reflect)]
pub struct BPB
{
  pub bytes_per_sector                  : u16,
  pub sector_per_cluster                : u8,
  pub media_descriptor                  : u8,
  pub total_sectors                     : u64, 
  pub mft_logical_cluster_number        : u64, 
  pub mft_mirror_logical_cluster_number : u64, 
  pub clusters_per_mft_record           : i8, 
  pub clusters_per_index_record         : i8,
  pub volume_serial_number              : u64, 
  pub checksum                          : u32, 
}

#[derive(Debug, Reflect)]
pub struct BootSector
{
  pub oem_id : u64,
  pub bpb : Arc<BPB>,
  pub end_of_sector : u16,
  pub cluster_size : u32,
  pub mft_record_size : u32,
  pub index_record_size : u32
  //end_of_sector : u16,
}

impl BootSector
{
  pub fn from_file<T : VFile>(file : &mut T) -> Result<BootSector>
  {
    file.seek(SeekFrom::Start(0))?;

    let mut data = [0;512]; 
    file.read_exact(&mut data)?;
    //first 3 u8 contain the jmp code
    let oem_id = LittleEndian::read_u64(&data[3..3+8]); //we read the OEMID
    let end_of_sector = LittleEndian::read_u16(&data[510..512]);
    if end_of_sector != 0xAA55
    {
      return Err(NtfsError::BootSectorInvalid("end of sector").into())
    };

    let bytes_per_sector = LittleEndian::read_u16(&data[0xb..0xd]);
    if bytes_per_sector == 0 || bytes_per_sector % 512 != 0
    {
      return Err(NtfsError::BootSectorInvalid("bytes per sector").into())
    }

    let sector_per_cluster = data[0xd];
    if sector_per_cluster == 0
    {
      return Err(NtfsError::BootSectorInvalid("sector per cluster").into())
    }

    let media_descriptor = data[0x15];
    let total_sectors = LittleEndian::read_u64(&data[0x28..0x30]);
    if total_sectors == 0
    {
      return Err(NtfsError::BootSectorInvalid("total sectors value").into())
    }
    let mft_logical_cluster_number = LittleEndian::read_u64(&data[0x30..0x38]);
    let mft_mirror_logical_cluster_number = LittleEndian::read_u64(&data[0x38..0x40]);
    if mft_logical_cluster_number > total_sectors && mft_mirror_logical_cluster_number > total_sectors
    {
      return Err(NtfsError::BootSectorInvalid("MFT logical cluster number").into())
    }
    let clusters_per_mft_record = data[0x40] as i8;
    if clusters_per_mft_record == 0
    {
      return Err(NtfsError::BootSectorInvalid("invalid cluster per MFT record").into())
    }
    let clusters_per_index_record = data[0x44] as i8;
    if clusters_per_index_record == 0
    {
      return Err(NtfsError::BootSectorInvalid("invalid cluster per index buffer").into())
    }
    let volume_serial_number = LittleEndian::read_u64(&data[0x48..0x48+8]);
    let checksum = LittleEndian::read_u32(&data[0x50..0x54]);

    let cluster_size : u32 = sector_per_cluster as u32 * bytes_per_sector as u32;
  
    let mft_record_size : u32 = if clusters_per_mft_record > 0 
    {
      clusters_per_mft_record as u32 * cluster_size
    }
    else
    {
       1 << (-clusters_per_mft_record) as i32
    };

    let index_record_size : u32 = if clusters_per_index_record > 0
    {
      clusters_per_index_record as u32 * cluster_size
    }
    else
    {
      1 << (-clusters_per_index_record) as i32
    };
 

    let bpb = BPB{
      bytes_per_sector,
      sector_per_cluster,
      media_descriptor,
      total_sectors, 
      mft_logical_cluster_number, 
      mft_mirror_logical_cluster_number, 
      clusters_per_mft_record, 
      clusters_per_index_record,
      volume_serial_number, 
      checksum,
    };

    Ok(BootSector{ 
      oem_id,
      bpb : Arc::new(bpb),
      end_of_sector,
      cluster_size,
      mft_record_size,
      index_record_size,
    })
  }

  pub fn size(&self) -> u64
  {
    self.bpb.bytes_per_sector as u64 * 16
  }

  pub fn to_builder(&self, builder : Arc<dyn VFileBuilder>) -> Arc<dyn VFileBuilder>
  {
    let mut file_ranges = FileRanges::new();

    let start = 0;
    let len = self.size();
    let range = 0 .. len; 
    file_ranges.push(range, start, builder);
    Arc::new(MappedVFileBuilder::new(file_ranges))
  }

  pub fn add_attribute(self, node : &Node, parent_builder : Arc<dyn VFileBuilder>)
  {
    let boot_sector_data = self.to_builder(parent_builder);
    let mut ntfs_attr = Attributes::new();
    ntfs_attr.add_attribute("bootsector", Arc::new(self), None);
    node.value().add_attribute("ntfs", ntfs_attr, None);
    node.value().remove_attribute("data"); //we remove any existing data attribute
    node.value().add_attribute("data", boot_sector_data, None);
    //avoid to recurse infinitely on a magic scan
    node.value().add_attribute("datatype", "ntfs/bootsector", None);
  }

  pub fn to_node(self, parent_builder : Arc<dyn VFileBuilder>) -> Node
  {
    let boot_sector_data = self.to_builder(parent_builder);
    let boot_sector_node = Node::new("$Boot");
    let mut ntfs_attr = Attributes::new();
    ntfs_attr.add_attribute("bootsector", Arc::new(self), None);
    boot_sector_node.value().add_attribute("ntfs", ntfs_attr, None);
    boot_sector_node.value().add_attribute("data", boot_sector_data, None);
    //avoid to recurse infinitely on a magic scan
    boot_sector_node.value().add_attribute("datatype", "ntfs/bootsector", None);

    boot_sector_node
  }
}
