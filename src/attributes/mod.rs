pub mod standard;
pub mod filename;
pub mod volume;
pub mod list;
pub mod bitmap;

bitflags! 
{
  pub struct FileAttributes : u32 
  {
    const READONLY             = 0x0000_0001;
    const HIDDEN               = 0x0000_0002;
    const SYSTEM               = 0x0000_0004;
    const DIRECTORY            = 0x0000_0010;
    const ARCHIVE              = 0x0000_0020;
    const DEVICE               = 0x0000_0040;
    const NORMAL               = 0x0000_0080;
    const TEMPORARY            = 0x0000_0100;
    const SPARSE               = 0x0000_0200;
    const REPARSE              = 0x0000_0400;
    const COMPRESSED           = 0x0000_0800;
    const OFFLINE              = 0x0000_1000;
    const NOT_INDEXED          = 0x0000_2000;
    const ENCRYPTED            = 0x0000_4000;
  }
}
