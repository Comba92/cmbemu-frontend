use std::{fs, io::{self, Read}};

fn read8(data: &[u8], offset: u32) -> u32 {
  let offset = offset as usize;
  data[offset] as u32
}

fn write8(data: &mut[u8], offset: u32, val: u32) {
  let offset = offset as usize;
  data[offset] = val as u8;
}

fn read16(data: &[u8], offset: u32) -> u32 {
  let offset = offset as usize;
  let bytes = data[offset..offset+2]
    .try_into().unwrap();
  u16::from_le_bytes(bytes) as u32
}

fn write16(data: &mut[u8], offset: u32, val: u32) {
  let offset = offset as usize;
  let bytes = (val as u16).to_le_bytes();
  data[offset..offset+2].copy_from_slice(&bytes);
}

fn read32(data: &[u8], offset: u32) -> u32 {
  let offset = offset as usize;
  let bytes = data[offset..offset+4]
    .try_into().unwrap();
  u32::from_le_bytes(bytes)
}

fn write32(data: &mut[u8], offset: u32, val: u32) {
  let offset = offset as usize;
  let bytes = val.to_le_bytes();
  data[offset..offset+4].copy_from_slice(&bytes);
}

pub struct MemRange {
  pub start: u32,
  pub length: u32,
  pub end: u32,
}
impl MemRange {
  pub const fn new(start: u32, length: u32) -> Self {
    Self {
      start,
      length,
      end: start + length,
    }
  }

  pub fn contains(&self, addr: u32) -> Option<u32> {
    if (self.start..self.end).contains(&addr) {
      Some(addr - self.start)
    } else {
      None
    }
  }
}
enum Target {
  Ram,
  Exp1,
  Scratchpad,
  MemCtrl1,
  MemCtrl2,
  IrqCtrl,
  Dma,
  Timers,
  CDRom,
  Gpu,
  Mdec,
  Spu,
  Exp2,
  Exp3,
  Bios,
  CacheCtrl,
}

pub struct Bios {
  data: Vec<u8>, 
}
impl Bios {
  pub fn new(path: &str) -> Result<Self, io::Error> {
    let mut file = fs::File::open(path)?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;

    if data.len() == Mmu::BIOS.length as usize {
      Ok(Self {data})
    } else {
      Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "Invalid BIOS size")
      )
    }
  }
}


pub struct Mmu {
  bios: Bios,
  pub ram: Box<[u8]>,
}

impl Mmu {
  const EXP1:     MemRange = MemRange::new(0x1f00_0000, 8192*1024);
  pub const BIOS: MemRange = MemRange::new(0x1fc0_0000, 512*1024);
  const SYS_CTRL: MemRange = MemRange::new(0x1f80_1000, 36);
  const RAM_CTRL: MemRange = MemRange::new(0x1f80_1060, 4);
  const IRQ_CTRL: MemRange = MemRange::new(0x1f80_1070, 8);
  const DMA: MemRange    = MemRange::new(0x1f80_1080, 128);
  const TIMERS: MemRange = MemRange::new(0x1f80_1100, 48);
  const SPU:    MemRange = MemRange::new(0x1f80_1c00, 640);
  const EXP2:   MemRange = MemRange::new(0x1f80_2000, 66);
  const EXP3:   MemRange = MemRange::new(0x1fa0_0000, 2048*1024);
  
  const RAM: MemRange = MemRange::new(0, 2048*1024);
  const GPU: MemRange = MemRange::new(0x1f801810, 8);
  const CACHE_CTRL: MemRange = MemRange::new(0xfffe_0130, 4);

  const REGION_MASK: [u32; 8] = [
    // KUSEG: 2GB
    0xffff_ffff, 0xffff_ffff, 0xffff_ffff, 0xffff_ffff,
    // KSEG0: 512MB | we mask 0x8000_0000..0x9fff_ffff down
    0x7fff_ffff,
    // KSEG1: 512MB | we mask 0xa000_0000..0bfff_ffff down
    0x1fff_ffff,
    // KSEG2: 1GB
    0xffff_ffff, 0xffff_ffff
  ];

