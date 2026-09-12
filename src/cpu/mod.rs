use std::fmt;
use crate::bus::Bus;

pub const FLAG_Z: u8 = 0x80;
pub const FLAG_N: u8 = 0x40;
pub const FLAG_H: u8 = 0x20;
pub const FLAG_C: u8 = 0x10;

#[derive(Clone, Copy, Debug, Default)]
pub struct Registers { pub a:u8, pub f:u8, pub b:u8, pub c:u8, pub d:u8, pub e:u8, pub h:u8, pub l:u8, pub sp:u16, pub pc:u16 }
impl Registers {
    pub fn af(&self)->u16 { u16::from_be_bytes([self.a, self.f & 0xF0]) }
    pub fn bc(&self)->u16 { u16::from_be_bytes([self.b,self.c]) }
    pub fn de(&self)->u16 { u16::from_be_bytes([self.d,self.e]) }
    pub fn hl(&self)->u16 { u16::from_be_bytes([self.h,self.l]) }
    pub fn set_af(&mut self,v:u16){let [a,f]=v.to_be_bytes();self.a=a;self.f=f&0xF0}
    pub fn set_bc(&mut self,v:u16){let [b,c]=v.to_be_bytes();self.b=b;self.c=c}
    pub fn set_de(&mut self,v:u16){let [d,e]=v.to_be_bytes();self.d=d;self.e=e}
    pub fn set_hl(&mut self,v:u16){let [h,l]=v.to_be_bytes();self.h=h;self.l=l}
}

pub struct Cpu { pub regs:Registers, pub ime:bool, pub halted:bool, pub stopped:bool, ime_delay:u8, halt_bug:bool }
impl Cpu {
    pub fn post_boot(cgb: bool)->Self {
        let regs = if cgb {
            // Mainline CGB boot ROM handoff state (PC = $0100).
            Registers { a: 0x11, f: 0x80, b: 0x00, c: 0x00, d: 0xFF, e: 0x56, h: 0x00, l: 0x0D, sp: 0xFFFE, pc: 0x0100 }
        } else {
            // Mainline DMG boot ROM handoff state.
            Registers { a: 0x01, f: 0xB0, b: 0x00, c: 0x13, d: 0x00, e: 0xD8, h: 0x01, l: 0x4D, sp: 0xFFFE, pc: 0x0100 }
        };
        Self { regs, ime:false, halted:false, stopped:false, ime_delay:0, halt_bug:false }
    }
    pub fn reset_post_boot(&mut self, cgb: bool){*self=Self::post_boot(cgb);}

    pub fn step(&mut self, bus:&mut Bus)->Result<u8,CpuError>{
        let pending = bus.ie & bus.interrupts.flags & 0x1F;
        if self.stopped {
            if pending != 0 { self.stopped = false; } else { return Ok(4); }
        }
        if self.halted {
            if pending == 0 { return Ok(4); }
            self.halted = false;
            if !self.ime { self.halt_bug = true; }
        }
        if self.ime && pending != 0 { return Ok(self.service_interrupt(bus, pending)); }

        let opcode = self.fetch8(bus);
        let cycles = self.execute(opcode, bus)?;
        if self.ime_delay > 0 {
            self.ime_delay -= 1;
            if self.ime_delay == 0 { self.ime = true; }
        }
        Ok(cycles)
    }

