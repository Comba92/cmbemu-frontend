use crate::cpu::Reg;

#[derive(Default)]
pub struct Cop0 {
  // bpc: u32,  // breakpoint exception (debug) 
  // bda: u32,  // data breakpoint except. (debug) 
  // dcic: u32, // enable/disable hardware breakpoints (debug)
  // bdam: u32, 
  // bpcm: u32,
  pub sr: u32,
  pub cause: u32,
  pub epc: u32,
}
impl Cop0 {
  pub fn reg(&self, reg: Reg) -> u32 {
    match reg.0 {
      // 03 => self.bpc,
      // 05 => self.bda,
      // 07 => self.dcic,
      // 09 => self.bdam,
      // 11 => self.bpcm,
      12 => self.sr,
      13 => self.cause,
      14 => self.epc,
      // n => panic!("unhandled cop0 register {:08x}", n),
      _ => 0,
    }
  }

  pub fn set_reg(&mut self, reg: Reg, val: u32) {
    match reg.0 {
      // 03 => self.bpc = val,
      // 05 => self.bda = val,
      // 07 => self.dcic = val,
      // 09 => self.bdam = val,
      // 11 => self.bpcm = val,
      3 | 5 | 6 | 7 | 9 | 11 => {
        if val != 0 { panic!("unhandled cop0 register write {:08x}", reg.0) }
      }
      12 => self.sr = val,
      13 => self.cause = val,
      14 => self.epc = val,
      n => panic!("unhandled cop0 register write {:08x}", n),
    }
  }

  pub fn is_cache_isolated(&self) -> bool {
    (self.sr >> 16) & 1 == 1 
  }
  
  pub fn boot_expt_vector(&self) -> bool {
    (self.sr >> 22) & 1 == 1
  }
}

pub enum Exception {
  Interrupt = 0,
  IllegalLoad = 4,
  IllegalStore = 5,
  Syscall = 8,
  Break = 9,
  IllegalInstr = 10,
  CopError = 11,
  Overflow = 12,
}