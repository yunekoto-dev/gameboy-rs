mod launcher;

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

use gameboy_rs::emulator::Emulator;
use gameboy_rs::hardware::joypad::Button as GbButton;
use gameboy_rs::{SCREEN_HEIGHT, SCREEN_WIDTH};
use sdl2::audio::{AudioQueue, AudioSpecDesired};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;

fn usage() {
    println!("Usage:\n  gameboy-rs [rom.gb|gbc] [OPTIONS]\n\nIf no ROM path is given, a retro picker window opens so you can\nbrowse your computer for a game.\n\nOptions:\n  --headless             Run without opening the SDL window\n  --cycles <N>           Run N emulation cycles in headless mode\n  --frames <N>           Run N frames, then exit\n  --save <PATH>          Override the .sav path\n  -h, --help             Show this help\n\nControls:\n  Arrow keys             D-pad\n  Z / X                  A / B\n  Right Shift            Select\n  Enter                  Start\n  Escape                 Quit\n\nThe hardware model (DMG/CGB) is detected automatically from the ROM header.");
}
fn die_usage(message: &str) -> ! { eprintln!("error: {message}\n"); usage(); process::exit(2); }

#[derive(Debug, Default)]
struct Cli { rom_path: Option<PathBuf>, headless: bool, cycles: Option<u64>, frames: Option<u64>, save_path: Option<PathBuf> }
fn parse_args<I: IntoIterator<Item=String>>(args:I)->Cli{
    let mut it=args.into_iter();let mut cli=Cli::default();let mut pos=Vec::new();
    while let Some(arg)=it.next(){match arg.as_str(){"--"=>{pos.extend(it);break},"-h"|"--help"=>{usage();process::exit(0)},"--headless"=>cli.headless=true,
        "--cycles"=>{let v=it.next().unwrap_or_else(||die_usage("--cycles requires a number"));cli.cycles=Some(v.parse().unwrap_or_else(|_|die_usage("--cycles expects an unsigned integer")))},
        "--frames"=>{let v=it.next().unwrap_or_else(||die_usage("--frames requires a number"));cli.frames=Some(v.parse().unwrap_or_else(|_|die_usage("--frames expects an unsigned integer")))},
        "--save"=>{let v=it.next().unwrap_or_else(||die_usage("--save requires a path"));cli.save_path=Some(PathBuf::from(v))},
        s if s.starts_with('-')=>die_usage(&format!("unknown option: {s}")),_=>pos.push(arg)}}
    match pos.len(){
        0=>{},
        1=>cli.rom_path=Some(PathBuf::from(pos.remove(0))),
        _=>die_usage("multiple ROM paths were provided"),
    }
    cli
}

fn main(){
    let cli=parse_args(env::args().skip(1));
    let rom_path=match cli.rom_path{
        Some(p)=>p,
        None=>match launcher::pick_rom(){
            Some(p)=>p,
            None=>return,
        },
    };
    let rom=fs::read(&rom_path).unwrap_or_else(|e|{eprintln!("failed to read {}: {e}",rom_path.display());process::exit(1)});
    let save=cli.save_path.or_else(||Some(rom_path.with_extension("sav")));
    let mut machine=Emulator::from_rom(rom,save.as_deref()).unwrap_or_else(|e|{eprintln!("failed to start Game Boy: {e}");process::exit(1)});
    println!("ROM: {} | Model: {} | Title: {}",rom_path.display(),machine.model().name(),machine.bus.cart.title());
    if cli.headless{let n=cli.cycles.unwrap_or(1_000_000);if let Err(e)=machine.run_cycles(n){eprintln!("emulation error: {e}");process::exit(1)}let _=machine.save();return;}
    if let Err(e)=run_sdl(&mut machine,cli.frames.unwrap_or(u64::MAX)){eprintln!("frontend error: {e}");let _=machine.save();process::exit(1)}let _=machine.save();
}

fn run_sdl(machine:&mut Emulator,target_frames:u64)->Result<(),String>{
    let sdl=sdl2::init()?;let video=sdl.video()?;let audio=sdl.audio()?;
    let (width,height)=(SCREEN_WIDTH,SCREEN_HEIGHT);
    let window=video.window("Game Boy — gameboy-rs",(width*3)as u32,(height*3)as u32).position_centered().build().map_err(|e|e.to_string())?;
    let mut canvas=window.into_canvas().present_vsync().build().map_err(|e|e.to_string())?;
    let creator=canvas.texture_creator();let mut texture=creator.create_texture_streaming(PixelFormatEnum::ARGB8888,width as u32,height as u32).map_err(|e|e.to_string())?;
    let spec=AudioSpecDesired{freq:Some(44_100),channels:Some(2),samples:Some(1024)};let queue:AudioQueue<i16>=audio.open_queue(None,&spec).map_err(|e|e.to_string())?;queue.resume();
    let mut pump=sdl.event_pump()?;let mut pixels=vec![0u8;width*height*4];let mut frames=0u64;let frame_ns:u64=16_742_706;
    'running:while frames<target_frames{let start=std::time::Instant::now();for ev in pump.poll_iter(){match ev{Event::Quit{..}=>break 'running,Event::KeyDown{keycode:Some(Keycode::Escape),repeat:false,..}=>break 'running,Event::KeyDown{keycode:Some(k),repeat:false,..}=>set_key(machine,k,true),Event::KeyUp{keycode:Some(k),repeat:false,..}=>set_key(machine,k,false),_=>{}}}
        machine.run_frame().map_err(|e|e.to_string())?;for(i,&px)in machine.framebuffer().iter().enumerate(){pixels[i*4..i*4+4].copy_from_slice(&px.to_ne_bytes());}texture.update(None,&pixels,width*4).map_err(|e|e.to_string())?;canvas.clear();canvas.copy(&texture,None,None)?;canvas.present();
        let samples=machine.bus.apu.generate_samples(738);let _=queue.queue_audio(&samples);
        let elapsed=start.elapsed();let target=std::time::Duration::from_nanos(frame_ns);if elapsed<target{std::thread::sleep(target-elapsed)}frames+=1;}
    Ok(())
}

fn set_key(machine:&mut Emulator,k:Keycode,down:bool){
    let b=match k{Keycode::Right=>Some(GbButton::Right),Keycode::Left=>Some(GbButton::Left),Keycode::Up=>Some(GbButton::Up),Keycode::Down=>Some(GbButton::Down),Keycode::Z=>Some(GbButton::A),Keycode::X=>Some(GbButton::B),Keycode::RShift=>Some(GbButton::Select),Keycode::Return=>Some(GbButton::Start),_=>None};
    if let Some(b)=b{if down{machine.bus.joypad.press(b,&mut machine.bus.interrupts)}else{machine.bus.joypad.release(b)}}
}
