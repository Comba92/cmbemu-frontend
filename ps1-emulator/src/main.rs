use ps1_emulator::{cpu::Cpu, mmu::{Bios, Mmu}};

fn main() {
  let bios = Bios::new("ps-22a.bin").unwrap();
  let mmu = Mmu::new(bios);
  let mut cpu = Cpu::new(mmu);

  // let exe = include_bytes!("../psxtest_cpu.exe"); 
  // cpu.sideload_exe(exe);

  for i in 0..1_000_000_000 {
    cpu.step();
  }
}