    fn fetch8(&mut self,b:&mut Bus)->u8 {
        let pc=self.regs.pc;
        let v=b.read8(pc);
        if !self.halt_bug { self.regs.pc=self.regs.pc.wrapping_add(1); } else { self.halt_bug=false; }
        v
    }
    fn fetch16(&mut self,b:&mut Bus)->u16 { let lo=self.fetch8(b);let hi=self.fetch8(b);u16::from_le_bytes([lo,hi]) }
    fn push16(&mut self,b:&mut Bus,v:u16){let [hi,lo]=v.to_be_bytes();self.regs.sp=self.regs.sp.wrapping_sub(1);b.write8(self.regs.sp,hi);self.regs.sp=self.regs.sp.wrapping_sub(1);b.write8(self.regs.sp,lo)}
    fn pop16(&mut self,b:&mut Bus)->u16{let lo=b.read8(self.regs.sp);self.regs.sp=self.regs.sp.wrapping_add(1);let hi=b.read8(self.regs.sp);self.regs.sp=self.regs.sp.wrapping_add(1);u16::from_le_bytes([lo,hi])}
    fn set_flags(&mut self,z:bool,n:bool,h:bool,c:bool){self.regs.f=(if z{FLAG_Z}else{0})|(if n{FLAG_N}else{0})|(if h{FLAG_H}else{0})|(if c{FLAG_C}else{0});}
    fn flag(&self,m:u8)->bool{self.regs.f&m!=0}

