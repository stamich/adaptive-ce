use ace_engine::AceEngine;
use ace_format::{checksum,encode_block_header,BlockHeader};
use ace_core::{CodecId,EntropyCodecId};

/// Builds a minimal format-1.0 RAW fixture without depending on the ACE 0.1 encoder binary.
fn format_1_0_raw_fixture(data:&[u8])->Vec<u8>{
    let mut file=[0u8;32]; file[0..4].copy_from_slice(b"ACE1"); file[4]=1; file[5]=0; file[8..12].copy_from_slice(&(262_144u32).to_le_bytes()); file[12..20].copy_from_slice(&(data.len() as u64).to_le_bytes()); file[20..28].copy_from_slice(&1u64.to_le_bytes()); let crc=checksum(&file[..28]); file[28..32].copy_from_slice(&crc.to_le_bytes());
    let header=BlockHeader{block_id:0,original_size:data.len() as u32,encoded_size:data.len() as u32,metadata_size:0,codec:CodecId::Raw,entropy:EntropyCodecId::None,transforms:Vec::new(),dictionary:None,flags:0,payload_crc32c:checksum(data)};
    let mut block=encode_block_header(&header); // descriptor layout is identical when dictionary flag is absent
    let mut out=file.to_vec(); out.append(&mut block); out.extend_from_slice(data); out
}

/// Ensures the ACE 0.2.1 sequential decoder remains compatible with format 1.0 RAW files.
#[test]
fn reads_format_1_0_fixture(){let data=b"ACE 0.1 compatibility fixture".repeat(1000);let fixture=format_1_0_raw_fixture(&data);assert_eq!(AceEngine::default_engine().decompress(&fixture).unwrap(),data);}
