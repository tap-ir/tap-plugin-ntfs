use std::sync::Arc;
use std::fmt::Debug;
use std::collections::HashMap;

use tap::tree::{Tree, TreeNodeId};
use tap::node::Node;
use tap::vfile::VFileBuilder;
use tap::reflect::ReflectStruct;
use tap::value::Value;
use tap_derive::Reflect;

use log::warn;
use anyhow::Result;

use crate::bootsector::BootSector;
use crate::mft::MftEntries;
use crate::mftentry::{MftEntry};
use crate::unallocated::freespace_builder;
use crate::attributes::standard::StandardInformation;
use crate::attributes::filename::{FileName};

/**
 *   Ntfs parser
 */
pub struct Ntfs
{
  mft_entries : MftEntries,
  nodes_ids : HashMap::<u64, Vec<(Option<u64>, TreeNodeId)>>,
}

impl Ntfs
{
  pub fn from_partition(partition_builder : Arc<dyn VFileBuilder>, boot_sector : &BootSector) -> Result<Ntfs>
  {
    //we create a builder from the main MFT so we can read attributes
    let mft_entries = MftEntries::from_partition(partition_builder,
                                               boot_sector.bpb.mft_logical_cluster_number,
                                               boot_sector.cluster_size,
                                               boot_sector.bpb.bytes_per_sector,
                                               boot_sector.mft_record_size)?;

    Ok(Ntfs{mft_entries, nodes_ids : HashMap::new()})
  }

  pub fn mft_node(&self) -> Option<NtfsNode>
  {
    self.mft_entries.master_mft()
  }

  pub fn from_mft(master_mft_builder : Arc<dyn VFileBuilder>, sector_size : Option<u16>, mft_record_size : Option<u32>) -> Result<Ntfs>
  {
    let mft_entries = MftEntries::from_master_mft(master_mft_builder, sector_size, mft_record_size)?;
    Ok(Ntfs{mft_entries, nodes_ids : HashMap::new()})
  }

  pub fn create_nodes(&mut self, tree : &Tree)
  {
    //here we read each entry in the mft
    //we could use par_iter to multithread that 
    let entry_count = self.mft_entries.count();
    //we start from 1 as 0 is the $MFT and we already parsed it, 1 is $MFTMirror
    for i in 1..entry_count
    {
      if i % 10000 == 0 { warn!("entry {}/{}", i, entry_count); }

      let entry = match self.mft_entries.entry(i)
      {
        Ok(entry) => entry,
        Err(err) => { warn!("Can't read mft entry {} : {}", i, err); continue }
      };

      let ntfs_nodes = NtfsNode::from_entry(i, &entry, &self.mft_entries);

      for ntfs_node in ntfs_nodes.into_iter()  //we can return multiple nodes because of ADS 
      {
        let parent_id = ntfs_node.attributes.file_name.as_ref().map(|file_name| file_name.parent_mft_entry_id);

        let tree_node = ntfs_node.to_node();
        let tree_node_id = tree.new_node(tree_node);
        match parent_id
        {
          Some(parent_id) if parent_id != i => 
          {
            //we check for loop
            match self.nodes_ids.get_mut(&i)
            {
              Some(nodes) => {nodes.push((Some(parent_id), tree_node_id)); } ,
              None => {self.nodes_ids.insert(i, vec![(Some(parent_id), tree_node_id)]);} ,
            }
          },
          _ => match self.nodes_ids.get_mut(&i)
          {
            Some(nodes) => { nodes.push((None, tree_node_id)); },
            None => { self.nodes_ids.insert(i, vec![(None, tree_node_id)]); },
          },
        }
      }
    }
  }