    fn execute(&mut self, op:u8, b:&mut Bus)->Result<u8,CpuError>{
        if op==0xCB { let cb_op = self.fetch8(b); return Ok(self.execute_cb(cb_op,b)); }
        if (0x40..=0x7F).contains(&op) {
            if op==0x76 {
                let pending=b.ie & b.interrupts.flags & 0x1F;
                if !self.ime && pending!=0 { self.halt_bug=true; } else { self.halted=true; }
                return Ok(4)
            }
            let dst=(op>>3)&7; let src=op&7; let v=self.read_r8(src,b); self.write_r8(dst,v,b); return Ok(if dst==6||src==6{8}else{4});
        }
        if (0x80..=0xBF).contains(&op) {
            let y=(op>>3)&7; let src=op&7; let v=self.read_r8(src,b); self.alu(y,v); return Ok(if src==6{8}else{4});
        }
        if op&0xC7==0x04 { let r=(op>>3)&7; let v=self.read_r8(r,b); let n=self.inc8(v); self.write_r8(r,n,b); return Ok(if r==6{12}else{4}); }
        if op&0xC7==0x05 { let r=(op>>3)&7; let v=self.read_r8(r,b); let n=self.dec8(v); self.write_r8(r,n,b); return Ok(if r==6{12}else{4}); }
        if op&0xC7==0x06 { let r=(op>>3)&7; let n=self.fetch8(b); self.write_r8(r,n,b); return Ok(if r==6{12}else{8}); }
        if op&0xCF==0x03 { let p=(op>>4)&3; let v=self.rp(p); self.set_rp(p,v.wrapping_add(1)); return Ok(8); }
        if op&0xCF==0x0B { let p=(op>>4)&3; let v=self.rp(p); self.set_rp(p,v.wrapping_sub(1)); return Ok(8); }
        if op&0xCF==0x01 { let p=(op>>4)&3; let v=self.fetch16(b); self.set_rp(p,v); return Ok(12); }
        if op&0xCF==0x09 { let p=(op>>4)&3; let v=self.rp(p); self.add_hl(v); return Ok(8); }
        if op&0xCF==0x02 { let p=(op>>4)&3; let v=self.regs.a; match p {0=>b.write8(self.regs.bc(),v),1=>b.write8(self.regs.de(),v),2=>{let a=self.regs.hl();b.write8(a,v);self.regs.set_hl(a.wrapping_add(1));},_=>{let a=self.regs.hl();b.write8(a,v);self.regs.set_hl(a.wrapping_sub(1));}} return Ok(8); }
        if op&0xCF==0x0A { let p=(op>>4)&3; self.regs.a=match p {0=>b.read8(self.regs.bc()),1=>b.read8(self.regs.de()),2=>{let a=self.regs.hl();let v=b.read8(a);self.regs.set_hl(a.wrapping_add(1));v},_=>{let a=self.regs.hl();let v=b.read8(a);self.regs.set_hl(a.wrapping_sub(1));v}}; return Ok(8); }

        match op {
            0x00=>Ok(4),
            0x08=>{let a=self.fetch16(b);b.write8(a,self.regs.sp as u8);b.write8(a.wrapping_add(1),(self.regs.sp>>8) as u8);Ok(20)}
            0x10=>{let _=self.fetch8(b);if !b.switch_speed_if_armed(){self.stopped=true;}Ok(4)}
            0x18=>{let e=self.fetch8(b) as i8;self.regs.pc=self.regs.pc.wrapping_add(e as i16 as u16);Ok(12)}
            0x20|0x28|0x30|0x38=>{let e=self.fetch8(b) as i8;let c=(op>>3)&3; if self.cond(c){self.regs.pc=self.regs.pc.wrapping_add(e as i16 as u16);Ok(12)}else{Ok(8)}}
            0x07=>{self.rotate_a(0);Ok(4)} 0x0F=>{self.rotate_a(1);Ok(4)} 0x17=>{self.rotate_a(2);Ok(4)} 0x1F=>{self.rotate_a(3);Ok(4)}
            0x27=>{self.daa();Ok(4)} 0x2F=>{self.regs.a = !self.regs.a;self.regs.f|=FLAG_N|FLAG_H;Ok(4)}
            0x37=>{let z=self.flag(FLAG_Z);self.set_flags(z,false,false,true);Ok(4)}
            0x3F=>{let z=self.flag(FLAG_Z);self.set_flags(z,false,false,!self.flag(FLAG_C));Ok(4)}
            0xC0|0xC8|0xD0|0xD8=>{let c=(op>>3)&3;if self.cond(c){self.regs.pc=self.pop16(b);Ok(20)}else{Ok(8)}}
            0xC1|0xD1|0xE1|0xF1=>{let p=(op>>4)&3;let v=self.pop16(b);self.set_rp2(p,v);Ok(12)}
            0xC2|0xCA|0xD2|0xDA=>{let a=self.fetch16(b);let c=(op>>3)&3;if self.cond(c){self.regs.pc=a;Ok(16)}else{Ok(12)}}
            0xC3=>{self.regs.pc=self.fetch16(b);Ok(16)}
            0xC4|0xCC|0xD4|0xDC=>{let a=self.fetch16(b);let c=(op>>3)&3;if self.cond(c){self.push16(b,self.regs.pc);self.regs.pc=a;Ok(24)}else{Ok(12)}}
            0xC5|0xD5|0xE5|0xF5=>{let p=(op>>4)&3;self.push16(b,self.rp2(p));Ok(16)}
            0xC6|0xCE|0xD6|0xDE|0xE6|0xEE|0xF6|0xFE=>{let y=(op>>3)&7;let v=self.fetch8(b);self.alu(y,v);Ok(8)}
            0xC7|0xCF|0xD7|0xDF|0xE7|0xEF|0xF7|0xFF=>{self.push16(b,self.regs.pc);self.regs.pc=(op&0x38) as u16;Ok(16)}
            0xC9=>{self.regs.pc=self.pop16(b);Ok(16)}
            0xCD=>{let a=self.fetch16(b);self.push16(b,self.regs.pc);self.regs.pc=a;Ok(24)}
            0xD9=>{self.regs.pc=self.pop16(b);self.ime=true;self.ime_delay=0;Ok(16)}
            0xE0=>{let a=0xFF00u16+self.fetch8(b) as u16;b.write8(a,self.regs.a);Ok(12)}
            0xE2=>{b.write8(0xFF00+self.regs.c as u16,self.regs.a);Ok(8)}
            0xE8=>{let e=self.fetch8(b);let sp=self.regs.sp;let r=sp.wrapping_add(e as i8 as i16 as u16);let h=((sp&0x0F)+(e as u16&0x0F))>0x0F;let c=((sp&0xFF)+(e as u16&0xFF))>0xFF;self.regs.sp=r;self.set_flags(false,false,h,c);Ok(16)}
            0xE9=>{self.regs.pc=self.regs.hl();Ok(4)}
            0xEA=>{let a=self.fetch16(b);b.write8(a,self.regs.a);Ok(16)}
            0xF0=>{let a=0xFF00u16+self.fetch8(b) as u16;self.regs.a=b.read8(a);Ok(12)}
            0xF2=>{self.regs.a=b.read8(0xFF00+self.regs.c as u16);Ok(8)}
            0xF3=>{self.ime=false;self.ime_delay=0;Ok(4)}
            0xF8=>{let e=self.fetch8(b);let sp=self.regs.sp;let r=sp.wrapping_add(e as i8 as i16 as u16);let h=((sp&0x0F)+(e as u16&0x0F))>0x0F;let c=((sp&0xFF)+(e as u16&0xFF))>0xFF;self.regs.set_hl(r);self.set_flags(false,false,h,c);Ok(12)}
            0xF9=>{self.regs.sp=self.regs.hl();Ok(8)}
            0xFA=>{let a=self.fetch16(b);self.regs.a=b.read8(a);Ok(16)}
            0xFB=>{self.ime_delay=2;Ok(4)}
            _=>Err(CpuError::UnsupportedOpcode(op,self.regs.pc.wrapping_sub(1))),
        }
    }

