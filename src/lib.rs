//! Ntfs plugin take a VFile attribute return from a node and  add the result of an ntfs function to the attribute of this node

#![allow(clippy::new_ret_no_self)]

#[macro_use]
extern crate num_derive;
#[macro_use]
extern crate bitflags;

pub mod ntfs;
pub mod bootsector;
pub mod mft;
pub mod mftentry;
pub mod attribute;
pub mod attributecontent;
pub mod attributes;
pub mod ntfsattributes;
pub mod unallocated;
pub mod error;

use std::fmt::Debug;

use tap::plugin;
use tap::config_schema;
use tap::node::Node;
use tap::error::RustructError;
use tap::tree::{TreeNodeId, TreeNodeIdSchema};
use tap::plugin::{PluginInfo, PluginInstance, PluginConfig, PluginArgument, PluginResult, PluginEnvironment};

use serde::{Serialize, Deserialize};
use anyhow::Result;
use schemars::JsonSchema;
use log::warn;

use crate::bootsector::BootSector;
use crate::ntfs::Ntfs;

plugin!("ntfs", "File system", "Read and parse NTFS filesystem", NtfsPlugin, Arguments);


#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Arguments
{
  #[schemars(with = "TreeNodeIdSchema")] 
  file : TreeNodeId,
  ///if set the module will try to recover files and folders by carving MFT in unallocated clusters
  recovery : Option<bool>,
}

#[derive(Debug, Serialize, Deserialize,Default)]
pub struct Results
{
}

#[derive(Default)]
pub struct NtfsPlugin
{
}

impl NtfsPlugin
{
  fn run(&mut self, args : Arguments, env : PluginEnvironment) -> Result<Results>
  {
    let file_node = env.tree.get_node_from_id(args.file).ok_or(RustructError::ArgumentNotFound("file"))?;
    file_node.value().add_attribute(self.name(), None, None); 
    let value = file_node.value().get_value("data").ok_or(RustructError::ValueNotFound("data"))?;
    let partition_builder = value.try_as_vfile_builder().ok_or(RustructError::ValueTypeMismatch)?;

    let mut file = partition_builder.open()?;
    let boot_sector = BootSector::from_file(&mut file)?;

    let mut ntfs = Ntfs::from_partition(partition_builder.clone(), &boot_sector)?;
    ntfs.create_nodes(&env.tree);
    let ntfs_node = Node::new("ntfs");
    let ntfs_node_id = env.tree.add_child(args.file, ntfs_node)?;
    let orphan_node = Node::new("orphan");
    let orphan_node_id = env.tree.add_child(ntfs_node_id, orphan_node)?;
    ntfs.link_nodes(&env.tree, ntfs_node_id, orphan_node_id);

    //Create freespace and recover MFT entries if options is set
    let freespace_builder = ntfs.freespace(&env.tree, ntfs_node_id, partition_builder.clone(), boot_sector.bpb.bytes_per_sector as u64); //cath error we can continue 
    if let Some(freespace_builder) = freespace_builder
    {
      let freespace_node = Node::new("freespace");
      freespace_node.value().add_attribute("data", freespace_builder, None);
      let _freespace_node_id = env.tree.add_child(ntfs_node_id, freespace_node)?;

      if let Some(true) = args.recovery
      { 
        warn!("recovering data by carving"); 
        ntfs.recovery(); 
      }
        //carve and add node to free space
        //let entries = ntfs.recovery()
        //for each entry link to unallocated /freespace /tree ? 
    }

    //Add attribute of our parsed bootsector to $Boot
    if let Some(boot_node_id) = env.tree.find_node_from_id(ntfs_node_id, "/root/$Boot")
    {
      let boot_node = env.tree.get_node_from_id(boot_node_id).unwrap();
      boot_sector.add_attribute(&boot_node, partition_builder);
    }

    //Add our parsed $MFT with attribute to the tree 
    if let Some(root) = env.tree.find_node_from_id(ntfs_node_id, "/root")
    {
      if let Some(mft_ntfs_node) = ntfs.mft_node() 
      {
        let node = mft_ntfs_node.to_node();
        //avoid to recurse infinitely on a magic scan
        node.value().add_attribute("datatype", "ntfs/mft", None);
        env.tree.add_child(root, node)?;
      }
    }

    if let Some(mft_mirror) = env.tree.find_node_from_id(ntfs_node_id, "/root/$MFTMirr")
    {
      let mft_mirror_node = env.tree.get_node_from_id(mft_mirror).unwrap();
      mft_mirror_node.value().add_attribute("datatype", "ntfs/mft", None);
    }

    Ok(Results{})
  }
}
