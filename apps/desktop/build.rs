//! Embeds the official Swaggen multi-resolution Windows icon (assets/swaggen.ico)
//! directly into swaggen-desktop.exe via a standard Win32 .res resource file.
use std::{env, fs, path::PathBuf};

fn write_res_entry(out: &mut Vec<u8>, res_type: u16, res_id: u16, payload: &[u8]) {
    let data_size = payload.len() as u32;
    let header_size: u32 = 32;
    out.extend_from_slice(&data_size.to_le_bytes());
    out.extend_from_slice(&header_size.to_le_bytes());
    // Integer Type: 0xFFFF, res_type
    out.extend_from_slice(&0xFFFFu16.to_le_bytes());
    out.extend_from_slice(&res_type.to_le_bytes());
    // Integer Name: 0xFFFF, res_id
    out.extend_from_slice(&0xFFFFu16.to_le_bytes());
    out.extend_from_slice(&res_id.to_le_bytes());
    // DataVersion (4), MemoryFlags (2), LanguageId (2), Version (4), Characteristics (4)
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0x1030u16.to_le_bytes());
    out.extend_from_slice(&0x0409u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(payload);
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

fn main() {
    println!("cargo:rerun-if-changed=../../assets/swaggen.ico");
    println!("cargo:rerun-if-changed=../../assets/swaggen-icon-256.png");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let ico_path = manifest_dir
        .join("..")
        .join("..")
        .join("assets")
        .join("swaggen.ico");
    let ico = fs::read(&ico_path).expect("assets/swaggen.ico must exist");
    assert!(ico.len() >= 6, "invalid ICO header");
    let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
    assert!(
        count > 0 && ico.len() >= 6 + count * 16,
        "truncated ICO directory"
    );

    let mut res = Vec::with_capacity(ico.len() + 512);
    // Standard 32-byte Win32 .res empty header record
    res.extend_from_slice(&[
        0, 0, 0, 0, 0x20, 0, 0, 0, 0xFF, 0xFF, 0, 0, 0xFF, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0,
    ]);

    let mut group_dir = Vec::with_capacity(6 + count * 14);
    group_dir.extend_from_slice(&ico[0..6]);

    for i in 0..count {
        let entry_off = 6 + i * 16;
        let bytes_in_res = u32::from_le_bytes([
            ico[entry_off + 8],
            ico[entry_off + 9],
            ico[entry_off + 10],
            ico[entry_off + 11],
        ]) as usize;
        let img_offset = u32::from_le_bytes([
            ico[entry_off + 12],
            ico[entry_off + 13],
            ico[entry_off + 14],
            ico[entry_off + 15],
        ]) as usize;
        let img_data = &ico[img_offset..img_offset + bytes_in_res];
        let icon_id = (i + 1) as u16;
        // RT_ICON = 3
        write_res_entry(&mut res, 3, icon_id, img_data);
        // GRPICONDIRENTRY = first 12 bytes of ICONDIRENTRY + u16 icon_id
        group_dir.extend_from_slice(&ico[entry_off..entry_off + 12]);
        group_dir.extend_from_slice(&icon_id.to_le_bytes());
    }

    // RT_GROUP_ICON = 14, ID = 1
    write_res_entry(&mut res, 14, 1, &group_dir);

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let res_path = out_dir.join("swaggen-icon.res");
    fs::write(&res_path, &res).expect("failed writing swaggen-icon.res");
    println!(
        "cargo:rustc-link-arg-bin=swaggen-desktop={}",
        res_path.display()
    );
}