  pub fn new(bios: Bios) -> Self {
    Self { bios, ram: vec![0xca; 2048*1024].into_boxed_slice() }
  }

  fn mask_region(addr: u32) -> u32 {
    let index = (addr >> 29) as usize;
    addr & Self::REGION_MASK[index]
  }

  pub fn read32(&self, addr: u32) -> u32 {
    self.read::<4, _>(addr, read32)
  }
  pub fn read16(&self, addr: u32) -> u32 {
    self.read::<2, _>(addr, read16)
  }
  pub fn read8(&self, addr: u32) -> u32 {
    self.read::<1, _>(addr, read8)
  }
  pub fn write32(&mut self, addr: u32, val: u32) {
    self.write::<4, _>(addr, val, write32);
  }
  pub fn write16(&mut self, addr: u32, val: u32) {
    self.write::<2, _>(addr, val, write16);
  }
  pub fn write8(&mut self, addr: u32, val: u32) {
    self.write::<1, _>(addr, val, write8);
  }

  fn read<const SIZE: u32, Accessor: FnOnce(&[u8], u32) -> u32>(&self, addr: u32, access: Accessor) -> u32 {
    assert!(addr % SIZE == 0, "unaligned memory read at {:08x}", addr);

    let addr = Self::mask_region(addr);
    
    if let Some(offset) = Self::BIOS.contains(addr) {
      access(&self.bios.data, offset)
    } else if let Some(offset) = Self::RAM.contains(addr) {
      access(&self.ram, offset % (2048*1024))
    } else if let Some(offset) = Self::EXP1.contains(addr) {
      eprintln!("unhandled read to EXP1: {:08x}", offset);
      0xff
    } else if let Some(offset) = Self::IRQ_CTRL.contains(addr) {
      eprintln!("unhandled write to IRQ_CTRL: {:08x}", offset);
      0
    } else if let Some(offset) = Self::DMA.contains(addr) {
      eprintln!("unhandled write to DMA: {:08x}", offset);
      0
    } else if let Some(offset) = Self::SPU.contains(addr) {
      eprintln!("unhandled write to SPU: {:08x}", offset);
      0
    } else if let Some(offset) = Self::GPU.contains(addr) {
      eprintln!("unhandled write to GPU: {:08x}", offset);
      0
    } else {
      // panic!("unhandled address range read: {:08x}", addr)
      0
    }
  }

  fn write<const SIZE: u32, Accessor: FnOnce(&mut [u8], u32, u32)>(&mut self, addr: u32, val: u32, access: Accessor) {
    assert!(addr % SIZE == 0, "unaligned memory write at {:08x}", addr);

    let addr = Self::mask_region(addr);

    if let Some(offset) = Self::RAM.contains(addr) {
      access(&mut self.ram, offset % (2048*1024), val);
    } else if let Some(offset) = Self::SYS_CTRL.contains(addr) {
      eprintln!("unhandled write to MEM_CTRL {:08x}", offset);
    } else if let Some(offset) = Self::RAM_CTRL.contains(addr) {
      eprintln!("unhandled write to RAM_CTRL {:08x}", offset)
    } else if let Some(offset) = Self::CACHE_CTRL.contains(addr) {
      eprintln!("unhandled write to CACHE_CTRL {:08x}", offset)
    } else if let Some(offset) = Self::SPU.contains(addr) {
      eprintln!("unhandled write to SPU {:08x}", offset)
    } else if let Some(offset) = Self::EXP2.contains(addr) {
      eprintln!("unhandled write to EXP2 {:08x}", offset)
    } else if let Some(offset) = Self::IRQ_CTRL.contains(addr) {
      eprintln!("unhandled write to IRQ_CTRL: {:08x}", offset);
    } else if let Some(offset) = Self::TIMERS.contains(addr) {
      eprintln!("unhandled write to TIMERS: {:08x}", offset);
    } else if let Some(offset) = Self::DMA.contains(addr) {
      eprintln!("unhandled write to DMA: {:08x}", offset);
    } else if let Some(offset) = Self::GPU.contains(addr) {
      eprintln!("unhandled write to GPU: {:08x}", offset);
    } else {
      // panic!("unhandled address range write: {:08x} {:x}", addr, val);
    }
  }
}