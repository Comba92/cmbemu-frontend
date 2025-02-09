use core::panic;
use std::{collections::VecDeque, fmt::Debug};
use crate::{cop0::{Cop0, Exception}, mmu::Mmu};

const OPCODES_SPEC: [(u32, &'static str); 29] = [
  (0b000_000, "sll"),
  (0b000_010, "srl"),
  (0b000_011, "sra"),
  (0b000_100, "sllv"),
  (0b001_000, "jr"),
  (0b001_001, "jalr"),
  (0b001_100, "syscall"),
  (0b001_101, "break"),
  (0b011_000, "mult"),
  (0b011_001, "multu"),
  (0b011_010, "div"),
  (0b011_011, "divu"),
  (0b010_000, "mfhi"),
  (0b010_001, "mthi"),
  (0b010_010, "mflo"),
  (0b010_011, "mtlo"),
  (0b100_000, "add"),
  (0b100_001, "addu"),
  (0b100_010, "sub"),
  (0b100_011, "subu"),
  (0b100_100, "and"),
  (0b100_101, "or"),
  (0b101_010, "slt"),
  (0b101_011, "sltu"),
  (0b100_111, "nor"),
  (0b100_110, "xor"),
  (0b000_100, "sllv"),
  (0b000_110, "srlv"),
  (0b000_111, "srav"),
];

const OPCODES: [(u32, &'static str); 29] = [
  (0b000_000, "special"),
  (0b000_001, "bxxx"),
  (0b010_000, "cop0"),
  (0b000_010, "jump"),
  (0b000_011, "jal"),
  (0b000_100, "beq"),
  (0b000_101, "bne"),
  (0b000_110, "blez"),
  (0b000_111, "bgtz"),
  (0b001_000, "addi"),
  (0b001_001, "addiu"),
  (0b001_010, "slti"),
  (0b001_011, "sltiu"),
  (0b001_100, "andi"),
  (0b001_101, "ori"),
  (0b001_110, "xori"),
  (0b001_111, "lui"),
  (0b100_000, "lb"),
  (0b100_100, "lbu"),
  (0b100_101, "lhu"),
  (0b100_001, "lh"),
  (0b100_011, "lw"),
  (0b101_000, "sb"),
  (0b101_001, "sh"),
  (0b101_011, "sw"),
  (0x22, "lwl"),
  (0x26, "lwr"),
  (0x2a, "swl"),
  (0x2e, "swr"),
];

#[derive(Clone, Copy)]
struct Instr(u32);
impl Instr {
  fn name(&self) -> &str {
    OPCODES.iter()
    .find(|op| op.0 == self.opcode())
    .map(|op| op.1)
    .expect(&format!("unhandled instruction {:b}", self.opcode()))
  }

  fn name_spec(&self) -> &str {
    OPCODES_SPEC.iter()
    .find(|op| op.0 == self.funct())
    .map(|op| op.1)
    .expect(&format!("unhandled special instruction {:b}", self.funct()))
  }

  fn opcode(&self) -> u32 {
    (self.0 >> 26) & 0b11_1111
  }

  fn rs(&self) -> Reg {
    Reg((self.0 >> 21) & 0b1_1111)
  }

  fn rt(&self) -> Reg {
    Reg((self.0 >> 16) & 0b1_1111)
  }

  fn rd(&self) -> Reg {
    Reg((self.0 >> 11) & 0b1_1111)
  }

  fn shift(&self) -> u32 {
    (self.0 >> 6) & 0b1_1111
  }

  fn funct(&self) -> u32 {
    self.0 & 0b11_1111
  }

  fn imm16(&self) -> u32 {
    self.0 & 0xffff
  }

  fn imm16sign(&self) -> u32 {
    (self.imm16() as i16) as u32
  }

  fn imm26(&self) -> u32 {
    self.0 & 0x03ff_ffff
  }

  fn offset16sign(&self) -> u32 {
    self.imm16sign() << 2
  }

  fn offset26(&self) -> u32 {
    self.imm26() << 2
  }
}

#[derive(PartialEq)]
pub struct Reg(pub u32);

pub struct Cpu {
  regs: [u32; 32],
  hi: u32,
  lo: u32,
  pc: u32,
  mmu: Mmu,
  
  i: Instr,
  curr_pc: u32,
  // needed for the branch delay slot
  next_pc: u32,
  in_delay_slot: bool,

  // needed for the load delay slots
  ld_delay_slots: VecDeque<(Reg, u32)>,
  
  cop0: Cop0,
}

impl Debug for Cpu {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Cpu").field("regs", &self.regs).field("hi", &self.hi).field("lo", &self.lo).field("pc", &self.pc).finish()
  }
}

impl Cpu {
  pub fn new(mmu: Mmu) -> Self {
    let mut regs = [0xdead_beef; 32];
    regs[0] = 0;

    let pc = Mmu::BIOS.start;
    Self {
      regs,
      hi: 0, lo: 0,
      pc,
      i: Instr(0),
      curr_pc: pc,
      next_pc: pc + 4,
      ld_delay_slots: VecDeque::new(),
      in_delay_slot: false,
      mmu,
      cop0: Default::default(),
    }
  }

  fn tty_output(&self) {
    let pc = self.pc & 0x1FFFFFFF;
    if (pc == 0xA0 && self.regs[9] == 0x3C) || (pc == 0xB0 && self.regs[9] == 0x3D) {
        // "as u8 as char" is the incantation to make Rust interpret the lowest byte of
        // a u32 value as an ASCII character
        let ch = self.regs[4] as u8 as char;
        print!("{ch}");
    }
  }

  pub fn sideload_exe(&mut self, exe: &[u8]) {
    // wait for the bios to jump to the shell
    while self.pc != 0x8003_0000 {
      self.step();
    }

    let initial_pc   = u32::from_le_bytes(exe[0x10..0x14].try_into().unwrap());
    let initial_r28  = u32::from_le_bytes(exe[0x14..0x18].try_into().unwrap());
    let exe_ram_addr = u32::from_le_bytes(exe[0x18..0x1C].try_into().unwrap()) & 0x001F_FFFF;
    let exe_size = u32::from_le_bytes(exe[0x1C..0x20].try_into().unwrap());
    let initial_sp   = u32::from_le_bytes(exe[0x30..0x34].try_into().unwrap());
  
    println!("Exe start: {exe_ram_addr}");
    println!("Exe size: {exe_size}");
    println!("Exe actual size: {}", exe.len());

    self.mmu.ram[exe_ram_addr as usize .. (exe_ram_addr + exe_size) as usize]
      .copy_from_slice(&exe[2048..2048 + exe_size as usize]);

    self.set_reg(Reg(28), initial_r28);

    if initial_sp != 0 {
      self.set_reg(Reg(29), initial_sp);
      self.set_reg(Reg(30), initial_sp);
    }

    self.pc = initial_pc;
    self.next_pc = self.pc + 4;

    println!("Exe sideloaded!");
  }

  fn reg(&self, reg: Reg) -> u32 {
    self.regs[reg.0 as usize]
  }

  fn rs_val(&self) -> u32 {
    self.reg(self.i.rs()) 
  }

  fn rt_val(&self) -> u32 {
    self.reg(self.i.rt())
  }

  fn set_reg(&mut self, reg: Reg, res: u32) {
    self.regs[reg.0 as usize] = res;
    self.regs[0] = 0;
  }

  pub fn step(&mut self) {
    self.tty_output();
    
    let ld_delay = self.ld_delay_slots.pop_front();
    if let Some((reg, val)) = ld_delay {
      self.set_reg(reg, val);
    }
    
    self.curr_pc = self.pc;
    self.pc = self.next_pc;
    self.next_pc = self.next_pc.wrapping_add(4);

    if self.curr_pc % 4 != 0 {
      self.exception(Exception::IllegalLoad);
      return;
    }
    
    self.i = Instr(self.mmu.read32(self.curr_pc));

    let was_delay_slot = self.in_delay_slot;
    self.decode();
    
    if was_delay_slot {
      self.in_delay_slot = false;
    }
  }

  fn decode(&mut self) {
    // print!("Instr: {}", i.name());
    // if i.opcode() == 0 {
    //   eprintln!("{}", i.name_spec());
    // } else {
    //   eprintln!()
    // }

    match self.i.opcode() {
      0x00 => {
        match self.i.funct() {
        0b000_000 => self.sll(),
        0b000_010 => self.srl(),
        0b000_011 => self.sra(),
        0b000_100 => self.sllv(),
        0b001_000 => self.jr(),
        0b001_001 => self.jalr(),
        0b001_100 => self.syscall(),
        0b001_101 => self.brk(),
        0b011_000 => self.mult(),
        0b011_001 => self.multu(),
        0b011_010 => self.div(),
        0b011_011 => self.divu(),
        0b010_000 => self.mfhi(),
        0b010_001 => self.mthi(),
        0b010_010 => self.mflo(),
        0b010_011 => self.mtlo(),
        0b100_000 => self.add(),
        0b100_001 => self.addu(),
        0b100_010 => self.sub(),
        0b100_011 => self.subu(),
        0b100_100 => self.and(),
        0b100_101 => self.or(),
        0b101_010 => self.slt(),
        0b101_011 => self.sltu(),
        0b100_111 => self.nor(),
        0b100_110 => self.xor(),
        0b000_111 => self.srav(),
        0b000_110 => self.srlv(),
        _ => self.exception(Exception::IllegalInstr),
        }
      }
      
      0b010_000 => match self.i.rs().0 {
        0b00_000 => self.mfc0(),
        0b00_100 => self.mtc0(),
        0b10_000 => self.rfe(),
        _ => panic!("unhandled coprocessor0 instr {:b}", self.i.rs().0)
      }
      
      0b010_001 => self.exception(Exception::CopError),
      0b010_010 => panic!("unhandled coprocessor 2"),
      0b010_011 => self.exception(Exception::CopError),
      
      0x30 => self.exception(Exception::CopError),
      0x31 => self.exception(Exception::CopError),
      0x32 => panic!("unhandled coprocessor 2 load"),
      0x33 => self.exception(Exception::CopError),
      
      0x38 => self.exception(Exception::CopError),
      0x39 => self.exception(Exception::CopError),
      0x3a => panic!("unhandled coprocessor 2 store"),
      0x3b => self.exception(Exception::CopError),

      0b000_001 => self.bxxx(),
      0b000_010 => self.jump(),
      0b000_011 => self.jal(),
      0b000_100 => self.beq(),
      0b000_101 => self.bne(),
      0b000_110 => self.blez(),
      0b000_111 => self.bgtz(),
      0b001_000 => self.addi(),
      0b001_001 => self.addiu(),
      0b001_010 => self.slti(),
      0b001_011 => self.sltiu(),
      0b001_100 => self.andi(),
      0b001_101 => self.ori(),
      0b001_110 => self.xori(),
      0b001_111 => self.lui(),
      0b100_000 => self.lb(),
      0b100_001 => self.lh(),
      0b100_101 => self.lhu(),
      0b100_100 => self.lbu(),
      0b100_011 => self.lw(),
      0b101_000 => self.sb(),
      0b101_001 => self.sh(),
      0b101_011 => self.sw(),
      0x22 => self.lwl(),
      0x26 => self.lwr(),
      0x2a => self.swl(),
      0x2e => self.swr(),

      _ => self.exception(Exception::IllegalInstr),
    }
  }

  fn mtc0(&mut self) {
    let res = self.rt_val();
    self.cop0.set_reg(self.i.rd(), res);
  }

  fn mfc0(&mut self) {
    let res = self.cop0.reg(self.i.rd());
    self.ld_delay_slots.push_back((self.i.rt(), res));
  }

  fn rfe(&mut self) {
    if self.i.funct() != 0b01_0000 {
      panic!("unhandled coprocessor 0 rfe instruction {:b}", self.i.funct());
    }

    let mode = self.cop0.sr & 0x3f;
    self.cop0.sr = (self.cop0.sr & !0x3f) | (mode >> 2);
  }

  fn lui(&mut self) {
    let res = self.i.imm16() << 16;
    self.set_reg(self.i.rt(), res);
  }

  fn lw(&mut self) {
    if self.cop0.is_cache_isolated() {
      // eprintln!("ignoring load while cache is isolated");
      return;
    }

    let addr = self.rs_val().wrapping_add(self.i.imm16sign());
    if addr % 4 == 0 {
      let res = self.mmu.read32(addr);
      self.ld_delay_slots.push_back((self.i.rt(), res));
    } else {
      self.exception(Exception::IllegalLoad);
    }
  }

  fn lh(&mut self) {
    if self.cop0.is_cache_isolated() {
      // eprintln!("ignoring load while cache is isolated");
      return;
    }

    let addr = self.rs_val().wrapping_add(self.i.imm16sign());
    if addr % 2 == 0 {
      let res = self.mmu.read16(addr) as i16;

      self.ld_delay_slots.push_back((self.i.rt(), res as u32));
    } else {
      self.exception(Exception::IllegalLoad);
    }
  }

  fn lhu(&mut self) {
    if self.cop0.is_cache_isolated() {
      // eprintln!("ignoring load while cache is isolated");
      return;
    }

    let addr = self.rs_val().wrapping_add(self.i.imm16sign());
    if addr % 2 == 0 {
      let res = self.mmu.read16(addr);
      self.ld_delay_slots.push_back((self.i.rt(), res));
    } else {
      self.exception(Exception::IllegalLoad);
    }
  }

  fn lb(&mut self) {
    if self.cop0.is_cache_isolated() {
      // eprintln!("ignoring load while cache is isolated");
      return;
    }

    let addr = self.rs_val().wrapping_add(self.i.imm16sign());
    let res = self.mmu.read8(addr) as i8;
    
    self.ld_delay_slots.push_back((self.i.rt(), res as u32));
  }

  fn lbu(&mut self) {
    if self.cop0.is_cache_isolated() {
      // eprintln!("ignoring load while cache is isolated");
      return;
    }

    let addr = self.rs_val().wrapping_add(self.i.imm16sign());
    let res = self.mmu.read8(addr);

    self.ld_delay_slots.push_back((self.i.rt(), res));
  }

  fn lwl(&mut self) {
    let addr = self.rs_val().wrapping_add(self.i.imm16sign());
    let reg = self.ld_delay_slots
      .iter()
      .find_map(|r| (r.0 == self.i.rt()).then_some(r.1))
      .unwrap_or(self.rt_val());

    let aligned_addr = addr & !3;
    let aligned_word = self.mmu.read32(aligned_addr);

    let res = match addr & 3 {
      0 => (reg & 0x00ff_ffff) | (aligned_word << 24), 
      1 => (reg & 0x0000_ffff) | (aligned_word << 16), 
      2 => (reg & 0x0000_00ff) | (aligned_word << 8), 
      3 => (reg & 0x0000_0000) | (aligned_word << 0), 
      _ => unreachable!()
    };

    self.ld_delay_slots.push_back((self.i.rt(), res));
  }

  fn lwr(&mut self) {
    let addr = self.rs_val().wrapping_add(self.i.imm16sign());
    let reg = self.ld_delay_slots
      .iter()
      .find_map(|r| (r.0 == self.i.rt()).then_some(r.1))
      .unwrap_or(self.rt_val());

    let aligned_addr = addr & !3;
    let aligned_word = self.mmu.read32(aligned_addr);

    let res = match addr & 3 {
      0 => (reg & 0x0000_0000) | (aligned_word << 0), 
      1 => (reg & 0xff00_0000) | (aligned_word << 8), 
      2 => (reg & 0xffff_0000) | (aligned_word << 16), 
      3 => (reg & 0xffff_ff00) | (aligned_word << 24), 
      _ => unreachable!()
    };

    self.ld_delay_slots.push_back((self.i.rt(), res));
  }

  fn sw(&mut self) {
    if self.cop0.is_cache_isolated() {
      // eprintln!("ignoring store while cache is isolated");
      return;
    }

    let addr = self.rs_val().wrapping_add(self.i.imm16sign());
    if addr % 4 == 0 {
      let val = self.rt_val();
      self.mmu.write32(addr, val);
    } else {
      self.exception(Exception::IllegalStore);
    }
  }

  fn sh(&mut self) {
    if self.cop0.is_cache_isolated() {
      // eprintln!("ignoring store while cache is isolated");
      return;
    }

    let addr = self.rs_val().wrapping_add(self.i.imm16sign());
    if addr % 2 == 0 {
      let val = self.rt_val();
      self.mmu.write16(addr, val);
    } else {
      self.exception(Exception::IllegalStore);
    }
  }

  fn sb(&mut self) {
    if self.cop0.is_cache_isolated() {
      // eprintln!("ignoring store while cache is isolated");
      return;
    }

    let addr = self.rs_val().wrapping_add(self.i.imm16sign());
    let val = self.rt_val();
    self.mmu.write8(addr, val);
  }

  fn swl(&mut self) {
    let addr = self.rs_val().wrapping_add(self.i.imm16sign());
    let reg = self.rt_val();

    let aligned_addr = addr & !3;
    let aligned_word = self.mmu.read32(aligned_addr);

    let res = match addr & 3 {
      0 => (reg & 0xffff_ff00) | (aligned_word >> 24), 
      1 => (reg & 0xffff_0000) | (aligned_word >> 16), 
      2 => (reg & 0xff00_0000) | (aligned_word >> 8), 
      3 => (reg & 0x0000_0000) | (aligned_word >> 0), 
      _ => unreachable!()
    };

    self.mmu.write32(addr, res);
  }

  fn swr(&mut self) {
    let addr = self.rs_val().wrapping_add(self.i.imm16sign());
    let reg = self.rt_val();

    let aligned_addr = addr & !3;
    let aligned_word = self.mmu.read32(aligned_addr);

    let res = match addr & 3 {
      0 => (reg & 0x0000_0000) | (aligned_word << 0), 
      1 => (reg & 0x0000_00ff) | (aligned_word << 8), 
      2 => (reg & 0x0000_ffff) | (aligned_word << 16), 
      3 => (reg & 0x00ff_ffff) | (aligned_word << 24), 
      _ => unreachable!()
    };

    self.mmu.write32(addr, res);
  }

  fn add(&mut self) {
    let rs = self.rs_val() as i32;
    let res = rs.checked_add(self.rt_val() as i32);
    
    match res {
      Some(v) => self.set_reg(self.i.rd(), v as u32),
      None =>  self.exception(Exception::Overflow),
    }
  }

  fn addi(&mut self) {
    let rs = self.rs_val() as i32;
    let res = rs.checked_add(self.i.imm16sign() as i32);
  
    match res {
      Some(v) => self.set_reg(self.i.rt(), v as u32),
      None => self.exception(Exception::Overflow),
    }
  }

  fn addiu(&mut self) {
    let res = self.rs_val().wrapping_add(self.i.imm16sign());
    self.set_reg(self.i.rt(), res);
  }

  fn addu(&mut self) {
    let res = self.rs_val().wrapping_add(self.rt_val());
    self.set_reg(self.i.rd(), res);
  }

  fn sub(&mut self) {
    let rs = self.rs_val() as i32;
    let res = rs.checked_sub(self.rt_val() as i32);
    
    match res {
      Some(v) => self.set_reg(self.i.rd(), v as u32),
      None =>  self.exception(Exception::Overflow),
    }
  }

  fn subu(&mut self) {
    let res = self.rs_val().wrapping_sub(self.rt_val());
    self.set_reg(self.i.rd(), res);
  }

  fn mult(&mut self) {
    let a = self.rs_val() as i32;
    let b = self.rt_val() as i32;
    let res = a as i64 * b as i64;
    self.lo = res as u32;
    self.hi = (res >> 32) as u32;
  }

  fn multu(&mut self) {
    let res = self.rs_val() as u64 * self.rt_val() as u64;
    self.lo = res as u32;
    self.hi = (res >> 32) as u32;
  }

  fn div(&mut self) {
    // TODO: division stall

    let dividend = self.rs_val() as i32;
    let divisor = self.rt_val() as i32;

    if divisor == 0 {
      self.hi = dividend as u32;

      self.lo = if dividend > 0 {
        0xffff_ffff
      } else {
        1
      };
    } else if dividend as u32 == 0x8000_0000 && divisor == -1 {
      // result too big
      self.hi = 0;
      self.lo = 0x8000_0000;
    } else {
      self.hi = (dividend % divisor) as u32;
      self.lo = (dividend / divisor) as u32;
    }
  }

  fn divu(&mut self) {
    // TODO: division stall

    let dividend = self.rs_val();
    let divisor = self.rt_val();

    if divisor == 0 {
      self.hi = dividend;
      self.lo = 0xffff_ffff;
    } else {
      self.hi = (dividend % divisor) as u32;
      self.lo = (dividend / divisor) as u32;
    }
  }

  fn mfhi(&mut self) {
    self.set_reg(self.i.rd(), self.hi);
  }

  fn mflo(&mut self) {
    self.set_reg(self.i.rd(), self.lo);
  }

  fn mthi(&mut self) {
    self.hi = self.rs_val()
  }

  fn mtlo(&mut self) {
    self.lo = self.rs_val()
  }

  fn jump(&mut self) {
    let res = (self.pc & 0xf000_0000) | self.i.offset26();
    self.next_pc = res;
    self.in_delay_slot = true;
  }

  fn jal(&mut self) {
    self.set_reg(Reg(31), self.next_pc);
    self.jump();
  }

  fn jr(&mut self) {
    self.next_pc = self.rs_val();
    self.in_delay_slot = true;
  }

  fn jalr(&mut self) {
    self.set_reg(self.i.rd(), self.next_pc);
    self.next_pc = self.rs_val();
    self.in_delay_slot = true;
  }

  fn exception(&mut self, expt: Exception) {
    self.cop0.cause = (self.cop0.cause & !0x7c) | ((expt as u32) << 2);

    let mode = self.cop0.sr & 0x3f;
    self.cop0.sr = (self.cop0.sr & !0x3f) | ((mode << 2) & 0x3f);

    let handler = match self.cop0.boot_expt_vector() {
      true  => 0xbfc0_0180,
      false => 0x8000_0080,
    };

    self.cop0.epc = self.curr_pc;
    if self.in_delay_slot {
      self.cop0.epc = self.cop0.epc.wrapping_sub(4);
      self.cop0.cause |= 1 << 31;
    }

    self.pc = handler;
    self.next_pc = self.pc.wrapping_add(4);
  }

  fn syscall(&mut self) {
    self.exception(Exception::Syscall);
  }

  fn slt(&mut self) {
    let res = (self.rs_val() as i32) < (self.rt_val() as i32);
    self.set_reg(self.i.rd(), res as u32);
  }

  fn slti(&mut self) {
    let res = (self.rs_val() as i32) < (self.i.imm16sign() as i32);
    self.set_reg(self.i.rt(), res as u32);
  }

  fn sltu(&mut self) {
    let res = self.rs_val() < self.rt_val();
    self.set_reg(self.i.rd(), res as u32);
  }

  fn sltiu(&mut self) {
    let res = self.rs_val() < self.i.imm16sign();
    self.set_reg(self.i.rt(), res as u32);
  }

  fn branch(&mut self, cond: bool) {
    if cond {
      self.next_pc = self.next_pc
        // compensate for the +4 in step()
        .wrapping_sub(4)
        .wrapping_add(self.i.offset16sign());
    }

    self.in_delay_slot = true;
  }

  fn beq(&mut self) {
    let cond = self.rs_val() == self.rt_val();
    self.branch(cond);
  }

  fn bgez(&mut self) {
    self.branch((self.rs_val() as i32) >= 0);
  }

  fn bgezal(&mut self) {
    self.set_reg(Reg(31), self.next_pc);
    self.bgez();
  }

  fn bgtz(&mut self) {
    self.branch((self.rs_val() as i32) > 0);
  }

  fn blez(&mut self) {
    self.branch((self.rs_val() as i32) <= 0);
  }

  fn bltz(&mut self) {
    self.branch((self.rs_val() as i32) < 0);
  }

  fn bltzal(&mut self) {
    self.set_reg(Reg(31), self.next_pc);
    self.bltz();
  }

  fn bne(&mut self) {
    let cond = self.rs_val() != self.rt_val();
    self.branch(cond);
  }


  fn bxxx(&mut self) {
    let kind = self.i.rt().0;
    let is_bgez = kind & 1 != 0;
    let is_link = kind & 1_0000 != 0;

    match (is_bgez, is_link) {
      (true, true) => self.bgezal(),
      (true, false) => self.bgez(),
      (false, true) => self.bltzal(),
      (false, false) => self.bltz(),
    }
  }

  fn brk(&mut self) { 
    self.exception(Exception::Break);
  }

  fn and(&mut self) {
    let res = self.rs_val() & self.rt_val();
    self.set_reg(self.i.rd(), res);
  }

  fn andi(&mut self) {
    let res = self.rs_val() & self.i.imm16();
    self.set_reg(self.i.rt(), res);
  }

  fn or(&mut self) {
    let res = self.rs_val() | self.rt_val();
    self.set_reg(self.i.rd(), res);
  }

  fn ori(&mut self) {
    let res = self.rs_val() | self.i.imm16();
    self.set_reg(self.i.rt(), res);
  }

  fn nor(&mut self) {
    let res = !(self.rs_val() | self.rt_val());
    self.set_reg(self.i.rd(), res);
  }

  fn xor(&mut self) {
    let res = self.rs_val() ^ self.rt_val();
    self.set_reg(self.i.rd(), res);
  }

  fn xori(&mut self) {
    let res = self.rs_val() ^ self.i.imm16();
    self.set_reg(self.i.rt(), res);
  }

  fn sll(&mut self) {
    let res = self.rt_val() << self.i.shift();
    self.set_reg(self.i.rd(), res);
  }

  fn srl(&mut self) {
    let res = self.rt_val() >> self.i.shift();
    self.set_reg(self.i.rd(), res);
  }

  fn sra(&mut self) {
    let res = (self.rt_val() as i32) >> self.i.shift();
    self.set_reg(self.i.rd(), res as u32);
  }

  fn sllv(&mut self) {
    let res = self.rt_val() << (self.rs_val() & 0x1f);
    self.set_reg(self.i.rd(), res);
  }

  fn srav(&mut self) {
    let res = (self.rt_val() as i32) >> (self.rs_val() & 0x1f);
    self.set_reg(self.i.rd(), res as u32);
  }

  fn srlv(&mut self) {
    let res = self.rt_val() >> (self.rs_val() & 0x1f);
    self.set_reg(self.i.rd(), res);
  }
}
