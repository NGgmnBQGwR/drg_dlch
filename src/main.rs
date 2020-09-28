mod process;

use inputbot::{KeybdKey, MouseButton};
use process::Process;
use winapi::ctypes::c_void;

const GAME_PROCESS_NAME: &str = "FSD-Win64-Shipping.exe";
// use_button = 'E';
const CHARGED_SHOT_CHARGE_TIME: u64 = 1_150;
const NORMAL_SPEED: f32 = 44.1398490;
const CHARGED_SPEED: f32 = 13.199477;
const BALL_RADIUS: f32 = 1.5;
const WIND_DOWN_TIME: f32 = 0.1;

fn read_memory_offset<T>(process: &mut Process, offsets: &[usize]) -> Option<T> {
    let mut address = process.base() as usize;
    let noffsets: usize = offsets.len();
    for next_offset in offsets.iter().take(noffsets - 1) {
        match process.read::<usize>((address + next_offset) as *mut c_void) {
            None => return None,
            Some(v) => address = v,
        }
    }
    process.read::<T>((address + offsets.last().unwrap()) as *mut c_void)
}

fn get_delay(dist: f32) -> f32 {
    let normal_flight_time = dist / NORMAL_SPEED;
    let charged_flight_time = dist / CHARGED_SPEED;
    (charged_flight_time - normal_flight_time) - WIND_DOWN_TIME
}

fn main() {
    let mut process = Process::find_by_exe(GAME_PROCESS_NAME).expect("No DRG process found.");

    let offsets: Vec<usize> = vec![0x037C52D0, 0x1F8, 0x108, 0x610, 0x230, 0x118];
    let offsets: Vec<usize> = vec![0x037AA080, 0x30, 0x360, 0xcb8, 0x1c0, 0x658, 0x230, 0x118];
    println!("{:?}", read_memory_offset::<f32>(&mut process, &offsets));

    let (signal_sender, signal_receiver) = std::sync::mpsc::sync_channel(10);
    let (dist_sender, dist_receiver) = std::sync::mpsc::sync_channel(10);

    let dist_receiver_shared = std::sync::Arc::new(std::sync::Mutex::new(dist_receiver));

    KeybdKey::ZKey.bind(move || {
        println!("Sending request for distance.");
        signal_sender.send(()).unwrap();
        let dist = dist_receiver_shared.lock().unwrap().recv().unwrap();
        MouseButton::LeftButton.press();
        std::thread::sleep(std::time::Duration::from_millis(CHARGED_SHOT_CHARGE_TIME));
        MouseButton::LeftButton.release();
        std::thread::sleep(std::time::Duration::from_secs_f32(get_delay(dist)));
        MouseButton::LeftButton.press();
        std::thread::sleep(std::time::Duration::from_millis(20));
        MouseButton::LeftButton.release();
    });

    std::thread::spawn(||{inputbot::handle_input_events()});

    loop {
        signal_receiver.recv().unwrap();
        let dist = read_memory_offset::<f32>(&mut process, &offsets);
        println!("Got a request for dist, sending '{:?}'", dist);
        if let Some(v) = dist {
            dist_sender.send(v).unwrap();
        }
    }
}