  pub fn link_nodes(&self, tree : &Tree, ntfs_node_id : TreeNodeId, orphan_node_id : TreeNodeId) 
  {
    warn!("Linking tree");
    let mut i = 0;
    let valid_entry_count = self.nodes_ids.len();

    for (id, nodes) in &self.nodes_ids
    {
      if i % 10_000 == 0 { warn!("linking {}/{}", i, valid_entry_count); }
      for (parent_id, tree_node_id) in nodes
      {
        //root node is a special case as it link to itself but we want to add it to our root
        //we should maybe create a fake root if it doesn't exist to avoid having everything as
        //orphan
        if *id == 5 
        {
          tree.add_child_from_id(ntfs_node_id, nodes[0].1);
          continue
        }

        //check if node as a parent id to link to
        let parent_id = match parent_id
        {
          Some(parent_id) => parent_id,
          None => { tree.add_child_from_id(orphan_node_id, *tree_node_id); continue; }
        };

        //link node to it's parent
        match self.nodes_ids.get(parent_id)
        {
          //we check if we have a parent node and avoid loop by checking if parent_id != node_id
          Some(parent_nodes) if !parent_nodes.is_empty() && parent_nodes[0].1 != *tree_node_id =>
          { 
            tree.add_child_from_id(parent_nodes[0].1, *tree_node_id);
          },
          //if parent didn't exist we add node as orphan
          _ => tree.add_child_from_id(orphan_node_id, *tree_node_id),
        }
      }
      i += 1;
    }
  }

  pub fn freespace(&self, tree : &Tree, ntfs_node_id : TreeNodeId, partition_builder : Arc<dyn VFileBuilder>, cluster_size : u64) -> Option<Arc<dyn VFileBuilder>>
  {
    tree.find_node_from_id(ntfs_node_id, "/root/$Bitmap")
        .and_then(|node_id| tree.get_node_from_id(node_id))
        .and_then(|node| node.value().get_value("data"))
        .and_then(|value| value.try_as_vfile_builder())
        .map(|bitmap| freespace_builder(bitmap, partition_builder, cluster_size))
  }

  pub fn recovery(&self) 
  {

  }
}

fn option_to_value<T>(value : &Option<Arc<T>>) -> Option<Value>
 where T : ReflectStruct + Sync + Send + 'static
{
  value.as_ref().map(|value| Value::ReflectStruct(value.clone()))
}

#[derive(Debug, Reflect, Clone)]
pub struct NtfsNodeAttribute
{
  #[reflect(with = "option_to_value")]
  standard_information : Option<Arc<StandardInformation>>,
  #[reflect(with = "option_to_value")]
  file_name : Option<Arc<FileName>>,
  is_deleted : bool,
}

pub struct NtfsNode
{
  pub name : String,
  pub attributes : NtfsNodeAttribute,
  pub data  : Option<Arc<dyn VFileBuilder>>,
}

impl NtfsNode
{
  pub fn from_entry(entry_id : u64, entry : &MftEntry, entries : &MftEntries) -> Vec<NtfsNode>
  {
    let is_deleted = !entry.is_used();
    let attributes = entry.read_attributes(Some(entries)); //attribute list need to read other entries

    let datas = attributes.find_datas();
    let standard_information = attributes.find_standard_info().into_iter().next().map(Arc::new);

    let (name, file_name) = match entry_id
    {
      5 => ("root".into(), None),
      _ => match attributes.find_filename()
      {
        Some(file_name) => { (file_name.file_name.clone(), Some(Arc::new(file_name))) },
        None => (format!("Unknown_{}", entry_id), None),
      },
    };

    let attributes = NtfsNodeAttribute{ 
      standard_information,
      file_name,
      is_deleted,
    };

    if datas.is_empty()
    {
      return vec![NtfsNode{name, attributes, data : None}] 
    }
    
    let mut nodes = Vec::new();

    for data in datas.iter()
    {
      //happen when we read from MFT as we don't handle non-resident attribute
      let builder = data.builder().ok();
      let stream_name = match &data.mft_attribute.name
      {
        Some(data_name) => format!("{}:{}", name, data_name),
        None => name.clone(),
      };

      nodes.push(NtfsNode{name : stream_name, attributes : attributes.clone(), data : builder }); 
    }
      
    nodes
  }

  pub fn to_node(self) -> Node
  {
    let node = Node::new(self.name);
    node.value().add_attribute("ntfs", Arc::new(self.attributes), None);
    if let Some(data) = self.data 
    {
      node.value().add_attribute("data", data, None);
    }
    node
  }
}