    fn execute_cb(&mut self, op:u8,b:&mut Bus)->u8{
        let x=op>>6;let y=(op>>3)&7;let z=op&7;let v=self.read_r8(z,b);
        let n=match x{0=>self.cb_shift(y,v),1=>{let zf=v&(1<<y)==0;self.regs.f=(self.regs.f&FLAG_C)|FLAG_H|if zf{FLAG_Z}else{0};v},2=>v&!(1<<y),_=>v|(1<<y)};
        if x!=1{self.write_r8(z,n,b);}
        if z==6{if x==1{12}else{16}}else{8}
    }
    fn cb_shift(&mut self,y:u8,v:u8)->u8{
        let (n,c)=match y{0=>(v.rotate_left(1),v>>7),1=>(v.rotate_right(1),v&1),2=>((v<<1)|if self.flag(FLAG_C){1}else{0},v>>7),3=>((v>>1)|if self.flag(FLAG_C){0x80}else{0},v&1),4=>(v<<1,v>>7),5=>((v>>1)|(v&0x80),v&1),6=>(v.rotate_right(4),0),_=>(v>>1,v&1)};
        self.set_flags(n==0,false,false,c!=0);n
    }
    fn read_r8(&mut self,r:u8,b:&mut Bus)->u8{match r{0=>self.regs.b,1=>self.regs.c,2=>self.regs.d,3=>self.regs.e,4=>self.regs.h,5=>self.regs.l,6=>b.read8(self.regs.hl()),_=>self.regs.a}}
    fn write_r8(&mut self,r:u8,v:u8,b:&mut Bus){match r{0=>self.regs.b=v,1=>self.regs.c=v,2=>self.regs.d=v,3=>self.regs.e=v,4=>self.regs.h=v,5=>self.regs.l=v,6=>b.write8(self.regs.hl(),v),_=>self.regs.a=v}}
    fn rp(&self,p:u8)->u16{match p{0=>self.regs.bc(),1=>self.regs.de(),2=>self.regs.hl(),_=>self.regs.sp}}
    fn set_rp(&mut self,p:u8,v:u16){match p{0=>self.regs.set_bc(v),1=>self.regs.set_de(v),2=>self.regs.set_hl(v),_=>self.regs.sp=v}}
    fn rp2(&self,p:u8)->u16{if p==3{self.regs.af()}else{self.rp(p)}}
    fn set_rp2(&mut self,p:u8,v:u16){if p==3{self.regs.set_af(v)}else{self.set_rp(p,v)}}
    fn cond(&self,c:u8)->bool{match c{0=>!self.flag(FLAG_Z),1=>self.flag(FLAG_Z),2=>!self.flag(FLAG_C),_=>self.flag(FLAG_C)}}
    fn inc8(&mut self,v:u8)->u8{let n=v.wrapping_add(1);self.regs.f=(self.regs.f&FLAG_C)|if n==0{FLAG_Z}else{0}|if v&0x0F==0x0F{FLAG_H}else{0};n}
    fn dec8(&mut self,v:u8)->u8{let n=v.wrapping_sub(1);self.regs.f=(self.regs.f&FLAG_C)|FLAG_N|if n==0{FLAG_Z}else{0}|if v&0x0F==0{FLAG_H}else{0};n}
    fn add_hl(&mut self,v:u16){let hl=self.regs.hl();let r=hl.wrapping_add(v);let z=self.flag(FLAG_Z);let h=((hl&0x0FFF)+(v&0x0FFF))>0x0FFF;let c=(hl as u32+v as u32)>0xFFFF;self.set_flags(z,false,h,c);self.regs.set_hl(r)}
    fn alu(&mut self,y:u8,v:u8){match y{0=>self.add_a(v,false),1=>self.add_a(v,true),2=>self.sub_a(v,false),3=>self.sub_a(v,true),4=>{self.regs.a&=v;self.set_flags(self.regs.a==0,false,true,false)},5=>{self.regs.a^=v;self.set_flags(self.regs.a==0,false,false,false)},6=>{self.regs.a|=v;self.set_flags(self.regs.a==0,false,false,false)},_=>{let a=self.regs.a;let r=a.wrapping_sub(v);self.set_flags(r==0,true,(a&0x0F)<(v&0x0F),a<v)}}}
    fn add_a(&mut self,v:u8,carry:bool){let c=if carry&&self.flag(FLAG_C){1}else{0};let a=self.regs.a;let s=a as u16+v as u16+c as u16;let r=s as u8;self.regs.a=r;self.set_flags(r==0,false,((a&0x0F)+(v&0x0F)+c)>0x0F,s>0xFF)}
    fn sub_a(&mut self,v:u8,borrow:bool){let c=if borrow&&self.flag(FLAG_C){1}else{0};let a=self.regs.a;let r=a.wrapping_sub(v).wrapping_sub(c);self.regs.a=r;self.set_flags(r==0,true,(a&0x0F)<((v&0x0F)+c), (a as u16)<(v as u16+c as u16))}
    fn rotate_a(&mut self,k:u8){let a=self.regs.a;let (r,c)=match k{0=>(a.rotate_left(1),a>>7),1=>(a.rotate_right(1),a&1),2=>((a<<1)|if self.flag(FLAG_C){1}else{0},a>>7),_=>(a>>1|if self.flag(FLAG_C){0x80}else{0},a&1)};self.regs.a=r;self.set_flags(false,false,false,c!=0)}
    fn daa(&mut self){
        let mut a=self.regs.a;let mut adj=0;let mut c=self.flag(FLAG_C);
        if !self.flag(FLAG_N){
            if c||a>0x99{adj|=0x60;c=true;}
            if self.flag(FLAG_H)||a&0x0F>9{adj|=0x06;}
            a=a.wrapping_add(adj)
        }else{
            if c{adj|=0x60;}
            if self.flag(FLAG_H){adj|=0x06;}
            a=a.wrapping_sub(adj)
        }
        self.regs.a=a;self.regs.f=(if a==0{FLAG_Z}else{0})|(self.regs.f&FLAG_N)|if c{FLAG_C}else{0};
    }
    fn service_interrupt(&mut self,b:&mut Bus,pending:u8)->u8{let bit=pending.trailing_zeros() as u8;self.ime=false;self.ime_delay=0;b.interrupts.flags&=!(1<<bit);self.push16(b,self.regs.pc);self.regs.pc=0x0040+(bit as u16)*8;20}
}

#[derive(Debug)]
pub enum CpuError { UnsupportedOpcode(u8,u16) }
impl fmt::Display for CpuError { fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result{match self{Self::UnsupportedOpcode(op,pc)=>write!(f,"unsupported opcode 0x{op:02X} at 0x{pc:04X}")}} }
impl std::error::Error for CpuError {}
