mod process;

use inputbot::{KeybdKey, MouseButton};
use process::Process;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::time::{Duration, Instant};
use winapi::ctypes::c_void;

const GAME_PROCESS_NAME: &str = "FSD-Win64-Shipping.exe";
const CHARGED_SHOT_CHARGE_TIME: u64 = 1_150;
const NORMAL_SPEED: f32 = 44.1398490;
const CHARGED_SPEED: f32 = 13.199477;
// const BALL_RADIUS: f32 = 1.5;
const WIND_DOWN_TIME: f32 = 0.1;
const RESUPPLY_START_DELAY: u64 = 200;
const DEFAULT_SERVER_IP: &str = "37.194.31.70";
const DEFAULT_SERVER_PORT: &str = "34197";
const CLIENT_PORT: u16 = 16089;
const TEST_PING_COUNT: u64 = 5;
// const NORMAL_SUPPLY_USE_TIME: u64 = 4_000;
const EXPLOIT_SUPPLY_USE_TIME: u64 = 2_500;

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

fn epc_helper() -> Result<(), anyhow::Error> {
    let mut process = Process::find_by_exe(GAME_PROCESS_NAME)
        .ok_or("No DRG process found.")
        .map_err(anyhow::Error::msg)?;

    let offsets: Vec<usize> = vec![0x037AA080, 0x30, 0x360, 0xcb8, 0x1c0, 0x658, 0x230, 0x118];

    let (signal_sender, signal_receiver) = std::sync::mpsc::sync_channel(10);
    let (dist_sender, dist_receiver) = std::sync::mpsc::sync_channel(10);

    let dist_receiver_shared = std::sync::Arc::new(std::sync::Mutex::new(dist_receiver));

    KeybdKey::ZKey.bind(move || {
        signal_sender.send(()).unwrap();
        let dist = dist_receiver_shared.lock().unwrap().recv().unwrap();
        if dist < 0.1 || dist > 1000.0 {
            return;
        }
        MouseButton::LeftButton.press();
        std::thread::sleep(std::time::Duration::from_millis(CHARGED_SHOT_CHARGE_TIME));
        MouseButton::LeftButton.release();
        std::thread::sleep(std::time::Duration::from_secs_f32(get_delay(dist)));
        MouseButton::LeftButton.press();
        std::thread::sleep(std::time::Duration::from_millis(20));
        MouseButton::LeftButton.release();
    });

    std::thread::spawn(|| inputbot::handle_input_events());

    loop {
        signal_receiver.recv()?;
        let dist = read_memory_offset::<f32>(&mut process, &offsets);
        if let Some(v) = dist {
            dist_sender.send(v)?;
        }
    }
}

fn get_ping(socket: &UdpSocket) -> u64 {
    let mut pings: Vec<u64> = Vec::with_capacity(TEST_PING_COUNT as usize);
    let mut buf = [0; 8];
    for _ in 0..TEST_PING_COUNT {
        let before_send = Instant::now();
        socket.send(&u64::MAX.to_be_bytes()).unwrap();
        socket.recv(&mut buf).unwrap();
        let after_recvd = Instant::now();
        pings.push((before_send.elapsed() - after_recvd.elapsed()).as_millis() as u64);
    }
    pings.iter().sum::<u64>() / TEST_PING_COUNT
}

fn start_client(ip: Ipv4Addr, port: u16) -> Result<(), anyhow::Error> {
    let server_addr = SocketAddrV4::new(ip, port);
    let local_addr = SocketAddrV4::new(Ipv4Addr::from([0, 0, 0, 0]), CLIENT_PORT);
    let socket = UdpSocket::bind(local_addr)?;
    socket.connect(server_addr)?;

    let ping = get_ping(&socket);

    KeybdKey::BKey.bind(move || {
        socket.send(&ping.to_be_bytes()).unwrap();
        let mut buf = [0; 8];
        socket.recv(&mut buf).unwrap();
        let timeout = u64::from_be_bytes(buf);
        std::thread::sleep(Duration::from_millis(timeout));
        KeybdKey::EKey.press();
        std::thread::sleep(Duration::from_millis(EXPLOIT_SUPPLY_USE_TIME));
        KeybdKey::EKey.release();
    });

    inputbot::handle_input_events();

    Ok(())
}

fn start_server(port: u16) -> Result<(), anyhow::Error> {
    let addr = SocketAddrV4::new(Ipv4Addr::from([0, 0, 0, 0]), port);
    let socket = UdpSocket::bind(addr)?;

    std::thread::spawn(|| inputbot::handle_input_events());

    let mut buf = [0; 8];
    loop {
        let (_, client_addr) = socket.recv_from(&mut buf).unwrap();
        if buf.iter().all(|x| *x == 0xFF) {
            socket.send_to(&0u64.to_be_bytes(), client_addr).unwrap();
            continue;
        }
        let ping = u64::from_be_bytes(buf);
        socket
            .send_to(&RESUPPLY_START_DELAY.to_be_bytes(), client_addr)
            .unwrap();
        std::thread::sleep(Duration::from_millis(RESUPPLY_START_DELAY + (ping/2)));
        KeybdKey::EKey.press();
        std::thread::sleep(Duration::from_millis(EXPLOIT_SUPPLY_USE_TIME));
        KeybdKey::EKey.release();
    }
}

fn main() -> Result<(), anyhow::Error> {
    let args = clap::App::new("DRG DLCH")
        .about("DRG Dirty Little Cheater Helper")
        .arg(
            clap::Arg::with_name("driller")
                .help("helps up with drillers' EPC usage")
                .short("d")
                .long("driller"),
        )
        .arg(
            clap::Arg::with_name("server")
                .help("starts the server for Ammo Exploiter")
                .short("s")
                .long("server")
                .requires("ip"),
        )
        .arg(
            clap::Arg::with_name("client")
                .help("starts the client for Ammo Exploiter")
                .short("c")
                .long("client")
                .requires_all(&["ip", "port"]),
        )
        .arg(
            clap::Arg::with_name("ip")
                .help("IP of Ammo Helper Server")
                .short("ip")
                .takes_value(true)
                .default_value(DEFAULT_SERVER_IP),
        )
        .arg(
            clap::Arg::with_name("port")
                .help("Port of Ammo Helper Server")
                .short("port")
                .takes_value(true)
                .default_value(DEFAULT_SERVER_PORT),
        )
        .group(
            clap::ArgGroup::with_name("actions")
                .args(&["driller", "client", "server"])
                .required(true),
        )
        .get_matches();

    let driller = args.is_present("driller");
    let client = args.is_present("client");
    let server = args.is_present("server");

    if driller {
        epc_helper()
    } else if client {
        start_client(
            args.value_of("ip").unwrap().parse()?,
            args.value_of("port").unwrap().parse()?,
        )
    } else if server {
        start_server(args.value_of("port").unwrap().parse()?)
    } else {
        unreachable!()
    }
}
