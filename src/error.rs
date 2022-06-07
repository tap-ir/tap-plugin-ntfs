use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum NtfsError
{
  #[error("MFT record size is 0")]
  MftRecordSize, 

  #[error("No partition provided to read non-resident attribute data")]
  NonResidentData,
  
  #[error("Boot sector as an invalid {0} value")]
  BootSectorInvalid(&'static str),

  #[error("MFT entry is unused")]
  MftUnusedEntry,

  #[error("MFT signature is invalid")]
  MftInvalidSignature,

  #[error("MFT Attribute {0} not found")]
  MftAttributeNotFound(&'static str),

  #[error("MFT attribute unknown type {0}")]
  MftAttributeUnknownType(u32),

  #[error("MFT attributes end")]
  MftAttributesEnd,

  #[error("MFT attribute unknown data type")]
  MftAttributeDataType,

  #[error("MFT Attribute FileName unknown name space {0}")]
  MftAttributeUnknownNameSpace(u8),

  #[error("MFT Attribute FileName name space size is invalid")]
  MftAttributeNameSpaceInvalidSize,

  #[error("MFT Attribute Standard Information size is invalid")]
  MftAttributeStandardInvalidSize,

  #[error("MFT Attribute List end")]
  MftAttributeListEnd,

  #[error("Resident attribute offset is large than MFT")]
  ResidentAttributeOffsetTooLarge,

  #[error("Resident attribute content size is larger than MFT")]
  ResidentAttributeContentTooLarge,

  #[error("Non resident attribute offset is larger than partition")]
  NonResidentAttributeOffsetTooLarge,

  #[error("Non resident attribute require a zero builder to read sparse attribute")]
  NonResidentAttributeZeroBuilder,

  #[error("Non resident attribute require cluster size to be read")]
  NonResidentAttributeClusterSize,
}
